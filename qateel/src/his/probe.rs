use std::path::Path;
use reqwest::Client;
use std::sync::OnceLock;
use std::process::Stdio;
use futures::stream::StreamExt;
use std::collections::HashMap;
use reqwest::header::{
	RANGE,
	ACCEPT_RANGES,
	CONTENT_LENGTH,
	CONTENT_DISPOSITION,
};
use tracing::{error};
use serde_json::{json};
use tokio::sync::RwLock;
use std::net::TcpListener;
use tokio::time::{sleep, Duration, Instant};
use crate::{
	app::AppState,
	his::extract::Extraction,
	aria2::types::{
		UriMeta,
		TorrentMeta,
	},
};

pub const UNTITLED: &str = "<Untitled>";
const HTTP_CLIENT_TIMEOUT_DURATION: u64 = 15;
static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();
static NET_STATE: OnceLock<RwLock<NetState>> = OnceLock::new();

#[derive(Debug, Clone)]
struct NetState {
    status: bool,
    last_checked: Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct MagnetProbeItem {
    pub uri: String,
    pub info_hash: Option<String>,
}

fn net_state() -> &'static RwLock<NetState> {
    NET_STATE.get_or_init(|| {
        RwLock::new(NetState {
            status: true,
            last_checked: None,
        })
    })
}

fn get_available_port() -> Option<u16> {
    TcpListener::bind("127.0.0.1:0")
        .ok()?
        .local_addr()
        .ok()
        .map(|addr| addr.port())
}

pub fn http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(HTTP_CLIENT_TIMEOUT_DURATION))
            .build()
            .unwrap_or_default()
    })
}

pub fn extract_filename(header_val: &str) -> Option<String> {
    if let Some(idx) = header_val.find("filename=\"") {
        let start = idx + 10;
        if let Some(end) = header_val[start..].find('"') {
            return Some(header_val[start..start+end].to_string());
        }
    }

    if let Some(idx) = header_val.find("filename=") {
        let start = idx + 9;
        let end = header_val[start..].find(';').unwrap_or(header_val[start..].len());
        let clean = header_val[start..start+end].trim_matches(|c| c == '"' || c == '\'');
        return Some(clean.to_string());
    }

    None
}

pub async fn is_internet_connected() -> bool {
    let state_lock = net_state();

    // Acquire a read lock and check if the cache is still valid
    {
        let state = state_lock.read().await;
        if let Some(last) = state.last_checked {
            if last.elapsed() < Duration::from_secs(5) {
                return state.status;
            }
        }
    }

    // Acquire a write lock to perform the actual check
    let mut state = state_lock.write().await;

    // Another task might have updated it while waiting for the write lock
    if let Some(last) = state.last_checked {
        if last.elapsed() < Duration::from_secs(5) {
            return state.status;
        }
    }

    // Perform the check with a fast timeout
    let ok = http_client().get("http://clients3.google.com/generate_204")
        .timeout(Duration::from_secs(3))								// Fail fast
        .send().await
        .map(|res| res.status() == 204)
        .unwrap_or(false);

    // Update the cache
    state.status = ok;
    state.last_checked = Some(Instant::now());

    ok
}

