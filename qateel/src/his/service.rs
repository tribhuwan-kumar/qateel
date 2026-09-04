use url::Url;
use serde_json::Value;
use tokio::sync::broadcast;
use std::collections::HashSet;
use std::collections::HashMap;
use tokio::time::{self, Duration};
use base64::{Engine as _, engine::general_purpose::STANDARD};

use tracing::debug;
use tracing::{error, info, warn};

use super::{
    extract::Extraction,
    hasher::spawn_content_hasher,
};

use crate::{
    AppState,

    aria2::types::{
        Kind,
        Aria2Res,
        TickTask,
        GidStatus,
        GlobalStat,
        Aria2Options,
        ItemMetaData,
        DdlWsMessage,
		Aria2JsonRpcResp,
        TorrentMagnetMeta,
		GlobalStatWsMessage,
    }
};

const TICK_INTERVAL: u64 = 500;
const TICK_DB_UPDATE_INTERVAL: u64 = 10;

pub struct HistoryService;

impl HistoryService {
    pub async fn init(
        state: AppState,
        mut rx: broadcast::Receiver<Aria2JsonRpcResp>,
    ) {
        Self::sync_init(state.clone()).await;

        // Spawn event listener
        let state_e = state.clone();
        tokio::spawn(async move {
            info!("History event monitor is alive!!");
            while let Ok(msg) = rx.recv().await {
                debug!("Listend aria2 msg: {:?}", &msg);
                if let Some(method) = msg.method {
                    debug!("Listend aria2 method: {:?}", method);
                    let gid = msg.params
                        .as_ref()
                        .and_then(|p| p.first())
                        .and_then(|obj| obj.get("gid"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    if let Some(gid) = gid {
						// Aria2 doesn't emit values on `onDownloadStop`, handling it manually
                        if method == "aria2.onDownloadStop" {
                            debug!("Stop event {:?} for gid {:?} - updating db manually", method, gid);
                            Self::handle_event(&state_e, gid, Some("stopped")).await;
                        } else if method.starts_with("aria2.on") {
                            debug!("Event {:?} for gid {:?}", method, gid);
                            Self::refresh_gid(&state_e, gid).await;
                        }
                    } else if method.starts_with("aria2.on") {
                        warn!("Received '{}' event but couldn't extract the gid", method);
                    }
                }
            }
        });

        /*
          * I can't find any better way to avoid using polling, if you're reading this please fix it!!
        */
        let state_p = state.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(TICK_INTERVAL));
            let mut tick_count = 0u64;
            loop {
                interval.tick().await;
                tick_count = tick_count.wrapping_add(1);
                Self::tick(&state_p, tick_count).await;
            }
        });
    }


    async fn tick(state: &AppState, tick_count: u64) {
        let global_stat = match state.aria2.call("aria2.getGlobalStat", vec![]).await {
            Ok(json) => serde_json::from_value::<GlobalStat>(json).unwrap_or_default(),
            Err(_) => return,
        };

        /*
          * Kinda inefficient, later fix it
          * Get all gid from db
        */
        let status_rows = sqlx::query!(
            "SELECT
                gid,
                user_id
            FROM download_history
            WHERE status IN (
                'active',
                'seeding'
            )"
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        // Publish global stat per user in the current active set
        let mut seen_users = HashSet::new();
        for row in &status_rows {
            if seen_users.insert(row.user_id.clone()) {
                let _ = state.global_tx.send(GlobalStatWsMessage::Global {
                    user_id: row.user_id.clone(),
                    data: global_stat.clone(),
                });
            }
        }

        if status_rows.is_empty() {
            /* Don't send anything */
            return;
        }

        // Fetch details of gid
        let calls: Vec<serde_json::Value> = status_rows.iter().map(|row| {
            serde_json::json!({
                "methodName": "aria2.tellStatus",
                "params": [row.gid]
            })
        }).collect();

        if let Ok(response) = state.aria2.call("system.multicall", vec![serde_json::json!(calls)]).await {
            if let Some(results) = response.as_array() {
                // bundle group result to the `user_id`
                let mut updates_by_user: HashMap<String, Vec<TickTask>> = HashMap::new();
                /* update the database on every 10 ticks */
                let should_update_db = tick_count % TICK_DB_UPDATE_INTERVAL == 0;

                for (i, result) in results.iter().enumerate() {
                    let user_id = status_rows[i].user_id.clone();
                    let gid_ref = status_rows[i].gid.clone();

                    if let Some(status_arr) = result.as_array() {
                        debug!("Response from `tick`, aria2 tellStatus: {:?}", status_arr);
                        if let Some(json_obj) = status_arr.first() {
                            if let Ok(res) = serde_json::from_value::<Aria2Res>(json_obj.clone()) {
                                let meta = Extraction::extract(json_obj, &gid_ref);
								// Convert string bool to actual bool
								let is_verify_integrity_pending: Option<bool> = res.verify_integrity_pending
									.as_deref()
									.and_then(|s| match s {
										"true" => Some(true),
										"false" => Some(false),
										_ => None,
									});
                                let tick_task = TickTask {
                                    gid: meta.gid,
                                    name: meta.name,
                                    status: meta.status,
                                    source_uri: meta.source_uri,
                                    kind: meta.kind,
                                    dir: meta.dir,

									// Values those need streaming updates
                                    upload_length: res.upload_length.clone(),
									completed_length: res.completed_length.clone(),
									total_length: res.total_length.clone(),
									verified_length: res.verified_length.clone(),
									connections: res.connections.clone(),
                                    download_speed: res.download_speed.clone(),
                                    upload_speed: res.upload_speed.clone(),
									num_seeders: res.num_seeders.clone(),
									verify_integrity_pending: is_verify_integrity_pending,
                                };

                                updates_by_user.entry(user_id).or_default().push(tick_task);
                                // Just upating the progress
                                if should_update_db {
                                    let state_db = state.clone();
                                    let res_clone = res.clone();
                                    let gid_ref = status_rows[i].gid.clone();
                                    tokio::spawn(async move {
                                        Self::update_progress(&state_db, &gid_ref, &res_clone).await;
                                    });
                                }
                            }
                        }
                    } else if let Some(code_val) = result.get("code").and_then(|v| v.as_i64()) {
                        debug!("Error from aria2 `tick`: {:?}, error code: {:?}", result.get("message"), code_val);
                        match code_val {
                            1 => {
                                debug!("GID {:?} not found in current aria2 session, maybe aria2 restarted!!", gid_ref);
                                // emit the event
                                Self::handle_event(&state, gid_ref, None).await;
                            },
                            code => {
                                error!("aria2 error for gid {}: code {}, msg: {:?}", gid_ref, code, result.get("message"));
                            },
                        }
                    }
                }

                // Forward bundles
                for (user_id, tasks) in updates_by_user {
                    let msg = DdlWsMessage::Tick {
                        user_id,
                        tasks,
                    };
                    let _ = state.history_tx.send(msg);
                }
            }
        }
    }

	/// Real time meta data updates
    async fn update_progress(state: &AppState, gid: &str, res: &Aria2Res) {
        let value = serde_json::to_value(res).unwrap();
        let meta = Extraction::extract(&value, gid);

        let _ = sqlx::query!(
            r#"
				UPDATE download_history SET
					name = CASE WHEN ? IS NOT NULL THEN ? ELSE name END,
					status = ?,
					files = COALESCE(NULLIF(?, '[]'), files),
					is_resume_supported = CASE WHEN is_resume_supported = 1 THEN 1 ELSE COALESCE(?, is_resume_supported) END,
					completed_length = ?,
					total_length = ?,
					upload_length = ?,
					seeder = ?,
					connections = ?,
					num_seeders = ?,
					verified_length = ?,
					verify_integrity_pending = ?,
					completed_at = CASE
						WHEN ? = 'complete'
						OR ? = 'seeding'
						THEN COALESCE(completed_at, CURRENT_TIMESTAMP)
						ELSE NULL
					END
				WHERE gid = ?
            "#,
            meta.name,
			meta.name,
            meta.status,
			meta.files,
            meta.is_resume_supported,
            res.completed_length,
            res.total_length,
            res.upload_length,
            meta.seeder,
			meta.connections,
			meta.num_seeders,
			meta.verified_length,
			meta.verify_integrity_pending,
            meta.status,
            meta.status,
            gid
        ).execute(&state.db).await;
    }

    async fn sync_init(
        state: AppState,
    ) {
		/*
		  * Use the db metadata and restore all the download in Aria2, like it was before
		  * Restore `active` dl as `active`
		  * Restore `pause` dl as `pause`
		  * Restore `waiting` dl as `waiting`
		  * Restore `seeding` dl as `seeding`
		  * Leave `error`, `stopped`, `completed`, `removed`, let user handle it
		  * After restoring the all dl, in aria2 then we call the tellStatus
		*/
        info!("Bootstrapping aria2 configurations from database...");
		Self::bootstrap_cmd_options(&state).await;
		Self::bootstrap_global_options(&state).await;
		Self::restore_ddl(&state).await;
		Self::bootstrap_gid_options(&state).await;

        /*
          * Check every status on init; anything can be changed
        */
        let gids = sqlx::query!(
            "SELECT gid FROM download_history
            WHERE status IN (
                'active',
                'waiting',
                'paused',
                'stopped',
                'complete',
                'error',
                'removed'
            )"
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        if gids.is_empty() {
            info!("No pending downloads to sync");
            return;
        }

        info!("Checking status of '{}' pending downloads...", gids.len());

        /*
          * Batch process in chunks prevent rpc timeout
          * TODO: Find some way to minimize these hardcoded values
        */
        for chunk in gids.chunks(100) {
            let calls: Vec<Value> = chunk.iter().map(|row| {
                serde_json::json!({
                    "methodName": "aria2.tellStatus",
                    "params": [row.gid]
                })
            }).collect();

            match state.aria2.call("system.multicall", vec![serde_json::json!(calls)]).await {
                Ok(response) => {
                    if let Some(results) = response.as_array() {
                        for (i, result) in results.iter().enumerate() {
                            let gid = &chunk[i].gid;
                            if let Some(status_array) = result.as_array() {
                                if let Some(status_info) = status_array.first() {
                                    // If gid found. update db
                                    let mut meta = Extraction::extract(status_info, gid);
                                    debug!("Item meta data `sync_init`: {:?}", meta);
                                    Self::upsert_db(&state, &mut meta).await;
                                }
                            } else if let Some(code_val) = result.get("code").and_then(|v| v.as_i64()) {
                                // This means it was purged from memory or removed externally
                                // Mark it as 'removed' and don't check it again
                                debug!("Error from aria2 `sync_init`: {:?}, error code: {:?}", result.get("message"), code_val);
                                match code_val {
                                    1 => {
                                        debug!("GID {:?} not found in current aria2 session!!", gid);
                                    },
                                    code => {
                                        error!("aria2 error for gid {}: code {}, msg: {:?}", gid, code, result.get("message"));
                                    },
                                }
                            }
                        }
                    }
                }
                Err(e) => error!("Sync chunk failed: {}", e),
            }
        }
        info!("Sync complete...");
    }

    /// Refresh gid on aria2 event notifications
    pub async fn refresh_gid(state: &AppState, gid: String) {
        info!("Refreshing gid: {:?}", gid);
        let params = vec![serde_json::json!(gid)];
        if let Ok(info) = state.aria2.call("aria2.tellStatus", params).await {
            /* Handle child gid */
            Self::handle_inheritance(state, &gid, &info).await;

            let mut meta = Extraction::extract(&info, &gid);
            Self::upsert_db(state, &mut meta).await;

            /* Spawn content hasher after complete download */
            if (meta.status == GidStatus::Complete || meta.status == GidStatus::Seeding) && meta.content_hash.is_none() {
                spawn_content_hasher(state.clone(), meta.gid.clone(), meta.files.clone());
            }
            debug!("Refreshed extracted meta: {:?}", meta);
        }
    }

    /// Called on aria2 event notifications
    async fn upsert_db(state: &AppState, meta: &mut ItemMetaData) {
        debug!("Status from `upsert_db`: name: {:?}, gid: {:?}, status: {:?}",
			meta.name, meta.gid, meta.status);
        let res = sqlx::query!(
            r#"
				UPDATE download_history
				SET
					name = CASE WHEN ? IS NOT NULL THEN ? ELSE name END,
					status = ?,
					dir = COALESCE(NULLIF(?, ''), dir),
					files = COALESCE(NULLIF(?, '[]'), files),
					total_length = COALESCE(NULLIF(?, '0'), total_length),
					completed_length = COALESCE(NULLIF(?, '0'), completed_length),
					upload_length = COALESCE(NULLIF(?, '0'), upload_length),
					source_uri = COALESCE(NULLIF(?, ''), source_uri),
					info_hash = COALESCE(NULLIF(?, ''), info_hash),
					seeder = ?,
					content_hash = COALESCE(NULLIF(?, ''), content_hash),
					error_code = ?,
					error_message = ?,
					torrent = COALESCE(?, torrent),
					options = COALESCE(?, options),
					is_resume_supported = CASE WHEN is_resume_supported = 1 THEN 1 ELSE COALESCE(?, is_resume_supported) END,
					connections = ?,
					num_pieces = COALESCE(NULLIF(?, '0'), num_pieces),
					num_seeders = ?,
					piece_length = COALESCE(NULLIF(?, '0'), piece_length),
					verified_length = COALESCE(NULLIF(?, '0'), verified_length),
					verify_integrity_pending = ?,
					completed_at = CASE
						WHEN ? = 'complete' OR
							 ? = 'seeding'
							 THEN COALESCE(completed_at, CURRENT_TIMESTAMP)
						ELSE NULL
				END,
					updated_at = CURRENT_TIMESTAMP
				WHERE gid = ?
				RETURNING user_id, created_at, completed_at
            "#,
            meta.name,
			meta.name,
            meta.status,
            meta.dir,
            meta.files,
            meta.total_length,
            meta.completed_length,
            meta.upload_length,
            meta.source_uri,
            meta.info_hash,
            meta.seeder,
			meta.content_hash,
            meta.error_code,
            meta.error_message,
			meta.torrent,
			meta.options,
			meta.is_resume_supported,
			meta.connections,
			meta.num_pieces,
			meta.num_seeders,
			meta.piece_length,
			meta.verified_length,
			meta.verified_length,
            meta.status,
            meta.status,
            meta.gid
        )
        .fetch_optional(&state.db)
        .await;

        match res {
            Ok(Some(row)) => {
                meta.user_id = row.user_id.clone();
                meta.created_at = row.created_at;
                meta.completed_at = row.completed_at;
                /* Send on global channel */
                let msg = DdlWsMessage::Event {
                    user_id: row.user_id,
                    data: meta.clone(),
                };
                debug!("Item metadata `upsert_db`: {:?}", msg);
                let _ = state.history_tx.send(msg);
            },
            Ok(None) => {
                warn!("Received aria2 update for unknown gid: {:?}, download: {:?} weird error", meta.gid, meta.name);
            },
            Err(e) => error!("Database update failed: {}", e),
        }
    }

    /// Manually marks a gid as some 'status'
	/// Because aria2 doesn't emits evets for some status
    pub async fn handle_event(
        state: &AppState,
        gid: String,
        status: Option<&str>
    ) {
        let res = sqlx::query_as!(
            ItemMetaData,
            r#"
				UPDATE download_history
				SET
					status = COALESCE(?, status),
					updated_at = CASE WHEN ? IS NOT NULL THEN CURRENT_TIMESTAMP ELSE updated_at END
				WHERE gid = ?
				RETURNING
					gid,
					name,
					kind as "kind: Kind",
					status as "status: GidStatus",
					dir,
					files,
					total_length,
					completed_length,
					upload_length,
					source_uri,
					info_hash,
					seeder,
					content_hash,
					error_code,
					error_message,
					torrent as "torrent: sqlx::types::Json<TorrentMagnetMeta>",
					options,
					is_resume_supported,
					connections,
					num_pieces,
					num_seeders,
					piece_length,
					verified_length,
					verify_integrity_pending,
					created_at,
					completed_at,
					user_id
            "#,
            status,
            status,
            gid
        )
        .fetch_optional(&state.db)
        .await;

        match res {
            Ok(Some(meta)) => {
                /* Broadcast the manual update so the ui turns "stopped" immediately */
                let msg = DdlWsMessage::Event {
                    user_id: meta.user_id.clone(),
                    data: meta,
                };
                let _ = state.history_tx.send(msg);
            },
            Ok(None) => warn!("Received {:?} event for unknown or already deleted gid: '{}'", status, gid),
            Err(e) => error!("Failed to manually stop gid: '{}': '{}'", gid, e),
        }
    }

    /// Handles gid linking for parent -><- child transitions
    pub async fn handle_inheritance(
        state: &AppState,
        current_gid: &str,
        info: &serde_json::Value										// `aria2.tellStatus` res
    ) {
        if let Some(children) = info.get("followedBy").and_then(|v| v.as_array()) {
            for child in children {
                if let Some(child_gid) = child.as_str() {
                    let res = sqlx::query!(
                        r#"
                            INSERT INTO download_history (
                                gid,
                                user_id,
                                kind,
                                source_uri,
                                status,
								options,
                                created_at,
                                updated_at
                            )
                            SELECT ?,
                                user_id,
                                kind,
                                source_uri,
                                'waiting',
								options,
                                CURRENT_TIMESTAMP,
                                CURRENT_TIMESTAMP
                            FROM download_history
                            WHERE gid = ?
                            ON CONFLICT(gid) DO NOTHING
                        "#,
                        child_gid,
                        current_gid
                    )
                    .execute(&state.db)
                    .await;

                    if let Ok(r) = res {
                        if r.rows_affected() > 0 {
                            info!("Forward inheritance: Parent {} spawned Child {}", current_gid, child_gid);
                        }
                    }
                }
            }
        }

        if let Some(parent_gid) = info.get("following").and_then(|v| v.as_str()) {
            let res = sqlx::query!(
                r#"
                    INSERT INTO download_history (
                        gid,
                        user_id,
                        kind,
                        source_uri,
                        status,
						options,
                        created_at,
                        updated_at
                        )
                    SELECT
					?,
                    user_id,
                    kind,
                    source_uri,
                    'active',
					options,
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                    FROM download_history
                    WHERE gid = ?
                    ON CONFLICT(gid) DO NOTHING
                "#,
                current_gid, parent_gid
            )
            .execute(&state.db)
            .await;

            if let Ok(r) = res {
                if r.rows_affected() > 0 {
                    info!("Reverse Inheritance: Child {} copied context from Parent {}", current_gid, parent_gid);
                }
            }
        }
    }

	async fn bootstrap_global_options(state: &AppState) {
        if let Ok(Some(global_record)) = sqlx::query!(
            "SELECT value FROM settings WHERE key = 'aria2_global_options' ORDER BY updated_at DESC LIMIT 1"
        )
        .fetch_optional(&state.db)
        .await
        {
            if let Ok(parsed_opts) = serde_json::from_str::<Aria2Options>(&global_record.value) {
                let params = vec![serde_json::json!(parsed_opts.0)];
                if let Err(e) = state.aria2.call("aria2.changeGlobalOption", params).await {
                    error!("Failed to restore global options: {}", e);
                } else {
                    info!("Successfully restored global options.");
                }
            }
        }
	}

	async fn bootstrap_gid_options(state: &AppState) {
        let records = sqlx::query!(
            r#"
				SELECT
					gid,
					options
				FROM download_history
				WHERE status IN (
					'active',
					'waiting',
					'paused',
					'stopped',
					'complete',
					'error',
					'removed'
            )
            AND options IS NOT NULL AND options != '{}'
            "#
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        if !records.is_empty() {
            let mut calls: Vec<Value> = Vec::new();

            for record in records {
                if let Some(opts_str) = record.options {
                    if let Ok(parsed_opts) = serde_json::from_str::<Aria2Options>(&opts_str) {
                        calls.push(serde_json::json!({
                            "methodName": "aria2.changeOption",
                            "params": [record.gid, parsed_opts]
                        }));
                    }
                }
            }

            if !calls.is_empty() {
                if let Err(e) = state.aria2.call("system.multicall", vec![serde_json::json!(calls)]).await {
                    error!("Failed to restore local options via multicall: {}", e);
                } else {
                    info!("Successfully restored local options for {} downloads.", calls.len());
                }
            }
        }
	}

	async fn bootstrap_cmd_options(state: &AppState) {
		let saved_cmd_args = match sqlx::query!(
				"SELECT value FROM settings WHERE key = 'aria2_cmd_options' ORDER BY updated_at DESC LIMIT 1"
			)
			.fetch_optional(&state.db)
			.await
			{
				Ok(Some(record)) => {
					serde_json::from_str::<HashMap<String, String>>(&record.value).unwrap_or_default()
				}
				_ => HashMap::new(),
			};
		if let Err(e) = state.daemon.start(&state.config, &saved_cmd_args).await {
			error!("Failed to launch backend aria2c managed execution handle: {}", e);
		} else {
			info!("Managed aria2c backend daemon bound successfully.");
		}
	}

	async fn restore_ddl(state: &AppState) {
        info!("Restoring interrupted downloads into Aria2...");
        // Fetch active/paused downloads, including kind and info_hash
        let records = sqlx::query!(
            r#"
				SELECT gid, source_uri, options, status, kind, is_resume_supported, info_hash, user_id
				FROM download_history
				WHERE status IN ('active', 'waiting', 'paused', 'seeding')
            "#
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        if records.is_empty() {
            info!("No interrupted downloads to restore.");
            return;
        }

        let mut calls: Vec<Value> = Vec::new();
		let mut unresumable_gids: Vec<String> = Vec::new();

        for row in records {
			if row.is_resume_supported == Some(false) {
				unresumable_gids.push(row.gid.clone());
				continue;
			}

            let mut opts = if let Some(opts_str) = row.options {
                serde_json::from_str::<serde_json::Map<String, Value>>(&opts_str)
                    .unwrap_or_default()
            } else {
                serde_json::Map::new()
            };

            // Force the GID to match the DB
            opts.insert("gid".to_string(), Value::String(row.gid.clone()));

            if row.status == "paused" {
                opts.insert("pause".to_string(), Value::String("true".to_string()));
            }

			if row.status == "seeding" {
				opts.insert("check-integrity".to_string(), Value::String("true".to_string()));
			}

            if row.kind == "torrent" {
				if let Some(uri) = &row.source_uri {
					if let Ok(u) = Url::parse(&uri) {
						if u.scheme() == "magnet" {
							calls.push(serde_json::json!({
								"methodName": "aria2.addUri",
								"params": [vec![uri], opts]
							}));

							/* Skip the `.torrent` file lookup */
							continue;
						}
					}
				}
				// `user_id-gid.torrent` format
				let torrent_path = state.config.data_dir
						.join("torrents")
						.join(format!("{}-{}.torrent", row.user_id, row.gid));

				if torrent_path.exists() {
					if let Ok(bytes) = tokio::fs::read(&torrent_path).await {
						let b64 = STANDARD.encode(&bytes);
						calls.push(serde_json::json!({
							"methodName": "aria2.addTorrent",
							"params": [b64, Vec::<String>::new(), opts]
						}));
					} else {
						error!("Found torrent file but lacked read permissions: {:?}", torrent_path);
					}
				} else {
					warn!("Torrent file missing on disk for GID {:?}. Cannot restore.", row.gid);
				}
            } else {
				// URI restoration
                if let Some(uri) = row.source_uri {
                    calls.push(serde_json::json!({
                        "methodName": "aria2.addUri",
                        "params": [vec![uri], opts]
                    }));
                } else {
                    warn!("Can't restore URI for GID {:?}: Missing `source_uri`", row.gid);
                }
            }
        }

        if !unresumable_gids.is_empty() {
            for gid in &unresumable_gids {
                let _ = sqlx::query!(
                    r#"
                        UPDATE download_history
                        SET status = 'error',
                            error_code = 8,
                            error_message = 'Server does not support resume'
                        WHERE gid = ?
                    "#,
                    gid
                )
                .execute(&state.db)
                .await;
            }
            warn!("Marked {:?} downloads as `error_code` `8` because they do not support resume.", unresumable_gids.len());
		}

        if !calls.is_empty() {
            for chunk in calls.chunks(100) {
                if let Err(e) = state.aria2.call("system.multicall", vec![serde_json::json!(chunk)]).await {
                    error!("Failed to inject restored downloads: {}", e);
                }
            }
            info!("Successfully injected {:?} downloads back into Aria2.", calls.len());
        }
    }
}

