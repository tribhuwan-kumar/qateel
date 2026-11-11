use url::Url;
use std::slice;
use serde_json::Value;
use std::path::PathBuf;
use tokio::sync::broadcast;
use std::collections::HashSet;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use notify_debouncer_full::{
	new_debouncer,
	notify::{RecursiveMode, EventKind},
	notify::event::{ModifyKind, RenameMode}
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use tracing::debug;
use tracing::{error, info, warn};

use super::{
    hasher::spawn_content_hasher,
};

use crate::{
    AppState,
	aria2::proxy::MAX_MULTICALL_BATCH,
    aria2::types::{
        Kind,
        Aria2Res,
        TickTask,
		Category,
        GidStatus,
        GlobalStat,
		DlWsMessage,
        Aria2Options,
        ItemMetaData,
		Aria2JsonRpcResp,
		GlobalStatWsMessage,
    }
};

const TICK_INTERVAL: u64 = 500;
const DB_UPDATE_INTERVAL: u64 = 10;

pub struct HistoryService;

impl HistoryService {
    pub async fn init(
        state: AppState,
        mut rx: broadcast::Receiver<Aria2JsonRpcResp>,
    ) {
        Self::sync_init(state.clone()).await;
		Self::watch_aria2_dir(state.clone()).await;
		Self::progress_updater(state.clone()).await;

        // Spawn event listener
        let state_e = state.clone();
        tokio::spawn(async move {
            info!("History event monitor is alive!!");
			loop {
				match rx.recv().await {
					Ok(msg) => {
						debug!("Aria2 msg from event listener: {:?}", &msg);
						if let Some(method) = msg.method {
							debug!("Listend event for aria2 method: {:?}", method);
							let gid = msg.params
								.as_ref()
								.and_then(|p| p.first())
								.and_then(|obj| obj.get("gid"))
								.and_then(|v| v.as_str())
								.map(|s| s.to_string());

							if let Some(gid) = gid {
								let state_cln = state_e.clone();
								// Aria2 doesn't emit values on `onDownloadStop`, handling it manually
								tokio::spawn(async move {
									if method == "aria2.onDownloadStop" {
										debug!("Stop event {:?} for gid {:?}; updating db manually", method, gid);
										Self::handle_event(&state_cln, slice::from_ref(&gid), Some(GidStatus::Stopped)).await;
									} else if method == "aria2.onDownloadStart" {
										let already_alive = state_cln.active_tasks.tasks.read().await.contains_key(&gid);
										if !already_alive {
											Self::refresh_gid(&state_cln, gid).await;
										}
									} else if method.starts_with("aria2.on") {
										debug!("Updating event {:?} for gid {:?}", method, gid);
										Self::refresh_gid(&state_cln, gid).await;
									}
								});
							} else if method.starts_with("aria2.on") {
								warn!("Received {:?} event but couldn't extract the gid", method);
							}
						}
					}

					Err(broadcast::error::RecvError::Lagged(skipped)) => {
						warn!("History even listener lagged by {:?} messages; continuing listener", skipped);
					}
					Err(broadcast::error::RecvError::Closed) => {
						warn!("History event listener channel closed; exiting monitor");
						break;
					}
				}
			}
		});

        /*
          * I can't find any better way to avoid using polling, if you're reading this please fix it!!
        */
        let state_p = state.clone();
        tokio::spawn(async move {
            loop {
				sleep(Duration::from_millis(TICK_INTERVAL)).await;
                Self::tick(&state_p).await;
            }
        });
    }

	/// Stream stuff
    async fn tick(state: &AppState) {
		// Send stas
        let global_stat = match state.aria2.call("aria2.getGlobalStat", vec![]).await {
            Ok(json) => serde_json::from_value::<GlobalStat>(json).unwrap_or_default(),
            Err(_) => return,
        };

		let (gids, user_ids): (Vec<String>, HashSet<String>) = {
			let tasks_guard = state.active_tasks.tasks.read().await;
			let gids = tasks_guard.keys().cloned().collect::<Vec<_>>();
			let user_ids = tasks_guard
				.values()
				.filter_map(|item| {
					if item.user_id.is_empty() {
						None
					} else {
						Some(item.user_id.clone())
					}
				})
			.collect::<HashSet<_>>();

			(gids, user_ids)
		};

		if gids.is_empty() {
			return;
		}

        // Publish global stat per user in the current active set
		for user_id in user_ids {
			let _ = state.global_tx.send(GlobalStatWsMessage::Global {
				user_id,
				data: global_stat.clone(),
			});
		}

        // Get details of gid
		let calls: Vec<Value> = gids
			.iter()
			.map(|gid| {
				serde_json::json!({
					"methodName": "aria2.tellStatus",
					"params": [gid, [
						"gid",
						"dir",
						"status",
						"downloadSpeed",
						"uploadSpeed",
						"totalLength",
						"completedLength",
						"uploadLength",
						"errorCode",
						"errorMessage",
						// "bitfield",
						"infoHash",
						"bittorrent",
						"files",
						"connections",
						"numPieces",
						"numSeeders",
						"pieceLength",
						"following",
						"followedBy",
						"belongsTo",
						"seeder",
						"verifiedLength",
						"verifyIntegrityPending",
					]]
				})
			})
		.collect();

        if let Ok(response) = state.aria2.call("system.multicall", vec![serde_json::json!(calls)]).await {
            if let serde_json::Value::Array(results) = response {
                // Bundle group result to the `user_id`
                let mut updates_by_user: HashMap<String, Vec<TickTask>> = HashMap::new();
				// Free the evicted tasks from memory
				let mut items_to_evict: Vec<String> = Vec::new();
                /* Update the database on every 10 ticks */
                for (i, result) in results.into_iter().enumerate() {
					let gid = &gids[i];

                    if let serde_json::Value::Array(mut status_arr) = result {
						if !status_arr.is_empty() {
							let json_obj = status_arr.remove(0);

							if let Ok(mut res) = serde_json::from_value::<Aria2Res>(json_obj) {
								let upload_speed = std::mem::take(&mut res.upload_speed);
								let download_speed = std::mem::take(&mut res.download_speed);
								let num_seeders = res.num_seeders.take();
								let connections = res.connections.take();

								let meta = ItemMetaData::extract(res, gid);

								if let Some((user_id, tick_task)) = state.active_tasks
									.upsert_tick(gid, meta, upload_speed, download_speed, num_seeders, connections) .await
								{
									updates_by_user
										.entry(user_id)
										.or_default()
										.push(tick_task.clone());

									state.progress_buf.add_update(tick_task).await;
								} else {
									/*
									  *  If `upsert_tick` returned None, means the task is either
									  *  terminal or not streaming, so queue it for eviction.
									*/
									items_to_evict.push(gid.clone());
								}
							}
						}
                    } else if let Some(code_val) = result.get("code").and_then(|v| v.as_i64()) {
                        debug!("Error from aria2 `tick`: {:?}, error code: {:?}", result.get("message"), code_val);
                        match code_val {
                            1 => {
								// Emit the event
                                debug!("GID: '{:?}' not found in current aria2 session, maybe aria2 restarted!!", gid);
								Self::handle_event(state, slice::from_ref(gid), None).await;
								state.active_tasks.remove(gid).await; // directly remove on error
                            },
                            code => {
                                error!("Aria2 error for gid {}: code {}, msg: {:?}", gid, code, result.get("message"));
                            },
                        }
                    }
                }

                // Forward bundles
                for (user_id, tasks) in updates_by_user {
                    let msg = DlWsMessage::Tick {
                        user_id,
                        tasks,
                    };

                    let _ = state.history_tx.send(msg);
                }

				// Evict terminal tasks, persist their final status to db & spawn content hasher
				for gid_to_remove in items_to_evict {
					if let Some(final_meta) = state.active_tasks.remove(&gid_to_remove).await {
						state.progress_buf.remove(&gid_to_remove).await;

						debug!("Evicted terminal GID {:?} from active_tasks map; status is {:?}", gid_to_remove, final_meta.status);

						let is_complete_or_seeding = (
							final_meta.status == GidStatus::Complete
							|| final_meta.status == GidStatus::Seeding
						) && final_meta.content_hash.is_none();

						let hash = final_meta.gid.clone();
						let files = final_meta.files.clone();

						Self::upsert_db(state, final_meta).await;

						if is_complete_or_seeding {
							spawn_content_hasher(state.clone(), hash, files);
						}
					}
				}
            }
        }
    }

	/// DB updater on progress task
	pub async fn progress_updater(state: AppState) {
		tokio::spawn(async move {
			let mut interval = tokio::time::interval(
				Duration::from_secs(DB_UPDATE_INTERVAL)
			);
			let mut shutdown_rx = state.shutdown_tx.subscribe();

			loop {
				tokio::select! {
					_ = interval.tick() => {
						Self::update_progress(&state).await;
					}
					_ = shutdown_rx.recv() => {
						info!("Flushing progress buffer received shutdown signal.");
						// Final update before exit
						Self::update_progress(&state).await;
						break;
					}
				}
			}
		});
	}

	async fn update_progress(state: &AppState) {
		let updates = state.progress_buf.get_all_updates().await;

		if updates.is_empty() {
			return;
		}

		debug!("Updating {:?} to database", updates.len());

		// Batch insert or update using a single db transaction
		let mut tx = match state.db.begin().await {
			Ok(t) => t,
			Err(e) => {
				error!("Failed to begin transaction: {:?}", e);
				return;
			}
		};

		for update in updates {
			let still_streaming = {
				let tasks_guard = state.active_tasks.tasks.read().await;
				tasks_guard.get(&update.gid).map(|item| item.is_streaming()).unwrap_or(false)
			};

			if !still_streaming {
				debug!("Skipping stale progress update for gid {:?} because it is no longer streaming.", update.gid);
				continue;
			}

			let res = sqlx::query!(
				r#"
				UPDATE download_history SET
					name = CASE WHEN ? IS NOT NULL THEN ? ELSE name END,
					status = ?,
					files = COALESCE(NULLIF(?, '[]'), files),
					is_resume_supported = CASE WHEN is_resume_supported = 'true' THEN 'true' ELSE COALESCE(?, is_resume_supported) END,
					completed_length = ?,
					total_length = ?,
					upload_length = ?,
					seeder = ?,
					verified_length = ?,
					verify_integrity_pending = ?,
					completed_at = CASE
							WHEN ? = 'complete'
							OR ? = 'seeding'
							THEN COALESCE(completed_at, CURRENT_TIMESTAMP)
							ELSE NULL
						END,
					updated_at = CURRENT_TIMESTAMP
				WHERE gid = ?
				"#,
				update.name,
				update.name,
				update.status,
				update.files,
				update.is_resume_supported,
				update.completed_length,
				update.total_length,
				update.upload_length,
				update.seeder,
				update.verified_length,
				update.verify_integrity_pending,
				update.status,
				update.status,
				update.gid
			)
			.execute(&mut *tx)
			.await;

			if let Err(e) = res {
				error!("Failed to update progress for gid {:?}: {:?}", update.gid, e);
			}
		}

		if let Err(e) = tx.commit().await {
			error!("Failed to commit progress update: {:?}", e);
		}
	}

	/// Syncing on `qateel` start
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
		  * After restoring the all dl, in aria2 then call the tellStatus
		*/
        info!("Bootstrapping aria2 configurations from database...");
		Self::bootstrap_cmd_options(&state).await;
		Self::bootstrap_global_options(&state).await;
		Self::restore_dl(&state).await;
		// Self::bootstrap_gid_options(&state).await;

        /*
          * Check every status on init; anything can be changed
        */
		let active_map = state.active_tasks.tasks.read().await.clone();

        if active_map.is_empty() {
            info!("No pending downloads to sync");
            return;
        }

        info!("Checking status of '{}' active downloads...", active_map.len());
		let gids: Vec<String> = active_map.keys().cloned().collect();
        /*
          * Batch process in chunks prevent rpc timeout
          * TODO: Find some way to minimize these hardcoded values
        */
        for chunk in gids.chunks(100) {
			let calls: Vec<Value> = chunk
				.iter()
				.map(|gid| {
					serde_json::json!({
						"methodName": "aria2.tellStatus",
						"params": [gid]
					})
				}).collect();

            match state.aria2.call("system.multicall", vec![serde_json::json!(calls)]).await {
                Ok(response) => {
                    if let serde_json::Value::Array(results) = response {
                        for (i, result) in results.into_iter().enumerate() {
							let gid = &chunk[i];

                            if let serde_json::Value::Array(mut status_arr) = result {
								if !status_arr.is_empty() {
									let status_info = status_arr.remove(0);
									if let Ok(info) = serde_json::from_value::<Aria2Res>(status_info)  {
										// If gid found, update db
										let meta = ItemMetaData::extract(info, gid);
										if let Some((is_term, _)) = state.active_tasks.tele_status(gid, meta).await {
											let meta = {
												let guard = state.active_tasks.tasks.read().await;
												guard.get(gid).cloned()
											};
											// Sync status to db
											if let Some(metadata) = meta {
												Self::upsert_db(&state, metadata).await;
											}
											// Evict terminal dls
											if is_term {
												state.active_tasks.remove(gid).await;
											}
										}
									}
								}
                            } else if let Some(code_val) = result.get("code").and_then(|v| v.as_i64()) {
                                // This means it was purged from memory or removed externally
                                debug!("Error from aria2 `sync_init`: {:?}, error code: {:?}", result.get("message"), code_val);
                                match code_val {
                                    1 => {
                                        debug!("GID {:?} not found in current aria2 session!!", gid);
										state.active_tasks.remove(gid).await;
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

    /// Refresh gid in db, calling when needed
    pub async fn refresh_gid(state: &AppState, gid: String) {
        info!("Refreshing gid: {:?}", gid);
        let params = vec![serde_json::json!(gid)];
        if let Ok(json_value) = state.aria2.call("aria2.tellStatus", params).await {
			let info: Aria2Res = match serde_json::from_value(json_value) {
				Ok(v) => v,
				Err(e) => {
					error!("Failed to parse aria2 status during refresh for GID '{:?}': {:?}", gid, e);
					return;
				}
			};

            /* Handle parent, child gid */
            Self::handle_inheritance(state, &gid, &info).await;

            let meta = ItemMetaData::extract(info, &gid);

			let is_complete_or_seeding = (
				meta.status == GidStatus::Complete
				|| meta.status == GidStatus::Seeding
			) && meta.content_hash.is_none();

			let hash = meta.gid.clone();
            let files = meta.files.clone();

			/* Update the db */
            Self::upsert_db(state, meta).await;

            /* Spawn content hasher after complete download */
			if is_complete_or_seeding {
                spawn_content_hasher(state.clone(), hash, files);
            }
        }
    }

    /// Called on aria2 event notifications, updating volatile vars
    async fn upsert_db(state: &AppState, mut meta: ItemMetaData) {
        debug!("Status from `upsert_db`: name: {:?}, gid: {:?}, status: {:?}", meta.name, meta.gid, meta.status);
		if meta.is_streaming() {
			state.active_tasks.insert(meta.gid.clone(), meta.clone()).await;
		} else {
			state.active_tasks.remove(&meta.gid).await;
			state.progress_buf.remove(&meta.gid).await;
		}
        let res = sqlx::query!(
            r#"
				UPDATE download_history
				SET
					name = CASE WHEN $1 IS NOT NULL THEN $1 ELSE name END,
					status = $2,
					dir = COALESCE(NULLIF($3, ''), dir),
					files = COALESCE(NULLIF($4, '[]'), files),
					total_length = COALESCE(NULLIF($5, '0'), total_length),
					completed_length = COALESCE(NULLIF($6, '0'), completed_length),
					upload_length = COALESCE(NULLIF($7, '0'), upload_length),
					source_uri = COALESCE(NULLIF($8, ''), source_uri),
					info_hash = COALESCE(NULLIF($9, ''), info_hash),
					content_hash = COALESCE(NULLIF($10, ''), content_hash),
					num_pieces = COALESCE(NULLIF($11, '0'), num_pieces),
					piece_length = COALESCE(NULLIF($12, '0'), piece_length),
					is_resume_supported = CASE WHEN is_resume_supported = 'true' THEN 'true' ELSE COALESCE($13, is_resume_supported) END,
					verified_length = COALESCE(NULLIF($14, '0'), verified_length),
					torrent = COALESCE($15, torrent),
					options = COALESCE($16, options),
					seeder = $17,
					error_code = $18,
					error_message = $19,
					verify_integrity_pending = $20,
					completed_at = CASE
						WHEN $2 = 'complete' OR
							 $2 = 'seeding'
							 THEN COALESCE(completed_at, CURRENT_TIMESTAMP)
						ELSE NULL
					END,
					updated_at = CURRENT_TIMESTAMP
				WHERE gid = $21
				RETURNING
					user_id,
					name,
					source_uri,
					created_at,
					completed_at,
					kind as "kind: Kind",
					category as "category: Category"
            "#,
            meta.name,
            meta.status,
            meta.dir,
            meta.files,
            meta.total_length,
            meta.completed_length,
            meta.upload_length,
            meta.source_uri,
            meta.info_hash,
			meta.content_hash,
			meta.num_pieces,
			meta.piece_length,
			meta.is_resume_supported,
			meta.verified_length,
			meta.torrent,
			meta.options,
			meta.seeder,
			meta.error_code,
			meta.error_message,
			meta.verify_integrity_pending,
            meta.gid
        )
        .fetch_optional(&state.db)
        .await;

        match res {
            Ok(Some(row)) => {
				meta.user_id = row.user_id;
                meta.created_at = row.created_at;
                meta.completed_at = row.completed_at;
                meta.kind = row.kind;

                if meta.category.is_none() { meta.category = row.category }
                if meta.name.is_none() { meta.name = row.name; }
                if meta.source_uri.is_none() { meta.source_uri = row.source_uri; }

				/* Insert into or remove from `ActiveTasks` based on status */
                if meta.is_streaming() {
                    state.active_tasks.insert(meta.gid.clone(), meta.clone()).await;
                } else {
                    state.active_tasks.remove(&meta.gid).await;
                }

                /* Send on global channel */
                let msg = DlWsMessage::Event {
                    user_id: meta.user_id.clone(),
                    data: vec![meta],
                };

                let _ = state.history_tx.send(msg);
            },
            Ok(None) => {
                debug!("Received aria2 update for unknown gid: {:?}, download: {:?} weird error", meta.gid, meta.name);
            },
            Err(e) => error!("Database update failed: {}", e),
        }
    }

    /// Manually marks a gid as some 'status'
	/// Because aria2 doesn't emits evets for some status
    pub async fn handle_event(
        state: &AppState,
        gids: &[String],
        status: Option<GidStatus>
    ) {
		if gids.is_empty() {
			return;
		}
		/*
		  *  Terminal & manual events must evict
				immediately to stop stale tick & progress updates
		*/
		if !matches!(status, Some(GidStatus::Active) | Some(GidStatus::Seeding)) {
			for gid in gids {
				// Remove from active task & progress updater
				state.active_tasks.remove(gid).await;
				state.progress_buf.remove(gid).await;
			}
		}

		let status_str = status.as_ref().map(|s| match s {
			GidStatus::Paused => "paused",
			GidStatus::Waiting => "waiting",
			GidStatus::Stopped => "stopped",
			GidStatus::Active => "active",
			GidStatus::Complete => "complete",
			GidStatus::Error => "error",
			GidStatus::Removed => "removed",
			GidStatus::Seeding => "seeding",
		});

		for chunk in gids.chunks(MAX_MULTICALL_BATCH) {
			let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
        	    "UPDATE download_history SET status = COALESCE("
			);
        	qb.push_bind(status_str);
        	qb.push(", status), updated_at = CURRENT_TIMESTAMP WHERE gid IN (");

        	let mut separated = qb.separated(", ");

        	for gid in chunk {
				separated.push_bind(gid);
			}

			separated.push_unseparated( ") RETURNING
				gid,
				name,
				kind,
				category,
				status,
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
				torrent,
				options,
				is_resume_supported,
				num_pieces,
				piece_length,
				verified_length,
				verify_integrity_pending,
				created_at,
				completed_at,
				user_id"
			);

			match qb.build_query_as::<ItemMetaData>().fetch_all(&state.db).await {
				Ok(updated_rows) => {
					let mut by_user: HashMap<String, Vec<ItemMetaData>> = HashMap::new();

					for meta in updated_rows {
						let gid = meta.gid.clone();

						// Sync active tasks
						if matches!(status, Some(GidStatus::Active) | Some(GidStatus::Seeding)) {
							state.active_tasks.insert(gid.clone(), meta.clone()).await;
						} else {
							state.active_tasks.remove(&gid).await;
							state.progress_buf.remove(&gid).await;
						}

						by_user.entry(meta.user_id.clone()).or_default().push(meta);
					}

					for (user_id, items) in by_user {
						let msg = DlWsMessage::Event{
							user_id,
							data: items,
						};

						let _ = state.history_tx.send(msg);
					}

				}
				Err(e) => {
					error!("Failed to batch update download status: {}", e);
				}
			}
		}
    }

    /// Handles gid linking for parent -><- child transitions
    pub async fn handle_inheritance(
        state: &AppState,
        current_gid: &str,
        info: &Aria2Res
    ) {
		let info = serde_json::to_value(info).unwrap_or_default();
        if let Some(children) = info.get("followedBy").and_then(|v| v.as_array()) {
            for child in children {
                if let Some(child_gid) = child.as_str() {
                    let res = sqlx::query!(
                        r#"
                            INSERT INTO download_history (
                                gid,
                                user_id,
                                kind,
								category,
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
								category,
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
                            info!("Forward inheritance: Parent {:?} spawned child {:?}", current_gid, child_gid);
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
						category,
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
						category,
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
                    info!("Reverse inheritance: Child {:?} copied context from parent {:?}", current_gid, parent_gid);
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

	#[allow(unused)]
	async fn bootstrap_gid_options(state: &AppState) {
        let records = sqlx::query!(
            r#"
				SELECT
					gid,
					options
				FROM download_history
				WHERE status IN (
					'error',
					'paused',
					'active',
					'waiting',
					'removed',
					'stopped',
					'seeding',
					'complete'
            )
            AND options IS NOT NULL AND options != '{}'
            "#
        )
        .fetch_all(&state.db)
        .await
		.unwrap_or_else(|err| {
			error!("Failed to query to bootstrap gid options: {:?}", err);
			Vec::new()
		});

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

	async fn restore_dl(state: &AppState) {
        info!("Restoring interrupted downloads into Aria2...");
        // Fetch  active, waiting, paused, seeding  downloads, including kind and info_hash
		let records: Vec<ItemMetaData> = sqlx::query_as!(
			ItemMetaData,
			r#"
				SELECT
					gid,
					name,
					kind as "kind: Kind",
					category as "category: Category",
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
					torrent,
					options,
					is_resume_supported,
					num_pieces,
					piece_length,
					verified_length,
					verify_integrity_pending,
					created_at,
					completed_at,
					user_id
				FROM download_history
				WHERE status IN ('active', 'waiting', 'paused', 'seeding')
			"#
		).fetch_all(&state.db).await.unwrap_or_else(|e| {
			error!("Failed to query to restore downloads: {:?}", e);
			Vec::new()
		});

        if records.is_empty() {
            info!("No interrupted downloads to restore.");
            return;
        }

        let mut calls: Vec<Value> = Vec::new();
		let mut unresumable_gids: Vec<String> = Vec::new();

        for row in records {
			if let Some(is_resume_supported) = row.is_resume_supported_bool() {
				if is_resume_supported == false {
					unresumable_gids.push(row.gid.clone());
					continue;
				}
			}

            let mut opts = if let Some(opts_str) = row.options.as_ref() {
                serde_json::from_str::<serde_json::Map<String, Value>>(&opts_str).unwrap_or_default()
            } else {
                serde_json::Map::new()
            };

            // Force the GID to match the DB
            opts.insert("gid".to_string(), Value::String(row.gid.clone()));

			if row.status == GidStatus::Paused {
				opts.insert("pause".to_string(), Value::String("true".to_string()));
			}

			if row.status == GidStatus::Seeding {
				opts.insert("check-integrity".to_string(), Value::String("true".to_string()));
			}

			let mut staged_for_restore = false;

            if row.kind == Kind::Torrent {
				if let Some(uri) = &row.source_uri {
					if let Ok(u) = Url::parse(&uri) {
						if u.scheme() == "magnet" {
							calls.push(serde_json::json!({
								"methodName": "aria2.addUri",
								"params": [vec![uri], opts]
							}));

							staged_for_restore = true;
						}
					}
				}

				if !staged_for_restore {
					// `user_id-info_hash.torrent` format
					// Most likely there would be a info hash,
					// If it panics, it panics
					let torrent_path = state.config.data_dir
							.join("torrents")
							.join(format!("{}-{}.torrent", row.user_id, row.info_hash.clone().unwrap()));

					if torrent_path.exists() {
						if let Ok(bytes) = tokio::fs::read(&torrent_path).await {
							let b64 = STANDARD.encode(&bytes);
							calls.push(serde_json::json!({
								"methodName": "aria2.addTorrent",
								"params": [b64, Vec::<String>::new(), opts]
							}));

						staged_for_restore = true;

						} else {
							error!("Found torrent file but lack read permissions: {:?}", torrent_path);
						}
					} else {
						warn!("Torrent file missing on disk for GID {:?}. Cannot restore.", row.gid);
					}
				}
            } else {
				// URI restoration
                if let Some(uri) = row.source_uri.clone() {
                    calls.push(serde_json::json!({
                        "methodName": "aria2.addUri",
                        "params": [vec![uri], opts]
                    }));

					staged_for_restore = true;

                } else {
                    warn!("Can't restore URI for GID {:?}: Missing `source_uri`", row.gid);
                }
            }

            if staged_for_restore {
                if row.is_streaming() {
                    state.active_tasks.insert(row.gid.clone(), row.clone()).await;
					debug!("Restoring interrupted DLs.");
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
                            error_message = 'Server does not support resume.'
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

    /// Watch aria2 dir for deleted and moved files and mark matching history entries as 'removed'.
    /// For each filesystem event (remove or rename-from), look for entries in `download_history`
    /// That match filename & dir and call handle_event to set status to 'removed'.
	/// This watcher can only watch the global aria2 download dir,
	/// Gid with specified `dir` options can't be watched, that'd be tough
	pub async fn watch_aria2_dir(state: AppState) {
		let watch_dir = state.config.aria2_dir.clone();
		if !watch_dir.exists() {
			warn!("Aria2 download directory does not exist, skipping directory watcher: {:?}", watch_dir);
			return;
		}

		let (tx, mut rx) = tokio::sync::mpsc::channel(100);

		let mut debouncer = match new_debouncer(Duration::from_millis(500), None, move |res| {
			if let Err(e) = tx.blocking_send(res) {
				error!("Failed to forward filesystem events to channel: {}", e);
			}
		}) {
			Ok(d) => d,
			Err(e) => {
				error!("Failed to create file system debouncer: {}", e);
				return;
			}
		};

        if let Err(e) = debouncer.watch(&watch_dir, RecursiveMode::NonRecursive) {
            error!("Failed to watch aria2_dir {:?}: {}", watch_dir, e);
            return;
        }

		info!("Aria2 download directory watcher alive!!");

		let watch_dir_cln: PathBuf = watch_dir.clone();

		// Spawn on another thread
		tokio::spawn(async move {
			let _keep_alive = debouncer;
			while let Some(result) = rx.recv().await {
				match result {
					Ok(events) => {
						for debounced in events {
							let event = debounced.event;

							let relevant = matches!(
								event.kind,
								EventKind::Remove(_)
								| EventKind::Modify(ModifyKind::Name(RenameMode::From))
								| EventKind::Modify(ModifyKind::Name(RenameMode::Both))
							);

							if !relevant {
								continue;
							}

							for path in event.paths {
								let state_clone = state.clone();
								let watch_dir = watch_dir_cln.clone();
								// Since already in an async loop, use tokio::spawn directly
								// without acquiring the current runtime handle manually!!
								tokio::spawn(async move {
									let dir_str = watch_dir.to_string_lossy().to_string();
									let Some(fname) = path.file_name().and_then(|s| s.to_str()) else {
										return;
									};

									match sqlx::query!(
										r#"
											SELECT gid
											FROM download_history
											WHERE name = ? AND dir = ? AND status != 'removed'
										"#,
										fname,
										dir_str
									).fetch_all(&state_clone.db).await
										{
											Ok(rows) => {
												for row in rows {
													let _ = &state_clone.aria2.call("aria2.forceRemove", vec![serde_json::json!(&row.gid)]).await;
													sleep(Duration::from_millis(500)).await;
													Self::handle_event(&state_clone, slice::from_ref(&row.gid), Some(GidStatus::Removed)).await;
												}
											}
											Err(e) => {
												error!(
													"Failed to query download_history for removed, moved file '{}': {}",
													fname, e
												);
											}
										}
								});
							}
						}
					}
					Err(errors) => {
						for e in errors {
							error!("Directory watcher event error: {}", e);
						}
					}
				}
			}
		});
	}
}