/// Probes the url synchronously to extract headers.
/// Returns a Vec with results corresponding to the input URIs.
pub async fn probe_http_uris(
    uris: Vec<String>,
) -> Vec<Result<UriMeta, (String, UriMeta)>> {
    // Fast path
    if uris.is_empty() {
        return vec![];
    }

    // Single global internet connectivity check
    if !is_internet_connected().await {
        return uris
            .into_iter()
            .map(|uri| {
                let meta = UriMeta {
                    name: uri.clone(),
                    total_length: "0".to_string(),
                    resumable: false,
                };
                Err(("No internet connection.".to_string(), meta))
            })
            .collect();
    }

    let client = http_client();

    const MAX_RETRIES: u8 = 3;
    const CONCURRENCY: usize = 100;

    let len = uris.len();
    let mut results: Vec<Option<Result<UriMeta, (String, UriMeta)>>> =
        std::iter::repeat_with(|| None).take(len).collect();

    let stream = futures::stream::iter(uris.into_iter().enumerate()).map(|(idx, uri)| {
        let client = client.clone();

        async move {
            let mut meta = UriMeta {
                name: uri.clone(),
                total_length: "0".to_string(),
                resumable: false,
            };

            let mut filename: Option<String> = None;
            let mut success = false;

            for attempt in 1..=MAX_RETRIES {
                let mut head_success = false;

                if let Ok(res) = client.head(&uri).send().await {
                    // Auth required
                    if res.status() == reqwest::StatusCode::UNAUTHORIZED
                        || res.status() == reqwest::StatusCode::FORBIDDEN
                    {
                        return (
                            idx,
                            Err((
                                "Authentication required, set credentials in options.".to_string(),
                                meta,
                            )),
                        );
                    }

                    if res.status().is_success() {
                        head_success = true;

                        if let Some(val) = res.headers().get(ACCEPT_RANGES) {
                            meta.resumable = val
                                .to_str()
                                .map(|v| v.eq_ignore_ascii_case("bytes"))
                                .unwrap_or(false);
                        }

                        if let Some(val) = res.headers().get(CONTENT_LENGTH) {
                            if let Ok(s) = val.to_str() {
                                meta.total_length = s.to_string();
                            }
                        }

                        if filename.is_none() {
                            if let Some(val) = res.headers().get(CONTENT_DISPOSITION) {
                                if let Ok(s) = val.to_str() {
                                    filename = extract_filename(s);
                                }
                            }
                        }
                    }
                }

                // If HEAD did not give enough info, try a small GET (range 0-0)
                if !head_success || !meta.resumable || filename.is_none() {
                    if let Ok(res_get) = client.get(&uri).header(RANGE, "bytes=0-0").send().await {
                        if res_get.status() == reqwest::StatusCode::UNAUTHORIZED
                            || res_get.status() == reqwest::StatusCode::FORBIDDEN
                        {
                            return (
                                idx,
                                Err((
                                    "Authentication required, set credentials in options.".to_string(),
                                    meta,
                                )),
                            );
                        }

                        if res_get.status().is_success()
                            || res_get.status() == reqwest::StatusCode::PARTIAL_CONTENT
                        {
                            if !meta.resumable {
                                meta.resumable =
                                    res_get.status() == reqwest::StatusCode::PARTIAL_CONTENT
                                        || res_get
                                            .headers()
                                            .get(ACCEPT_RANGES)
                                            .and_then(|v| v.to_str().ok())
                                            .map(|s| s.eq_ignore_ascii_case("bytes"))
                                            .unwrap_or(false);
                            }

                            if meta.total_length == "0" {
                                if let Some(val) = res_get.headers().get(CONTENT_LENGTH) {
                                    if let Ok(s) = val.to_str() {
                                        meta.total_length = s.to_string();
                                    }
                                }
                            }

                            if filename.is_none() {
                                if let Some(val) = res_get.headers().get(CONTENT_DISPOSITION) {
                                    if let Ok(s) = val.to_str() {
                                        filename = extract_filename(s);
                                    }
                                }
                            }

                            success = true;
                        }
                    }
                } else {
                    success = true;
                }

                if success {
                    break;
                }

                // small backoff between retries
                if attempt < MAX_RETRIES {
                    sleep(Duration::from_millis(250)).await;
                }
            }

            let mut final_name = if !success {
                uri.clone()
            } else {
                filename.unwrap_or_else(|| {
                    url::Url::parse(&uri)
                        .ok()
                        .and_then(|u| {
                            u.path_segments()
                                .and_then(|seg| seg.last().map(|s| s.to_string()))
                        })
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| uri.clone())
                })
            };

            final_name = urlencoding::decode(&final_name)
                .map(|d| d.into_owned())
                .unwrap_or(final_name);

            if success {
                meta.name = final_name;
                (idx, Ok(meta))
            } else {
                (idx, Err(("Server is unreachable!!".to_string(), meta)))
            }
        }
    })
    .buffer_unordered(CONCURRENCY);

    tokio::pin!(stream);

    while let Some((idx, res)) = stream.next().await {
        results[idx] = Some(res);
    }

    // Convert Option<Result> to Result, replacing any None (shouldn't happen) with a generic error
    results
        .into_iter()
        .map(|opt| {
            opt.unwrap_or_else(|| {
                let meta = UriMeta {
                    name: "".to_string(),
                    total_length: "0".to_string(),
                    resumable: false,
                };
                Err(("Internal URI probe error.".to_string(), meta))
            })
        })
        .collect()
}


