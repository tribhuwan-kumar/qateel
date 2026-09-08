use url::Url;
use serde_json::Value;
use tokio::sync::watch;
use tokio::time::sleep;
use std::time::Duration;
use std::collections::HashMap;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use std::sync::{atomic::{AtomicU64, Ordering}, Arc};
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message
};
use tracing::{error, debug, info, warn};

use crate::{
    status::SysStatus,
};

use super::types::{
    Command,
    Aria2Client,
    Aria2Worker,
    Aria2JsonRpcReq,
    Aria2JsonRpcResp,
};

impl Aria2Client {
    pub fn new(
        url: String,
        secret: String,
        status_tx: Arc<watch::Sender<SysStatus>>,
        resp_tx: broadcast::Sender<Aria2JsonRpcResp>,
    ) -> Self {
        let (command_tx, command_rx) = mpsc::channel(32);

        let client = Self {
            command_tx,
            events: resp_tx.clone(),
        };

        // Spawn the background worker
        tokio::spawn(async move {
            Aria2Worker::run(url, secret, command_rx, resp_tx, status_tx).await;
        });

        client
    }

    /// For making RPC calls
    pub async fn call(&self, method: &str, params: Vec<Value>) -> Result<Value, String> {
        let (reply_tx, reply_rx) = oneshot::channel();

        self.command_tx
            .send(Command::Call {
                method: method.to_string(),
                params,
                reply: reply_tx,
            })
            .await
            .map_err(|_| "Aria2 worker is dead".to_string())?;

        reply_rx.await.map_err(|_| "Response channel closed".to_string())?
    }
}

impl Aria2Worker {
    async fn run(
        url: String,
        secret: String,
        mut command_rx: mpsc::Receiver<Command>,
        resp_tx: broadcast::Sender<Aria2JsonRpcResp>,
        status_tx: Arc<watch::Sender<SysStatus>>,
    ) {
        let worker = Arc::new(Self {
            url: url.clone(),
            secret: secret.clone(),
            id_counter: AtomicU64::new(1),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
        });

		// Wait a little till aria2 daemon starts
        sleep(Duration::from_millis(500)).await;

        loop {
            /* False before trying to connect */
            status_tx.send_modify(
                |s| s.aria2_alive = false
            );

            info!("Connecting Qateel to Aria2 at: {:?}", worker.url);
            let parsed_url = Url::parse(&worker.url).map_err(|e| error!("Invalid url: {:?}", e));

            match connect_async(parsed_url.unwrap().as_str()).await {
                Ok((ws_stream, _)) => {
                    info!("Connected to Aria2 daemon!");
                    /* Make it true */
                    status_tx.send_modify(
                        |s| s.aria2_alive = true
                    );
                    let (mut write, mut read) = ws_stream.split();

                    loop {
                        tokio::select! {
                            /* Sending commands from app to aria2 daemon */
                            cmd = command_rx.recv() => {
                                match cmd {
                                    Some(Command::Call { method, mut params, reply }) => {
                                        let id = worker.id_counter.fetch_add(1, Ordering::SeqCst).to_string();

                                        /* Inject secret token if needed */
										#[allow(irrefutable_let_patterns)]
                                        if let ref s = worker.secret {
                                            let token = Value::String(format!("token:{}", s));

                                            if method == "system.multicall" {
                                                /*
                                                  *  Injecting token into every inner call inside the array
                                                  *  params[0] is the array of calls: [ {methodName:..., params:[...]}, ... ]
                                                */
                                                if let Some(calls_arr) = params.get_mut(0).and_then(|v| v.as_array_mut()) {
                                                    for call in calls_arr {
                                                        /* Each 'call' is a json object */
                                                        if let Some(obj) = call.as_object_mut() {
                                                            /* Check if inner method needs auth */
                                                            let needs_token = obj.get("methodName")
                                                                .and_then(|n| n.as_str())
                                                                .map(|name| !name.starts_with("system."))
                                                                .unwrap_or(true);

                                                            if needs_token {
                                                                /* Inject token at index `0` of the inner params */
                                                                if let Some(inner_params) = obj.get_mut("params").and_then(|p| p.as_array_mut()) {
                                                                    inner_params.insert(0, token.clone());
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            } else if !method.starts_with("system.") {
                                                /* Standard case; single call!! */
                                                params.insert(0, token);
                                            }
                                        }

                                        let req = Aria2JsonRpcReq {
                                            id: id.clone(),
                                            params: params,
                                            method: method,
                                            jsonrpc: "2.0".into(),
                                        };

                                        /* Store the reply channel */
                                        worker.pending_requests.lock().await.insert(id, reply);

                                        let json = serde_json::to_string(&req).unwrap();
                                        if let Err(e) = write.send(Message::Text(json.into())).await {
                                            error!("Failed to send to Aria2: {}", e);
                                            break;
                                        }
                                    }
                                    None => return, /* Channel closed, shutdown */
                                }
                            }

                            /* Receiving messages from aria2 to app */
                            msg = read.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        worker.handle_message(&text, &resp_tx).await;
                                    }
                                    Some(Err(e)) => {
                                        error!("Aria2 socket error: {}", e);
                                        break;
                                    }
                                    None => {
                                        warn!("Aria2 socket is closed");
                                        break;
                                    }
                                    _ => {} /* Ignore binary/ping/pong for now */
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    status_tx.send_modify(
                        |s| s.aria2_alive = false
                    );
                    error!("Failed to connect to Aria2: {}. Retrying in 2s...", e);
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    /// Broadcast aria2 messages
    async fn handle_message(&self, text: &str, resp_tx: &broadcast::Sender<Aria2JsonRpcResp>) {
        if let Ok(resp) = serde_json::from_str::<Aria2JsonRpcResp>(text) {
            /* Response to request */
            if let Some(id) = &resp.id {
                let mut pending = self.pending_requests.lock().await;
                if let Some(reply) = pending.remove(id) {
                    if let Some(result) = resp.result {
                        let _ = reply.send(Ok(result));
                    } else if let Some(err) = resp.error {
                        let _ = reply.send(Err(err.to_string()));
                    }
                }
            }
            /* Notification event */
            else if let Some(method) = &resp.method.clone() {
                /* Broadcast it... */
                let _ = resp_tx.send(resp);
                debug!("Aria2 event: {:?}", method);
            }
        }
    }
}