/// Get magnet metadata
pub async fn probe_magnets(
    state: &AppState,
    user_id: &str,
    items: Vec<MagnetProbeItem>,
) -> Vec<Result<UriMeta, (String, UriMeta)>> {
    let mut results: Vec<Option<Result<UriMeta, (String, UriMeta)>>> = vec![None; items.len()];
    let torrent_dir = state.config.data_dir.join("torrents");

    if let Err(e) = tokio::fs::create_dir_all(&torrent_dir).await {
        error!("Failed to create torrents directory: {:?}", e);
        return items
            .into_iter()
            .map(|item| {
                let meta = UriMeta {
                    name: Extraction::extract_mgn_name(&item.uri)
                        .unwrap_or_else(|| item.uri.clone()),
                    total_length: "0".to_string(),
                    resumable: true,
                };
                Err(("Failed to create torrents directory".to_string(), meta))
            })
            .collect();
    }

    // Internet check
    if !is_internet_connected().await {
        return items
            .into_iter()
            .map(|item| {
                let meta = UriMeta {
                    name: Extraction::extract_mgn_name(&item.uri).unwrap_or_else(|| item.uri.clone()),
                    total_length: "0".to_string(),
                    resumable: true,
                };
                Err(("No internet connection.".to_string(), meta))
            })
            .collect();
    }

    // Separate items already cached on disk vs. items needing network probing
    let mut pending_indices = Vec::new();

    for (idx, item) in items.iter().enumerate() {
        let mut meta = UriMeta {
            name: Extraction::extract_mgn_name(&item.uri).unwrap_or_else(|| item.uri.clone()),
            total_length: "0".to_string(),
            resumable: true,
        };

        if let Some(ref hash) = item.info_hash {
            let file_name = format!("{}-{}.torrent", user_id, hash);
            let torrent_path = torrent_dir.join(&file_name);

            if torrent_path.exists() {
                // Already downloaded, read size directly without spawning aria2c
                if let Ok((size, is_private)) = extract_torrent_size(&torrent_path).await {
                    meta.total_length = size.to_string();
                    if is_private {
                        results[idx] = Some(Err(("Private torrent, set credentials in options.".to_string(), meta)));
                    } else {
                        results[idx] = Some(Ok(meta));
                    }
                } else {
                    results[idx] = Some(Ok(meta));
                }
                continue;
            }
        }

        pending_indices.push(idx);
    }

    // If all items were cached, return immediately
    if pending_indices.is_empty() {
        return results.into_iter().map(|r| r.unwrap()).collect();
    }

    // Allocate a single port and spawn single aria2c process for all pending magnets
    let port = match get_available_port() {
        Some(p) => p,
        None => {
            return items
                .into_iter()
                .map(|item| {
                    let meta = UriMeta {
                        name: Extraction::extract_mgn_name(&item.uri)
                            .unwrap_or_else(|| item.uri.clone()),
                        total_length: "0".to_string(),
                        resumable: true,
                    };
                    Err(("Failed to allocate local port for magnet URI probing.".to_string(), meta))
                })
                .collect();
        }
    };

    let batch_concurrent_count = pending_indices.len().max(1);
    const PROBE_TIMEOUT_SECS: u64 = 30;

    let mut child = match tokio::process::Command::new("aria2c")
        .arg("--daemon=false")
        .arg("--enable-rpc=true")
        .arg(format!("--rpc-listen-port={}", port))
        .arg("--rpc-listen-all=false")
        .arg("--no-conf=true")
        .arg(format!("--max-concurrent-downloads={}", batch_concurrent_count))
        .arg(format!("--dir={}", torrent_dir.display()))
        .arg("--bt-metadata-only=true")
        .arg("--bt-save-metadata=true")
        .arg("--bt-stop-timeout=7")
        .arg("--max-tries=3")
        .arg("--retry-wait=0")
        .arg("--console-log-level=error")
        .arg(format!("--timeout={}", PROBE_TIMEOUT_SECS))
        .arg(format!("--bt-tracker-timeout={}", PROBE_TIMEOUT_SECS))
        .arg(format!("--bt-tracker-connect-timeout={}", PROBE_TIMEOUT_SECS))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to spawn magnet probing aria2c process: {:?}", e);
            for idx in pending_indices {
                let item = &items[idx];
                let meta = UriMeta {
                    name: Extraction::extract_mgn_name(&item.uri).unwrap_or_else(|| item.uri.clone()),
                    total_length: "0".to_string(),
                    resumable: true,
                };
                results[idx] = Some(Err(("Failed to launch magnet probing process.".to_string(), meta)));
            }
            return results.into_iter().map(|r| r.unwrap()).collect();
        }
    };

    let client = reqwest::Client::new();
    let rpc_url = format!("http://127.0.0.1:{}/jsonrpc", port);

    // Wait for isolated RPC server to be ready
    let mut rpc_ready = false;
    for _ in 0..15 {
        sleep(Duration::from_millis(100)).await;
        let ping = client
            .post(&rpc_url)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": "ping",
                "method": "aria2.getVersion"
            }))
            .send()
            .await;

        if ping.is_ok() {
            rpc_ready = true;
            break;
        }
    }

    if !rpc_ready {
        let _ = child.kill().await;
        for idx in pending_indices {
            let item = &items[idx];
            let meta = UriMeta {
                name: Extraction::extract_mgn_name(&item.uri).unwrap_or_else(|| item.uri.clone()),
                total_length: "0".to_string(),
                resumable: true,
            };
            results[idx] = Some(Err(("Timed out waiting for magnet probing RPC.".to_string(), meta)));
        }
        return results.into_iter().map(|r| r.unwrap()).collect();
    }

    // `system.multiCall`
    let add_uri_calls: Vec<serde_json::Value> = pending_indices
        .iter()
        .map(|&idx| {
            json!({
                "methodName": "aria2.addUri",
                "params": [[items[idx].uri.clone()]]
            })
        })
        .collect();

    let add_uri_payload = json!({
        "jsonrpc": "2.0",
        "id": "add_uris_multicall",
        "method": "system.multicall",
        "params": [add_uri_calls]
    });

    let mut gid_to_index: HashMap<String, usize> = HashMap::new();

    if let Ok(resp) = client.post(&rpc_url).json(&add_uri_payload).send().await {
        if let Ok(rpc_resp) = resp.json::<serde_json::Value>().await {
            if let Some(res_array) = rpc_resp.get("result").and_then(|r| r.as_array()) {
                for (i, res_item) in res_array.iter().enumerate() {
                    let idx = pending_indices[i];
                    // Successful result element in system.multicall is ["<GID>"]
                    if let Some(gid) = res_item.get(0).and_then(|v| v.as_str()) {
                        gid_to_index.insert(gid.to_string(), idx);
                    }
                }
            }
        }
    }

    // Mark any items that failed to generate a GID as error
    for &idx in &pending_indices {
        if !gid_to_index.values().any(|&mapped_idx| mapped_idx == idx) {
            let item = &items[idx];
            let meta = UriMeta {
                name: Extraction::extract_mgn_name(&item.uri).unwrap_or_else(|| item.uri.clone()),
                total_length: "0".to_string(),
                resumable: true,
            };
            results[idx] = Some(Err(("Failed to add URI to batch probe instance.".to_string(), meta)));
        }
    }

    // system.multiCall
    let start_time = Instant::now();
    let max_poll_duration = Duration::from_secs(PROBE_TIMEOUT_SECS + 5);

    while !gid_to_index.is_empty() && start_time.elapsed() < max_poll_duration {
        sleep(Duration::from_secs(2)).await;

        let active_gids: Vec<String> = gid_to_index.keys().cloned().collect();
        let status_calls: Vec<serde_json::Value> = active_gids
            .iter()
            .map(|gid| {
                json!({
                    "methodName": "aria2.tellStatus",
                    "params": [gid]
                })
            })
            .collect();

        let status_payload = json!({
            "jsonrpc": "2.0",
            "id": "status_multicall",
            "method": "system.multicall",
            "params": [status_calls]
        });

        if let Ok(resp) = client.post(&rpc_url).json(&status_payload).send().await {
            if let Ok(rpc_resp) = resp.json::<serde_json::Value>().await {
                if let Some(res_array) = rpc_resp.get("result").and_then(|r| r.as_array()) {
                    let mut completed_gids = Vec::new();

                    for (i, res_item) in res_array.iter().enumerate() {
                        let gid = &active_gids[i];
                        let idx = match gid_to_index.get(gid) {
                            Some(&idx) => idx,
                            None => continue,
                        };

                        let item = &items[idx];
                        let mut meta = UriMeta {
                            name: Extraction::extract_mgn_name(&item.uri).unwrap_or_else(|| item.uri.clone()),
                            total_length: "0".to_string(),
                            resumable: true,
                        };

                        // system.multicall return value for tellStatus is [{ "status": "...", ... }]
                        let status_res = res_item.get(0);
                        let status = status_res
                            .and_then(|r| r.get("status"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        if status.eq_ignore_ascii_case("complete") {
                            if let Some(ref hash) = item.info_hash {
                                let orig_path = torrent_dir.join(format!("{}.torrent", hash));
                                let target_path = torrent_dir.join(format!("{}-{}.torrent", user_id, hash));

                                if orig_path.exists() {
                                    let _ = tokio::fs::rename(&orig_path, &target_path).await;
                                }

                                if target_path.exists() {
                                    if let Ok((size, is_private)) = extract_torrent_size(&target_path).await {
                                        meta.total_length = size.to_string();
                                        if is_private {
                                            results[idx] = Some(Err(("Private torrent, set credentials in options.".to_string(), meta)));
                                        } else {
                                            results[idx] = Some(Ok(meta));
                                        }
                                    } else {
                                        results[idx] = Some(Ok(meta));
                                    }
                                } else {
                                    results[idx] = Some(Ok(meta));
                                }
                            } else {
                                results[idx] = Some(Ok(meta));
                            }

                            completed_gids.push(gid.clone());
                        } else if status.eq_ignore_ascii_case("error") || status.eq_ignore_ascii_case("removed") {
                            let err_msg = status_res
                                .and_then(|r| r.get("errorMessage"))
                                .and_then(|v| v.as_str())
                                .filter(|s| !s.trim().is_empty())
                                .unwrap_or("Magnet metadata resolution failed.");

                            results[idx] = Some(Err((err_msg.to_string(), meta)));
                            completed_gids.push(gid.clone());
                        }
                    }

                    for gid in completed_gids {
                        gid_to_index.remove(&gid);
                    }
                }
            }
        }
    }

    // Any remaining GIDs timed out
    for (_gid, idx) in gid_to_index {
        let item = &items[idx];
        let meta = UriMeta {
            name: Extraction::extract_mgn_name(&item.uri).unwrap_or_else(|| item.uri.clone()),
            total_length: "0".to_string(),
            resumable: true,
        };
        results[idx] = Some(Err(("Magnet metadata probe timed out.".to_string(), meta)));
    }

    // Kill the single process
    let _ = child.kill().await;

    results.into_iter().map(|r| r.unwrap()).collect()
}

/// Decodes a `.torrent` file to calculate the total length & is it private??
pub async fn extract_torrent_size(path: &Path) -> Result<(u64, bool), String> {
    let bytes = tokio::fs::read(path).await.map_err(|e| e.to_string())?;
    let torrent: TorrentMeta = serde_bencode::from_bytes(&bytes)
		.map_err(|e| e.to_string())?;
	let is_private = torrent.info.private.unwrap_or(0) == 1;

	let size = if let Some(len) = torrent.info.length {
        len
    } else if let Some(files) = &torrent.info.files {
        files.iter().map(|f| f.length).sum()
    } else {
        return Err("No length info found in torrent.".to_string());
    };

	Ok((size, is_private))
}

