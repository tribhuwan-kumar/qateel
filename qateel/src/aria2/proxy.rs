use axum::{
    extract::Json,
    http::StatusCode,
    extract::{State, Query},
    response::{IntoResponse},
};
use std::path::Path;
use sha1::{Digest, Sha1};
use tokio::process::Command;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::collections::HashMap;
use sqlx::{Sqlite, QueryBuilder};
use tokio::time::{sleep, Duration};
use futures::stream::{self, StreamExt};
use super::types::{
    Kind,
    GidReq,
    MoveReq,
    StopReq,
	UriMeta,
	Category,
    PauseReq,
    RetryReq,
    GidStatus,
    ResumeReq,
    AddUriReq,
	UriInfoReq,
	UriInfoRes,
	SearchQuery,
	TorrentMeta,
	CheckNameRes,
    ItemMetaData,
	ChangeUriReq,
    Aria2Options,
    RemoveHisReq,
    SetOptionReq,
	CheckNameReq,
	TorsProbeReq,
	TorsProbeRes,
	ParsedTorrent,
	PendingAddUri,
    PaginationQuery,
    SetCmdOptionsReq,
	DuplicateConflict,
	ItemCoundStatsRes,
    TorrentMagnetMeta,
    BatchAddTorrentReq,
    SetGlobalOptionReq,
};

use tracing::{info, debug, error, warn};
use file_format::{FileFormat, Kind as FileKind};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use crate::{
    utils,
    his::probe,
    app::AppState,
    his::probe::UNTITLED,
	his::extract::Extraction,
    his::service::HistoryService,
    auth::types::User as AuthenticatedUser,
	aria2::daemon::RESTRICTED_ARIA2_KEYS,
};

pub const MAX_MULTICALL_BATCH: usize = 20;

/* All return json is in `camelCase`  */

/// Add all most all types of URIs
pub async fn add_uris(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<AddUriReq>,
) -> impl IntoResponse {
    let mut results: Vec<Value> = Vec::new();
	let mut queued_calls: Vec<(PendingAddUri, Value)> = Vec::new();

    for (i, task) in payload.tasks.into_iter().enumerate() {
        let mut opt = task.options.unwrap_or_default();
        let info_hash = Extraction::extract_mgn_info_hash(&task.uri);
        let is_magnet = info_hash.is_some();

        let kind = match url::Url::parse(&task.uri) {
            Ok(u) => match u.scheme().to_ascii_lowercase().as_str() {
                "magnet" => Kind::Torrent,
                "http" | "https" | "ftp" | "sftp" => Kind::Uri,
                _ => Kind::Unknown,
            },
            Err(_) => Kind::Unknown,
        };

        if kind == Kind::Unknown {
            results.push(serde_json::json!({
                "index": i,
                "error": "Unsupported or invalid URL format."
            }));
            continue;
        }

        let gid = utils::generate_gid();
        opt.insert("gid".to_string(), serde_json::json!(gid));

        // Inject the pre-probed file name so aria2 uses it
        if !is_magnet && !task.name.is_empty() {
            opt.insert("out".to_string(), serde_json::json!(task.name.clone()));
        }

		let category = Category::determine(Some(task.name.as_str()), &kind.as_str());

        // Store the values we'll need for the db insertion later
        let pending = PendingAddUri {
            index: i,
            kind: kind.clone(),
			category: category,
            name: task.name.clone(),
            source_uri: task.uri.clone(),
            total_length: task.total_length.clone(),
            is_resume_supported: task.is_resume_supported.clone(),
        };

        if is_magnet {
			if let Some(hash_str) = info_hash.as_deref() {
				let torrent_path = utils::get_torrent_path(&state.config.data_dir, user.user_id.clone(), &hash_str);
				// Directly inject the .torrent file downloaded during `probe_uris_info`
				if torrent_path.exists() {
					if let Ok(file_contents) = tokio::fs::read(&torrent_path).await {
						let b64 = STANDARD.encode(&file_contents);
						queued_calls.push((
							pending,
							serde_json::json!({
								"methodName": "aria2.addTorrent",
								"params": [
									b64,
									[],                                /* Webseeding URIs, keeping it empty till now */
									opt
								]
							})));

						continue;
					}
				}
			}
        }

        queued_calls.push((
            pending,
            serde_json::json!({
                "methodName": "aria2.addUri",
                "params": [[task.uri], opt]
            }),
        ));
    }

    if queued_calls.is_empty() {
        return (StatusCode::OK, Json(serde_json::json!({ "data": results })));
    }

    let mut batch_error: Option<String> = None;

    for chunk in queued_calls.chunks(MAX_MULTICALL_BATCH) {
        let rpc_batch = chunk
            .iter()
            .map(|(_, call)| call.clone())
            .collect::<Vec<Value>>();

	match state.aria2.call("system.multicall", vec![serde_json::json!(rpc_batch)]).await {
		Ok(gids_resp) => {
			if let Some(arr) = gids_resp.as_array() {
				for (local_idx, res) in arr.iter().enumerate() {
					let (pending, _) = &chunk[local_idx];
					let original_index = pending.index;

					if let Some(returned_gid) = res.as_array().and_then(|g| g.first()).and_then(|v| v.as_str()) {
						if let Err(e) = sqlx::query!(
							r#"
								INSERT INTO download_history
								(gid, user_id, kind, category, status, name, source_uri, total_length, is_resume_supported)
								VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
							"#,
							returned_gid,
							user.user_id,
							pending.kind,
							pending.category,
							"waiting",
							pending.name,
							pending.source_uri,
							pending.total_length,
							pending.is_resume_supported
						).execute(&state.db).await {
							error!("Failed to register record for GID {:?}: {:?}", returned_gid, e);
						}

						let state_clone = state.clone();
						let gid_clone = returned_gid.to_string();

						tokio::spawn(async move {
							if let Ok(opts_val) = state_clone.aria2.call(
								"aria2.getOption",
								vec![serde_json::json!(gid_clone)],
							).await {
								let opts_str = serde_json::to_string(&opts_val).unwrap_or_default();
								let _ = sqlx::query!(
									"
									UPDATE download_history
									SET options = ?
									WHERE gid = ?",
									opts_str,
									gid_clone
								)
								.execute(&state_clone.db)
								.await;
							}

							HistoryService::refresh_gid(&state_clone, gid_clone).await;
						});

						results.push(serde_json::json!({
							"index": original_index,
							"gid": returned_gid
						}));

					} else if let Some(err_obj) = res.as_object() {
						// Error format: { "code": 1, "message": "something" }
						let code_val = err_obj.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
						let msg = err_obj.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown error");

						results.push(serde_json::json!({
							"index": original_index,
							"code": code_val,
							"error": msg
						}));
					}
				}
			}
		}
		Err(e) => {
			batch_error = Some(e);
			break;
		}
	}
    }

    results.sort_by_key(|v| v.get("index").and_then(|i| i.as_u64()).unwrap_or(0));

    if let Some(err) = batch_error {
        return (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "error": err,
                "data": results
            })),
        );
    }

    (StatusCode::OK, Json(serde_json::json!({ "data": results })))
}


/// Get preflight info for almost all types of uris
pub async fn probe_uris_info(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<UriInfoReq>,
) -> impl IntoResponse {
    if payload.uris.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "No URIs provided." })));
    }

    let total_count = payload.uris.len();
    let user_id = user.user_id.clone();

    let mut magnet_indices = Vec::new();
    let mut magnet_probe_items = Vec::new();

    let mut http_indices = Vec::new();
    let mut http_uris = Vec::new();

    // Pre allocate slot array for indexed results
    let mut intermediate_results: Vec<Option<(
		String,						// uri
		UriMeta,					// meta
		Option<String>,				// probe error
		Option<String>,				// info_hash,
		bool,						// is_magnet,
		bool,						// is_ftp,
		bool						// is_sftp, no creating struct this time
	)>> = vec![None; total_count];

    // Categorize URIs
    for (idx, uri) in payload.uris.iter().enumerate() {
        let uri_lower = uri.to_ascii_lowercase();
        let is_magnet = uri_lower.starts_with("magnet:");
        let is_ftp = uri_lower.starts_with("ftp://");
        let is_sftp = uri_lower.starts_with("sftp://");
        let is_http = uri_lower.starts_with("http://") || uri_lower.starts_with("https://");

        if is_magnet {
            let info_hash = Extraction::extract_mgn_info_hash(uri);
            magnet_indices.push(idx);
            magnet_probe_items.push(probe::MagnetProbeItem {
                uri: uri.clone(),
                info_hash,
            });
        } else if is_http {
            http_indices.push(idx);
            http_uris.push(uri.clone());
        } else if is_ftp || is_sftp {
            let name = Extraction::extract_url_filename(uri).unwrap_or_else(|| uri.clone());
            let meta = UriMeta {
                name,
                total_length: "0".to_string(),
                resumable: true,
            };
            let err = Some("Authentication required, set credentials in options.".to_string());
            intermediate_results[idx] = Some((uri.clone(), meta, err, None, false, is_ftp, is_sftp));
        } else {
            let meta = UriMeta {
                name: uri.clone(),
                total_length: "0".to_string(),
                resumable: false,
            };
            let err = Some("Unsupported or invalid URL format".to_string());
            intermediate_results[idx] = Some((uri.clone(), meta, err, None, false, false, false));
        }
    }

    // Execute magnet and HTTP probes concurrently with bounded streams
    let (magnet_results, http_results) = tokio::join!(
        probe::probe_magnets(&state, &user_id, magnet_probe_items),
        probe::probe_http_uris(http_uris)
    );

    // Merge magnet results into slots
    for (m_idx, res) in magnet_indices.into_iter().zip(magnet_results) {
        let uri = &payload.uris[m_idx];
        let info_hash = Extraction::extract_mgn_info_hash(uri);
        match res {
            Ok(meta) => {
				intermediate_results[m_idx] = Some((uri.clone(), meta, None, info_hash, true, false, false))
			},
            Err((err, meta)) => {
				intermediate_results[m_idx] = Some((uri.clone(), meta, Some(err), info_hash, true, false, false))
			}
        }
    }

    // Merge http results into slots
    for (h_idx, res) in http_indices.into_iter().zip(http_results) {
        let uri = &payload.uris[h_idx];
        match res {
            Ok(meta) => intermediate_results[h_idx] = {
				Some((uri.clone(), meta, None, None, false, false, false))
			},
            Err((err, meta)) => intermediate_results[h_idx] = {
				Some((uri.clone(), meta, Some(err), None, false, false, false))
			},
        }
    }

    // Bounded duplicate checking against db
    let mut duplicate_checks = vec![None; total_count];
	const MAX_CONCURRENT_DB_CHECKS: usize = 16;

    let db_stream = stream::iter(intermediate_results.clone().into_iter().enumerate().map(|(idx, item_opt)| {
        let state_ref = state.clone();
        async move {
            let res = if let Some((uri, meta, _, info_hash, _, _, _)) = item_opt {
                find_duplicate(
                    &state_ref,
                    uri.as_str(),
                    &meta.name,
                    &meta.total_length,
                    info_hash.as_deref(),
                )
                .await
            } else {
                None
            };
            (idx, res)
        }
    }))
    .buffer_unordered(MAX_CONCURRENT_DB_CHECKS);

    tokio::pin!(db_stream);

    while let Some((idx, dup_res)) = db_stream.next().await {
        duplicate_checks[idx] = dup_res;
    }

    // Construct final response array maintaining original order
    let mut final_results: Vec<UriInfoRes> = Vec::with_capacity(total_count);

    for (item_opt, duplicate_check) in intermediate_results.into_iter().zip(duplicate_checks) {
        if let Some((uri, meta, probe_error, _info_hash, is_magnet, is_ftp, is_sftp)) = item_opt {
			let conflict_type = if let Some(conflict) = duplicate_check.as_ref() {
				Some(conflict.conflict_type.clone())
			} else {
				None
			};

            final_results.push(UriInfoRes {
                uri,
                name: meta.name,
                total_length: meta.total_length,
                is_resume_supported: meta.resumable.to_string(),
                is_magnet,
                is_ftp,
                is_sftp,
                is_duplicate: conflict_type.is_some(),
				conflict: duplicate_check,
                probe_error,
            });
        }
    }

    // Offline check fallback
    let is_completely_offline = final_results
        .iter()
        .any(|r| r.probe_error.as_deref() == Some("No internet connection."));

    if is_completely_offline {
        for item in &mut final_results {
            if item.is_magnet || item.is_ftp || item.is_sftp {
                item.probe_error = Some("No internet connection.".to_string());
            }
        }
    }

    (StatusCode::OK, Json(json!({ "data": final_results })))
}


/// Get duplication details
async fn find_duplicate(
    state: &AppState,
    uri: &str,
    name: &str,
    _total_length: &str,
    info_hash: Option<&str>
) -> Option<DuplicateConflict> {

    let records = sqlx::query!(
        r#"
            SELECT
				gid,
				dir,
				name,
				files,
				info_hash,
				source_uri,
				status as "status: GidStatus"
            FROM download_history
            WHERE (info_hash = $1 AND $1 IS NOT NULL)
               OR (source_uri = $2 AND $2 IS NOT NULL)
               OR (name = $3 AND $3 IS NOT NULL AND $3 != '' AND $3 != $4)
        "#,
        info_hash,
        uri,
        name,
		UNTITLED
    ).fetch_all(&state.db)
	.await.unwrap_or_default();

    for rec in records {
        let mut files_exist = false;

        if let Some(files_str) = &rec.files {
            if let Ok(paths) = serde_json::from_str::<Vec<String>>(files_str) {
                files_exist = paths.iter().any(|p| Path::new(p).exists());
            }
        }

        if !files_exist {
            if let (Some(dir), Some(rec_name)) = (&rec.dir, &rec.name) {
                files_exist = Path::new(dir).join(rec_name).exists();
            }
        }

		let is_uri_conflict = rec.source_uri.as_deref() == Some(uri);
        let is_hash_conflict = info_hash.is_some() && rec.info_hash.as_deref() == info_hash;
        let record_name = rec.name.unwrap_or_else(|| UNTITLED.into());

        if is_hash_conflict {
			let message = match (files_exist, &rec.status) {
				(true, GidStatus::Complete) => "Downloaded content from this BitTorrent info hash already exists.",
				(_, GidStatus::Complete) | (_, GidStatus::Removed) => {
					"Same BitTorrent info hash already exists in database but the downloaded content doesn't exist."
				},
				(_, GidStatus::Active) => "An active BitTorrent is already being downloaded from this info hash.",
				(_, GidStatus::Paused) => "There is a paused BitTorrent download from this info hash.",
				(_, GidStatus::Waiting) => "There is a waiting BitTorrent download from this info hash.",
				(_, GidStatus::Seeding) => "There is a BitTorrent already being seeded from this info hash.",
				(_, GidStatus::Stopped) => "There is a stopped BitTorrent download from this info hash.",
				(_, GidStatus::Error) => "There is an error BitTorrent download from this info hash.",
			}.to_string();

			return Some(DuplicateConflict {
				message,
                name: record_name,
				status: Some(rec.status),
				gid: Some(rec.gid),
                conflict_type: "hash".to_string(),
            });

        } else if is_uri_conflict {
			let message = match (files_exist, &rec.status) {
				(true, GidStatus::Complete) => "Downloaded content from this URI already exists.",
				(_, GidStatus::Complete) | (_, GidStatus::Removed) => {
					"Same URI exists in database but the downloaded content doesn't exist."
				},
				(_, GidStatus::Active) => "An active download is already being downloaded with this same URI.",
				(_, GidStatus::Paused) => "A paused download already exists.",
				(_, GidStatus::Waiting) => "There is a download from this URI in waiting queue.",
				(_, GidStatus::Stopped) => "There is a stopped download from this URI.",
				(_, GidStatus::Error) => "There is an error download from this source URI.",
				_ => "URI already exists.", // Handles Seeding / other edge cases safely
			}.to_string();

			return Some(DuplicateConflict {
				message,
                name: record_name,
				status: Some(rec.status),
				gid: Some(rec.gid),
                conflict_type: "uri".to_string(),
            });

        } else if files_exist {
			return Some(DuplicateConflict {
				name: record_name,
                gid: Some(rec.gid),
				status: Some(rec.status),
                conflict_type: "file".to_string(),
                message: "File with this name is already downloaded.".to_string(),
            });
        }
    }

    // Check untracked physical files on disk
    // If the DB query found nothing,
    //  * explicitly check if a file with this name is sitting in the aria2 directory.
	if !name.is_empty() && name != UNTITLED {
        let physical_path = Path::new(&state.config.aria2_dir).join(name);
        if physical_path.exists() {
            return Some(DuplicateConflict {
                gid: None,									// No GID because the database doesn't know about this file
				status: None,
                name: name.to_string(),
                conflict_type: "untrackedFile".to_string(),
                message: "An untracked file with this name already exists in the download directory.".to_string(),
            });
        }
    }

    None
}

/// Check if a newly typed name conflicts with db or Disk
pub async fn check_name_conflict(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(payload): Json<CheckNameReq>,
) -> impl IntoResponse {
    let name = payload.name.trim();
    if name.is_empty() || name == UNTITLED {
        return (StatusCode::OK, Json(CheckNameRes { is_conflict: false, msg: None }));
    }

    let physical_path = Path::new(&state.config.aria2_dir).join(name);

    // Check database
    let db_exists = sqlx::query!(
        "SELECT gid FROM download_history WHERE name = ? LIMIT 1",
        name
    ).fetch_optional(&state.db).await.unwrap_or_default();

    if db_exists.is_some() && physical_path.exists()  {
        return (StatusCode::OK, Json(CheckNameRes {
            is_conflict: true,
            msg: Some("File with this name already exists in database & disk.".to_string())
        }));
    }

    // Check disk
    if physical_path.exists() {
        return (StatusCode::OK, Json(CheckNameRes {
            is_conflict: true,
            msg: Some("File with this name already exists on disk.".to_string())
        }));
    }

	(StatusCode::OK, Json(CheckNameRes { is_conflict: false, msg: None }))
}


/// Specially for adding torrent files
/// Accepts torrents file base64 txt
pub async fn add_torrents(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<BatchAddTorrentReq>,
) -> impl IntoResponse {
    if payload.torrents.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "No torrents provided." })));
    }
    /*
      * Build the multicall params
      * Transform list of inputs into a list of aria2 method calls
    */
    let mut calls: Vec<Value> = Vec::new();
    let mut generated_gids: Vec<String> = Vec::new();
	let mut saved_file_paths: Vec<std::path::PathBuf> = Vec::new();

    for item in payload.torrents.into_iter() {
        let gid = utils::generate_gid();
        let mut opt = item.options.unwrap_or_else(|| serde_json::json!({}));

        if let Some(map) = opt.as_object_mut() {
            map.insert("gid".to_string(), serde_json::json!(gid));
        }

		let torrent_bytes = match STANDARD.decode(&item.torrent) {
			Ok(bytes) => bytes,
			Err(e) => {
				error!("Failed to decode Base64 torrent string: {:?}", e);
				for path in saved_file_paths {
					let _ = tokio::fs::remove_file(path).await;
				}
				return (StatusCode::BAD_REQUEST, Json(json!(
							{ "error": format!("Invalid Base64 payload: {}", e) })));
			}
		};

		let file_name = format!("{}-{}.torrent", user.user_id, gid);
		let torrent_path = state.config.data_dir.join("torrents").join(&file_name);

		if let Some(parent_dir) = torrent_path.parent() {
			if let Err(e) = tokio::fs::create_dir_all(parent_dir).await {
				error!("Failed to create torrents directory: {:?}", e);
				for path in saved_file_paths {
					let _ = tokio::fs::remove_file(path).await;
				}
				return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(
							{ "error": "Failed to initialize server storage directory." })));
			}
		}

		if let Err(e) = tokio::fs::write(&torrent_path, torrent_bytes).await {
			error!("Failed to write torrent file to disk: {:?}", e);
			for path in saved_file_paths {
				let _ = tokio::fs::remove_file(path).await;
			}
			return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(
						{ "error": "Failed to persist torrent data to server storage." })));
		}

		saved_file_paths.push(torrent_path.clone());

        if let Err(e) = sqlx::query!(
            r#"
                INSERT INTO download_history (gid, user_id, kind, status)
                VALUES (?, ?, 'torrent', 'waiting')
            "#,
            gid,
            user.user_id
        ).execute(&state.db).await{
            error!("Failed to register record for GID {:?}: {:?}", gid, e);
			for path in saved_file_paths {
				let _ = tokio::fs::remove_file(path).await;
			}
			return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(
						{ "error": "Database tracking registration failed." })));
		};

        generated_gids.push(gid.clone());

        calls.push(json!({
            "methodName": "aria2.addTorrent",
            "params": [
                item.torrent,
                [],                 /* Webseeding URIs, keeping it empty till now */
                opt
            ]
        }));
    }

    /*
      * Call system.multicall
      * Params for multicall is an array containing the array of calls: [ [call1, call2] ]
    */
    match state.aria2.call("system.multicall", vec![json!(calls)]).await {
        Ok(gids_resp) => {
            let mut results = Vec::new();

            if let Some(arr) = gids_resp.as_array() {
                for (i, res) in arr.iter().enumerate() {

                    let expected_gid = &generated_gids[i];
					let file_to_keep = &saved_file_paths[i];

                    if let Some(returned_gid) = res.as_array().and_then(|g| g.first()).and_then(|v| v.as_str()) {
                        let state_clone = state.clone();
                        let gid_clone = returned_gid.to_string();

                        tokio::spawn(async move {
                            if let Ok(opts_val) = state_clone.aria2.call("aria2.getOption", vec![json!(gid_clone)]).await {
                                let opts_str = serde_json::to_string(&opts_val).unwrap_or_default();
								let result = sqlx::query!(
									"UPDATE download_history SET options = ? WHERE gid = ?",
									opts_str,
									gid_clone
								)
								.execute(&state_clone.db)
								.await;

								if let Err(e) = result {
									error!("Failed to update options in database for GID {:?}: {:?}", gid_clone, e);
								}
                            }
                            HistoryService::refresh_gid(&state_clone, gid_clone).await;
                        });
                        results.push(json!({
                            "index": i,
                            "gid": returned_gid,
                        }));
                    }
                    else if let Some(err_obj) = res.as_object() {
                        let borrow_j = &json!(0);
                        let code = err_obj.get("code").unwrap_or(borrow_j);
                        let msg = err_obj.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                        let _ = sqlx::query!(
                            "DELETE FROM download_history WHERE gid = ?",
                            expected_gid
                        ).execute(&state.db).await;

						// Also delete its companion file from the filesystem
                        let _ = tokio::fs::remove_file(file_to_keep).await;

                        results.push(json!({
                            "index": i,
                            "code": code,
                            "error": msg,
                        }));
                    }
                    else {
                        results.push(json!({
							"index": i,
							"error": "Aria2 returned invalid response format"
						}));
                    }
                }
            }
            (StatusCode::OK, Json(json!({ "data": results })))
        }
        Err(e) => {
            error!("Failed to batch torrents: {}", e);
            for (i, gid) in generated_gids.iter().enumerate() {
                let _ = sqlx::query!(
                    "DELETE FROM download_history WHERE gid = ?",
                    gid
                ).execute(&state.db).await;
				let _ = tokio::fs::remove_file(&saved_file_paths[i]).await;
            }
            (StatusCode::BAD_GATEWAY, Json(json!({ "error": e })))
        }
    }
}


/// Parse a torrent file
pub fn parse_torrent_bytes(bytes: &[u8]) -> Result<ParsedTorrent, String> {
    let meta: TorrentMeta = serde_bencode::from_bytes(bytes)
        .map_err(|e| format!("Failed to parse torrents base64: {}", e))?;

    // Parse into generic value to preserve
	// All info keys for exact `SHA-1` hashing
    let value: serde_bencode::value::Value = serde_bencode::from_bytes(bytes)
        .map_err(|e| format!("Failed to parse raw torrents value: {}", e))?;

    let info_bytes = match value {
        serde_bencode::value::Value::Dict(map) => match map.get(&b"info"[..]) {
            Some(info_val) => serde_bencode::to_bytes(info_val)
                .map_err(|e| format!("Failed to re-encode info dict: {}", e))?,
            None => return Err("Missing 'info' dictionary in torrent file.".to_string()),
        },
        _ => return Err("Invalid torrent file structure.".to_string()),
    };

    let mut hasher = Sha1::new();
    hasher.update(&info_bytes);
    let info_hash = hex::encode(hasher.finalize());

    let name = meta.info.name.clone().unwrap_or_else(|| "Unknown".to_string());
    let total_size = meta.info.total_size();
    let is_private = meta.info.is_private();

    Ok(ParsedTorrent {
        name,
        total_size,
        is_private,
        info_hash,
    })
}

/// Get torrents info
pub async fn probe_torrents(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(payload): Json<TorsProbeReq>,
) -> impl IntoResponse {
    if payload.torrents.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No torrents provided to probe." })),
        );
    }

    let mut results = Vec::new();

    for (i, base64_str) in payload.torrents.into_iter().enumerate() {
        // Decode Base64
        let bytes = match STANDARD.decode(&base64_str) {
            Ok(b) => b,
            Err(e) => {
                results.push(TorsProbeRes {
                    index: i,
                    name: None,
                    info_hash: None,
                    total_length: None,
                    is_private: None,
					is_duplicate: None,
                    conflict: None,
                    probe_error: Some(format!("Invalid Base64 string: {}", e)),
                });
                continue;
            }
        };

        // Extract metadata & calculate info_hash
        let torrent = match parse_torrent_bytes(&bytes) {
            Ok(t) => t,
            Err(e) => {
                results.push(TorsProbeRes {
                    index: i,
                    name: None,
                    info_hash: None,
                    total_length: None,
                    is_private: None,
					is_duplicate: None,
                    conflict: None,
                    probe_error: Some(e),
                });
                continue;
            }
        };

        // Probe duplicate logic
        let conflict = find_duplicate(
            &state,
            "",								// Empty uri cause raw torrent file probes
            &torrent.name,
            &torrent.total_size.to_string(), // Its useless
            Some(&torrent.info_hash),
        )
        .await;

        results.push(TorsProbeRes {
            index: i,
            name: Some(torrent.name),
            info_hash: Some(torrent.info_hash),
            total_length: Some(torrent.total_size.to_string()),
            is_private: Some(torrent.is_private),
            is_duplicate: None,
            conflict,
            probe_error: None,
        });
    }

    (StatusCode::OK, Json(json!({ "data": results })))
}


/// ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.pause
/// aria2.pause
/// aria2.forcePause
// only `active` and `waiting` can be paused,
pub async fn pause(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(payload): Json<PauseReq>,
) -> impl IntoResponse {
    // Validate gids for the batch multicall
	let gids = payload.gids;

	if gids.is_empty(){
		return (
			StatusCode::BAD_REQUEST,
			Json(json!({ "error": "No GID provided to pause." })),
		).into_response();
	}

    let method = if payload.force_pause.unwrap_or(false) {
        "aria2.forcePause"
    } else {
        "aria2.pause"
    };

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for chunk in gids.chunks(MAX_MULTICALL_BATCH) {
        let calls: Vec<Value> = chunk
            .iter()
            .map(|gid| {
                json!({
                    "methodName": method,
                    "params": [gid]
                })
            })
            .collect();

        match state.aria2.call("system.multicall", vec![json!(calls)]).await {
            Ok(multicall_resp) => {
                let arr = match multicall_resp.as_array() {
                    Some(v) => v,
                    None => {
                        failed.extend(
                            chunk.iter() .map(|gid| json!({ "gid": gid, "error": "Invalid aria2 multicall response." })),
                        );
                        continue;
                    }
                };

                for (idx, res) in arr.iter().enumerate() {
                    let expected_gid = &chunk[idx];

                    let returned_gid = if let Some(v) = res.as_str() {
                        Some(v.to_string())
                    } else if let Some(list) = res.as_array() {
                        list.first()
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    };

                    if returned_gid.is_some() {

                        succeeded.push(expected_gid.clone());

                    } else if let Some(err_obj) = res.as_object() {
                        let code = err_obj
                            .get("code")
                            .cloned()
                            .unwrap_or_else(|| json!(0));
                        let msg = err_obj
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown error");

                        failed.push(json!({
                            "gid": expected_gid,
                            "code": code,
                            "error": msg
                        }));
                    } else {
                        failed.push(json!({
                            "gid": expected_gid,
                            "error": "Aria2 returned invalid response format."
                        }));
                    }
                }
            }
            Err(e) => {
                error!("`system.multicall` failed for {:?}: {:?}", method, &e);

                failed.extend(
                    chunk
                        .iter()
                        .map(|gid| json!({ "gid": gid, "error": e.to_string() })),
                );
            }
        }
    }

    info!("Batch `{}` executed via multicall: {:?} succeeded, {:?} failed", method, succeeded.len(), failed.len());

    if succeeded.is_empty() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "error": "Failed to pause specified downloads.",
                "method": method,
                "failed": failed
            })),
        ).into_response();
    }

	// `waiting` doesn't emits the notification event on being paused, had to it handle manually
	let succeeded_cln = succeeded.clone();
	tokio::spawn(async move {
		HistoryService::handle_event(&state, &succeeded_cln, Some(GidStatus::Paused)).await;
	});

    (
        StatusCode::OK,
        Json(json!({
            "status": "paused",
            "method": method,
            "succeeded": succeeded,
            "failed": failed
        })),
    ).into_response()
}

/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.forcePause
/// `aria2.forcePause`
pub async fn pause_all(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    // Collect gids from DB that are active,waiting
    // Adjust the column name if your app uses a different field name.
    let db_gids: HashSet<String> = sqlx::query_scalar!(
        r#"
        SELECT gid
        FROM download_history
        WHERE user_id = ?
          AND status IN ('active', 'waiting')
        "#,
        user.user_id
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })
    .into_iter()
    .flatten()
    .collect();

    // Collect gids from aria2 live session
    let mut live_gids: HashSet<String> = HashSet::new();

    let aria2_active = state.aria2.call("aria2.tellActive", vec![]).await;
	// Keep a large window for `tellWaiting`
    let aria2_waiting = state.aria2.call("aria2.tellWaiting", vec![json!(0), json!(99999)]).await;

    for response in [aria2_active, aria2_waiting] {
        let items = match response {
            Ok(value) => value,
            Err(_) => continue,
        };

        let arr = match items.as_array() {
            Some(v) => v,
            None => continue,
        };

        for item in arr {
            let gid = item
                .get("gid")
                .and_then(serde_json::Value::as_str)
                .map(|s| s.to_string());

			let is_not_paused = item
				.get("status")
				.and_then(serde_json::Value::as_str)
				.map(|s| s != "paused")
				.unwrap_or(false);

            if let Some(gid) = gid {
				if is_not_paused {
					live_gids.insert(gid);
				}
            }
        }
    }

    // Unique set: db + live aria2
    let gids: Vec<String> = db_gids
        .union(&live_gids)
        .cloned()
        .collect();

    if gids.is_empty() {
		return (
			StatusCode::BAD_REQUEST,
			Json(json!({ "error": "No active or waiting downloads to pause." })),
		).into_response();
    }


    let mut paused_gids: Vec<String> = Vec::new();
    let mut failed: Vec<serde_json::Value> = Vec::new();

    // Batch via `system.multicall`
    for chunk in gids.chunks(MAX_MULTICALL_BATCH) {
        let calls: Vec<serde_json::Value> = chunk
            .iter()
            .map(|gid| {
                serde_json::json!({
                    "methodName": "aria2.forcePause",
                    "params": [gid]
                })
            })
            .collect();

        match state.aria2.call("system.multicall", vec![serde_json::json!(calls)]).await {
            Ok(multicall_resp) => {
                let results = match multicall_resp.as_array() {
                    Some(v) => v,
                    None => {
                        for gid in chunk {
                            failed.push(serde_json::json!({
                                "gid": gid,
                                "error": "Invalid aria2 multicall response."
                            }));
                        }
                        continue;
                    }
                };

                for (idx, res) in results.iter().enumerate() {
                    let gid = match chunk.get(idx) {
                        Some(gid) => gid,
                        None => continue,
                    };

                    let success = res
                        .as_array()
                        .and_then(|v| v.first())
                        .and_then(serde_json::Value::as_str)
                        .is_some()
                        || res.as_str().is_some();

                    if success {

                        paused_gids.push(gid.clone());

                    } else if let Some(err_obj) = res.as_object() {
                        let code = err_obj
                            .get("code")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!(0));
                        let msg = err_obj
                            .get("message")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("Unknown error");

                        failed.push(serde_json::json!({
                            "gid": gid,
                            "code": code,
                            "error": msg
                        }));
                    } else {
                        failed.push(serde_json::json!({
                            "gid": gid,
                            "error": "Aria2 returned invalid response format."
                        }));
                    }
                }
            }
            Err(e) => {
                for gid in chunk {
					error!("Pasuing failed for gid: {:?}, error: {}", gid, e);
                    failed.push(serde_json::json!({
                        "gid": gid,
                        "error": e.to_string()
                    }));
                }
            }
        }
    }

    if paused_gids.is_empty() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "error": "Failed to pause all downloads.",
                "failed": failed
            })),
        ).into_response();
    }

    // Emit pause events only for successful gids
	let paused_cln = paused_gids.clone();
	tokio::spawn(async move {
		sleep(Duration::from_millis(250)).await;
		HistoryService::handle_event(&state, &paused_cln, Some(GidStatus::Paused)).await;
	});

    (
        StatusCode::OK,
        Json(json!({
            "status": "pausedAll",
            "succeeded": paused_gids,
            "failed": failed
        })),
    ).into_response()

}

/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.unpause
/// `aria2.unpause`
/// `paused` to `waiting`
pub async fn resume(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(payload): Json<ResumeReq>,
) -> impl IntoResponse {
    // Validate GIDs for the batch multicall
	let gids = payload.gids;
	if gids.is_empty(){
		return (
			StatusCode::BAD_REQUEST,
			Json(json!({ "error": "No GID provided to resume." })),
		).into_response();
	}

    let method = "aria2.unpause";
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for chunk in gids.chunks(MAX_MULTICALL_BATCH) {
		// Build the multicall params
        let calls: Vec<Value> = chunk
            .iter()
            .map(|gid| {
                json!({
                    "methodName": method,
                    "params": [gid]
                })
            })
            .collect();

        match state.aria2.call("system.multicall", vec![json!(calls)]).await {
            Ok(multicall_resp) => {
                if let Some(arr) = multicall_resp.as_array() {
                    for (i, res) in arr.iter().enumerate() {
                        let expected_gid = match chunk.get(i) {
                            Some(gid) => gid,
                            None => continue,
                        };

                        // Success format can be ["gid"]
                        let returned_gid = res
                            .as_array()
                            .and_then(|g| g.first())
                            .and_then(|v| v.as_str());

                        if returned_gid.is_some() {

                            succeeded.push(expected_gid.clone());

                        } else if let Some(err_obj) = res.as_object() {
                            let code = err_obj.get("code").cloned().unwrap_or_else(|| json!(0));
                            let msg = err_obj
                                .get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown error.");

                            failed.push(json!({
                                "gid": expected_gid,
                                "code": code,
                                "error": msg
                            }));
                        } else {
                            failed.push(json!({
                                "gid": expected_gid,
                                "error": "Aria2 returned invalid response format."
                            }));
                        }
                    }
                } else {
                    // Entire chunk failed due to invalid response shape
                    for gid in chunk {
                        failed.push(json!({
                            "gid": gid,
                            "error": "Aria2 returned invalid multicall response."
                        }));
                    }
                }
            }
            Err(e) => {
                error!("`system.multicall` failed for {:?}: {:?}", method, e);
                // Mark every gid in this chunk as failed
                for gid in chunk {
                    failed.push(json!({
                        "gid": gid,
                        "error": e.to_string()
                    }));
                }
            }
        }
    }

    info!("Batch {:?} executed via multicall: {:?} succeeded, {:?} failed", method, succeeded.len(), failed.len());

    if succeeded.is_empty() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "error": "Failed to resume specified downloads.",
                "method": method,
                "failed": failed
            })),
        ).into_response();
    }

	// History status update of 'resume' -> 'waiting'
	let succeeded_cln = succeeded.clone();
	tokio::spawn(async move {
		HistoryService::handle_event(&state, &succeeded_cln, Some(GidStatus::Waiting)).await;
	});

    (
        StatusCode::OK,
        Json(json!({
            "status": "resumed",
            "method": method,
            "succeeded": succeeded,
            "failed": failed
        })),
    ).into_response()
}

/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.forcePause
/// `aria2.unpause`
/// Only `paused` can be resumed
pub async fn resume_all(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    // Collect gids from DB that are active,waiting
    // Adjust the column name if your app uses a different field name.
    let db_gids: HashSet<String> = sqlx::query_scalar!(
        r#"
        SELECT gid
        FROM download_history
        WHERE user_id = ?
          AND status IN ('paused')
        "#,
        user.user_id
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })
    .into_iter()
    .flatten()
    .collect();

    let mut live_gids: HashSet<String> = HashSet::new();
	// `tellWaiting` returns `paused` gids too
    let aria2_waiting = state.aria2.call("aria2.tellWaiting", vec![json!(0), json!(99999)]).await;

	if let Ok(items) = aria2_waiting {
		if let Some(arr) = items.as_array() {
			for item in arr {
				let gid = item
					.get("gid")
					.and_then(serde_json::Value::as_str)
					.map(|s| s.to_string());

				let is_paused = item
					.get("status")
					.and_then(serde_json::Value::as_str)
					.map(|s| s == "paused")
					.unwrap_or(false);

				if let Some(gid) = gid {
					if is_paused {
						live_gids.insert(gid);
					}
				}
			}
		}
	}

    // Unique set: db + live aria2
    let gids: Vec<String> = db_gids
        .union(&live_gids)
        .cloned()
        .collect();

    if gids.is_empty() {
		return (
			StatusCode::BAD_REQUEST,
			Json(json!({ "error": "No paused downloads to resume." })),
		).into_response();
    }


    let mut resumed_gids: Vec<String> = Vec::new();
    let mut failed: Vec<serde_json::Value> = Vec::new();

    // Batch via `system.multicall`
    for chunk in gids.chunks(MAX_MULTICALL_BATCH) {
        let calls: Vec<serde_json::Value> = chunk
            .iter()
            .map(|gid| {
                serde_json::json!({
                    "methodName": "aria2.unpause",
                    "params": [gid]
                })
            })
            .collect();

        match state.aria2.call("system.multicall", vec![serde_json::json!(calls)]).await {
            Ok(multicall_resp) => {
                let results = match multicall_resp.as_array() {
                    Some(v) => v,
                    None => {
                        for gid in chunk {
                            failed.push(serde_json::json!({
                                "gid": gid,
                                "error": "Invalid aria2 multicall response."
                            }));
                        }
                        continue;
                    }
                };

                for (idx, res) in results.iter().enumerate() {
                    let gid = match chunk.get(idx) {
                        Some(gid) => gid,
                        None => continue,
                    };

                    let success = res
                        .as_array()
                        .and_then(|v| v.first())
                        .and_then(serde_json::Value::as_str)
                        .is_some()
                        || res.as_str().is_some();

                    if success {

                        resumed_gids.push(gid.clone());

                    } else if let Some(err_obj) = res.as_object() {
                        let code = err_obj
                            .get("code")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!(0));
                        let msg = err_obj
                            .get("message")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("Unknown error");

                        failed.push(serde_json::json!({
                            "gid": gid,
                            "code": code,
                            "error": msg
                        }));
                    } else {
                        failed.push(serde_json::json!({
                            "gid": gid,
                            "error": "Aria2 returned invalid response format."
                        }));
                    }
                }
            }
            Err(e) => {
                for gid in chunk {
                    failed.push(serde_json::json!({
                        "gid": gid,
                        "error": e.to_string()
                    }));
                }
            }
        }
    }

	let resumed_gids_cln = resumed_gids.clone();
	tokio::spawn(async move {
		// Mark as waiting
		HistoryService::handle_event(&state, &resumed_gids_cln, Some(GidStatus::Waiting)).await;
	});

    (
        StatusCode::OK,
        Json(json!({
            "status": "resumedAll",
            "succeeded": resumed_gids,
            "failed": failed
        })),
    ).into_response()

}



/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.remove
/// It'll remove the dl from the queue & free the memory by calling `aria2.removeDownloadResult`
/// Remove only from downloading queue, but don't delete the `file-allocation`
/// `aria2.remove`
/// `aria2.forceRemove`
pub async fn stop(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(payload): Json<StopReq>,
) -> impl IntoResponse {
	let gids = payload.gids;

	if gids.is_empty(){
		return (
			StatusCode::BAD_REQUEST,
			Json(json!({ "error": "No GID provided to stop." })),
		).into_response();
	}

    let method = if payload.force_stop.unwrap_or(false) {
        "aria2.forceRemove"
    } else {
        "aria2.remove"
    };

    let mut successful_gids = Vec::new();
    let mut failed = Vec::new();

    // `remove, forceRemove` in chunks
    for chunk in gids.chunks(MAX_MULTICALL_BATCH) {
        let calls: Vec<Value> = chunk
            .iter()
            .map(|gid| {
                json!({
                    "methodName": method,
                    "params": [gid]
                })
            })
            .collect();

        match state.aria2.call("system.multicall", vec![json!(calls)]).await {
            Ok(multicall_resp) => {
                let arr = match multicall_resp.as_array() {
                    Some(v) => v,
                    None => {
                        for gid in chunk {
                            failed.push(json!({
                                "gid": gid,
                                "error": "Aria2 returned invalid multicall response."
                            }));
                        }
                        continue;
                    }
                };

                for (idx, res) in arr.iter().enumerate() {
                    let expected_gid = match chunk.get(idx) {
                        Some(gid) => gid,
                        None => continue,
                    };

                    let removed_gid = res
                        .as_array()
                        .and_then(|g| g.first())
                        .and_then(|v| v.as_str())
                        .or_else(|| res.as_str());

                    if removed_gid.is_some() {

                        successful_gids.push(expected_gid.clone());

                    } else if let Some(err_obj) = res.as_object() {
                        let code = err_obj.get("code").cloned().unwrap_or_else(|| json!(0));
                        let msg = err_obj
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown error");

                        failed.push(json!({
                            "gid": expected_gid,
                            "code": code,
                            "error": msg
                        }));
                    } else {
                        failed.push(json!({
                            "gid": expected_gid,
                            "error": "Aria2 returned invalid response format"
                        }));
                    }
                }
            }
            Err(e) => {
                error!("Stopping failed for {} chunk: {}", method, e);
                for gid in chunk {
                    failed.push(json!({
                        "gid": gid,
                        "error": e.to_string()
                    }));
                }
            }
        }
    }

    if successful_gids.is_empty() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "error": "Failed to stop specified downloads.",
                "method": method,
                "failed": failed
            })),
        ).into_response();
    }

    // `aria2.removeDownloadResult` ONLY for successful GIDs, but ignore result
    for chunk in successful_gids.chunks(MAX_MULTICALL_BATCH) {
        let cleanup_calls: Vec<Value> = chunk
            .iter()
            .map(|gid| {
                json!({
                    "methodName": "aria2.removeDownloadResult",
                    "params": [gid]
                })
            })
            .collect();

        let _ = state
            .aria2
            .call("system.multicall", vec![json!(cleanup_calls)])
            .await;
    }

    // History update only for successful from `remove` or `forceRemove`
	let stopped_cln = successful_gids.clone();
	tokio::spawn(async move {
		HistoryService::handle_event(&state, &stopped_cln, Some(GidStatus::Stopped)).await;
	});

    (
        StatusCode::OK,
        Json(json!({
            "status": "stopped",
            "method": method,
            "succeeded": successful_gids,
            "failed": failed
        })),
    ).into_response()
}

/// Stop all `error`, `active`, `waiting`, `paused`. remove from aria2 dl queue
/// `aria2.forceRemove`
pub async fn stop_all(
    State(state): State<AppState>,
	user: AuthenticatedUser,
) -> impl IntoResponse {
    // Collect gids from DB that are active,waiting
    // Adjust the column name if your app uses a different field name.
    let db_gids: HashSet<String> = sqlx::query_scalar!(
        r#"
        SELECT gid
        FROM download_history
        WHERE user_id = ?
          AND status IN ('active', 'paused', 'error', 'waiting')
        "#,
        user.user_id
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })
    .into_iter()
    .flatten()
    .collect();

    let mut live_gids: HashSet<String> = HashSet::new();

    let aria2_active = state.aria2.call("aria2.tellActive", vec![]).await;
    let aria2_waiting = state.aria2.call("aria2.tellWaiting", vec![json!(0), json!(99999)]).await;
    let aria2_stopped = state.aria2.call("aria2.tellStopped", vec![json!(0), json!(99999)]).await;

    for response in [aria2_active, aria2_waiting, aria2_stopped] {
        let items = match response {
            Ok(value) => value,
            Err(_) => continue,
        };

        let arr = match items.as_array() {
            Some(v) => v,
            None => continue,
        };

        for item in arr {
            let gid = item
                .get("gid")
                .and_then(serde_json::Value::as_str)
                .map(|s| s.to_string());

            if let Some(gid) = gid {
				live_gids.insert(gid);
            }
        }
    }

    let gids: Vec<String> = db_gids
        .union(&live_gids)
        .cloned()
        .collect();

    if gids.is_empty() {
		return (
			StatusCode::BAD_REQUEST,
			Json(json!({ "error": "No active, paused, waiting, error downloads to stop." })),
		).into_response();
    }

	let mut failed = Vec::new();
    let mut successful_gids = Vec::new();

    // `remove, forceRemove` in chunks
    for chunk in gids.chunks(MAX_MULTICALL_BATCH) {
        let calls: Vec<Value> = chunk
            .iter()
            .map(|gid| {
                json!({
                    "methodName": "aria2.forceRemove",
                    "params": [gid]
                })
            })
            .collect();

        match state.aria2.call("system.multicall", vec![json!(calls)]).await {
            Ok(multicall_resp) => {
                let arr = match multicall_resp.as_array() {
                    Some(v) => v,
                    None => {
                        for gid in chunk {
                            failed.push(json!({
                                "gid": gid,
                                "error": "Aria2 returned invalid multicall response."
                            }));
                        }
                        continue;
                    }
                };

                for (idx, res) in arr.iter().enumerate() {
                    let expected_gid = match chunk.get(idx) {
                        Some(gid) => gid,
                        None => continue,
                    };

                    let removed_gid = res
                        .as_array()
                        .and_then(|g| g.first())
                        .and_then(|v| v.as_str())
                        .or_else(|| res.as_str());

                    if removed_gid.is_some() {

                        successful_gids.push(expected_gid.clone());

                    } else if let Some(err_obj) = res.as_object() {
                        let code = err_obj.get("code").cloned().unwrap_or_else(|| json!(0));
                        let msg = err_obj
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown error");
						/*
							if the aria2 error is something like this means this `GID e6147513de73a4a5 is not found`
							this gid doesn't found in aria2 so it should count as success cause its already has been removed
						*/
						if msg.to_lowercase().contains("is not found") {
							successful_gids.push(expected_gid.clone());
						} else {
							failed.push(json!({
								"gid": expected_gid,
								"code": code,
								"error": msg
							}));
						}
                    } else {
                        failed.push(json!({
                            "gid": expected_gid,
                            "error": "Aria2 returned invalid response format."
                        }));
                    }
                }
            }
            Err(e) => {
                for gid in chunk {
					error!("Stopping failed for gid: {:?}, error: {:?}", gid, e);
                    failed.push(json!({
                        "gid": gid,
                        "error": e.to_string()
                    }));
                }
            }
        }
    }

    if successful_gids.is_empty() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "error": "Failed to stop all downloads.",
                "failed": failed
            })),
        ).into_response();
    }

    // `aria2.removeDownloadResult` ONLY for successful GIDs, but ignore result
    for chunk in successful_gids.chunks(MAX_MULTICALL_BATCH) {
        let cleanup_calls: Vec<Value> = chunk
            .iter()
            .map(|gid| {
                json!({
                    "methodName": "aria2.removeDownloadResult",
                    "params": [gid]
                })
            })
            .collect();

        let _ = state.aria2.call("system.multicall", vec![json!(cleanup_calls)]).await;
    }

    // History update only for successful from `remove` or `forceRemove`
	let stopped_cln = successful_gids.clone();
	tokio::spawn(async move {
		HistoryService::handle_event(&state, &stopped_cln, Some(GidStatus::Stopped)).await;
	});

    (
        StatusCode::OK,
        Json(json!({
            "status": "stoppedAll",
            "succeeded": successful_gids,
            "failed": failed
        })),
    ).into_response()
}


/// Get details for a gid
pub async fn details(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(payload): Json<GidReq>,
) -> impl IntoResponse {
    let gid = payload.gid.clone();
    let history_item = match sqlx::query_as!(
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
				created_at as "created_at!",
				completed_at as "completed_at",
				user_id
			FROM download_history
			WHERE gid = ?
        "#,
        gid
    )
    .fetch_optional(&state.db)
    .await
    {
        Ok(item) => item,
        Err(e) => {
            error!("Failed to fetch download history for gid {}: {}", gid, e);
            None
        }
    };

    let is_torrent = history_item
        .as_ref()
        .map(|item| matches!(item.kind.clone(), Kind::Torrent)
            || item.torrent.is_some()
            || item.info_hash.is_some())
        .unwrap_or(false);

    let mut methods = vec![
        json!({ "methodName": "aria2.tellStatus", "params": [gid.clone()] }),
        json!({ "methodName": "aria2.getFiles", "params": [gid.clone()] }),
        json!({ "methodName": "aria2.getOption", "params": [gid.clone()] }),
    ];

    if is_torrent {
        methods.push(json!({ "methodName": "aria2.getPeers", "params": [gid.clone()] }));
    } else {
        /*
          * Specific to `uri`
        */
        methods.push(json!({ "methodName": "aria2.getUris", "params": [gid.clone()] }));
        methods.push(json!({ "methodName": "aria2.getServers", "params": [gid.clone()] }));
    }

    let params = vec![json!(methods)];

    match state.aria2.call("system.multicall", params).await {
        Ok(res) => {
            let is_empty = res.as_array().map(|arr| arr.is_empty()).unwrap_or(true);
            if is_empty {
                if let Some(item) = history_item {
                    return (
                        StatusCode::OK,
                        Json(json!({
                            "source": "database",
                            "data": item
                        })),
                    );
                }

                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "erorr": format!("GID {} not found", gid)
                    })),
                );
            }

            (
                StatusCode::OK,
                Json(json!({
                    "data": {
						"aria2Res": res,
						"history": history_item
					},
                    "source": "aria2",
                })),
            )
        }
        Err(e) => {
            if let Some(item) = history_item {
                error!("Aria2 details lookup failed for gid {:?}: {:?}, fetching data from db", gid, e);
                (
                    StatusCode::OK,
                    Json(json!({
						"data": item,
                        "source": "database",
                        "warning": e.to_string()
                    })),
                )
            } else {
                error!("Failed to fetch details for gid {:?}: {:?}", gid, e);
                (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({
                        "error": e.to_string()
                    })),
                )
            }
        }
    }
}

/// Retry a failed download on status `stopped`, `error`
/// If server `is_resume_supported`, download again `allow-overwrite=false` and `continue=true` with the download
/// If not download again with `allow-overwrite` & `continue=false` for uri
/// `check-integrity=true` for torrents and magnets
pub async fn retry(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(payload): Json<RetryReq>,
) -> impl IntoResponse {
 //    let gid = payload.gid.clone();
	//
 //    let record = sqlx::query!(
 //        r#"
	// 		SELECT
	// 			dir,
	// 			files,
	// 			options,
	// 			info_hash,
	// 			source_uri,
	// 			total_length,
	// 			is_resume_supported,
	// 			kind as "kind: Kind",
	// 			status as "status: GidStatus",
	// 			torrent as "torrent: sqlx::types::Json<TorrentMagnetMeta>"
	// 		FROM download_history
	// 		WHERE gid = ?
 //        "#,
 //        gid
 //    )
 //    .fetch_optional(&state.db)
 //    .await
 //    .unwrap_or(None);
	//
 //    let rec = match record {
 //        Some(r) => r,
 //        None => {
 //            return (StatusCode::NOT_FOUND,
 //                Json(json!({ "error": format!("GID {} not found", gid) })))
 //                .into_response();
 //        }
 //    };
	//
 //    if rec.status != GidStatus::Error
	// 	|| rec.status != GidStatus::Stopped
	// 	|| rec.status != GidStatus::Stopped {
 //        return (
 //            StatusCode::CONFLICT,
 //            Json(json!({
 //                "error": "Only GID with 'stopped', 'removed' and 'error' status can be retried",
 //            })),
 //        ).into_response();
 //    }
	//
 //    let resource_to_add = if let Some(uri) = rec.source_uri.clone() {
 //        uri
 //    } else if let Some(hash) = rec.info_hash.as_deref() {
 //        format!("magnet:?xt=urn:btih:{}", hash)
 //    } else {
 //        return (
 //            StatusCode::BAD_REQUEST,
 //            Json(json!({ "error": "Can't retry, no source URI or info hash available" })),
 //        ).into_response();
 //    };
	//
 //    let is_torrent = matches!(rec.kind, Kind::Torrent)
 //        || rec.torrent.is_some()
 //        || rec.info_hash.is_some();
	//
	// // Bools are text in db
	// let should_resume = rec
	// 	.is_resume_supported
	// 	.as_ref()
	// 	.map(|s| s.eq_ignore_ascii_case("true"))
	// 	.unwrap_or(false);
	//
	// // clear from aria2 memory
 //    if !should_resume {
 //        let _ = state.aria2.call("aria2.forceRemove", vec![json!(gid.clone())]).await;
 //        let _ = state.aria2.call("aria2.removeDownloadResult", vec![json!(gid.clone())]) .await;
 //    }
	//
 //    let mut options = serde_json::Map::new();
	//
 //    if let Some(dir) = rec.dir.clone() {
 //        options.insert("dir".to_string(), json!(dir));
 //    }
	//
 //    if should_resume {
 //        options.insert("continue".to_string(), json!(true));
 //        if is_torrent {
 //            options.insert("check-integrity".to_string(), json!(true));
 //        }
 //    } else {
 //        options.insert("allow-overwrite".to_string(), json!(true));
 //        options.insert("continue".to_string(), json!(false));
 //        options.insert("check-integrity".to_string(), json!(false));
 //    }
	//
 //    let params = vec![json!([resource_to_add]), json!(options)];
	//
 //    match state.aria2.call("aria2.addUri", params).await {
 //        Ok(new_gid_val) => {
 //            let new_gid = new_gid_val.as_str().unwrap_or_default();
	//
 //            let _ = sqlx::query!(
 //                r#"
	// 				UPDATE download_history
	// 					SET gid = ?,
	// 						error_code = NULL,
	// 						error_message = NULL,
	// 						updated_at = CURRENT_TIMESTAMP
	// 				WHERE gid = ?
 //                "#,
 //                new_gid,
 //                gid
 //            )
 //            .execute(&state.db)
 //            .await;
	//
 //            (StatusCode::OK, Json(json!({ "newGid": new_gid }))) .into_response()
 //        }
 //        Err(e) => (
 //            StatusCode::BAD_GATEWAY,
 //            Json(json!({ "error": e })),
 //        ).into_response(),
 //    }
}

/// Fetches the options snapshot from the local database instead of Aria2,
/// Ensuring availability even for `stopped` `removed` downloads.
/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.getOption
pub async fn get_option(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(payload): Json<GidReq>,
) -> impl IntoResponse {
    let row = match sqlx::query!(
        "SELECT options FROM download_history WHERE gid = ?",
        payload.gid
    )
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(row)) => row,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": format!("No download record found for GID '{}'", payload.gid) })),
            );
        }
        Err(e) => {
            error!("Database error fetching options for GID '{}': {:?}", payload.gid, &e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Internal server database error: {}", e.to_string()) })),
            );
        }
    };

    // Options column is present but null, fall back to an empty json
    let options_json: serde_json::Value = match row.options {
        Some(opts_str) if !opts_str.is_empty() => {
            serde_json::from_str(&opts_str).unwrap_or_else(|_| serde_json::json!({}))
        }
        _ => serde_json::json!({}),
    };

    (StatusCode::OK, Json(serde_json::json!({ "data": options_json })))
}

/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.changeOption
pub async fn set_option(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(payload): Json<SetOptionReq>,
) -> impl IntoResponse {
    let params = vec![
        serde_json::json!(payload.gid),
        serde_json::json!(payload.options),
    ];

    match state.aria2.call("aria2.changeOption", params).await {
        Ok(_) => {
            let existing = sqlx::query!(
                    "SELECT options
                    FROM download_history
                    WHERE gid = ?",
                    payload.gid
                ).fetch_optional(&state.db)
                .await
                .ok()
                .flatten()
                .and_then(|r| r.options)
                .unwrap_or_else(|| "{}".to_string());

            let mut current_opt: Aria2Options
                = serde_json::from_str(&existing).unwrap_or(Aria2Options(serde_json::Map::new()));

            current_opt.merge(payload.options);
            let final_json = serde_json::to_string(&current_opt).unwrap();

            let _ = sqlx::query!(
                "UPDATE download_history
                SET options = ?,
                updated_at = CURRENT_TIMESTAMP
                WHERE gid = ?",
                final_json,
                payload.gid
            ).execute(&state.db).await;

            (StatusCode::OK, Json(serde_json::json!({ "status": "optionsUpdated"})))
        }
        Err(e) => {
            error!("Failed to change options for GID {}: {}", payload.gid, e);
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": format!("Failed to change options for GID: {}, {}", payload.gid, e.to_string()) })))
        }
    }
}


/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.getGlobalOption
/// This returns whole options
pub async fn get_global_option(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> impl IntoResponse {
    match state.aria2.call("aria2.getGlobalOption", vec![]).await {
        Ok(options) => (StatusCode::OK,  Json(serde_json::json!({ "data": options }))),
        Err(e) => {
            error!("Failed to fetch global options: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Failed to retrieve global options: {}", e.to_string())  })))
        }
    }
}


/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.changeGlobalOption
pub async fn set_global_option(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<SetGlobalOptionReq>,
) -> impl IntoResponse {
    let mut safe_options = payload.options;
    safe_options.filter_restricted();

    let params = vec![serde_json::json!(safe_options)];

    match state.aria2.call("aria2.changeGlobalOption", params).await {
        Ok(_) => {
            let existing = sqlx::query!(
                "SELECT value FROM settings WHERE key = 'aria2_global_options' AND user_id = ?",
                user.user_id
            )
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .map(|r| r.value)
            .unwrap_or_else(|| "{}".to_string());

            let mut current_opt: Aria2Options = serde_json::from_str(&existing)
                .unwrap_or_else(|_| Aria2Options(serde_json::Map::new()));

            current_opt.merge(safe_options);

            let final_json = serde_json::to_string(&current_opt).unwrap();

            let _ = sqlx::query!(
                r#"
                    INSERT INTO settings (
                        key,
                        value,
                        user_id,
                        updated_at
                    )
                    VALUES (
                        'aria2_global_options',
                        ?,
                        ?,
                        CURRENT_TIMESTAMP
                    )
                    ON CONFLICT(key, user_id) DO UPDATE
                    SET value = excluded.value, updated_at = CURRENT_TIMESTAMP
                "#,
                final_json,
                user.user_id
            )
            .execute(&state.db)
            .await;

            (StatusCode::OK, Json(serde_json::json!({ "status": "globalOptionsUpdated" })))
        },
        Err(e) => {
            error!("Failed to set global options: {}", e);
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": format!("Failed to set global options: {}", e.to_string()) })))
        }
    }
}

/// Those options that can't be set via aria2 rpc
/// FIX: need restore the active, waiting, paused,
pub async fn set_cmd_option(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<SetCmdOptionsReq>,
) -> impl IntoResponse {
    let existing = sqlx::query!(
        "SELECT value FROM settings WHERE key = 'aria2_cmd_options' AND user_id = ?",
        user.user_id
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|r| r.value)
    .unwrap_or_else(|| "{}".to_string());

    let mut existing_cmd: HashMap<String, String> = serde_json::from_str(&existing)
		.unwrap_or_default();

    // `Aria2Options` into safe cli strings
    // Silently ignore restricted keys from payload
    let safe_new_options = payload
        .options
        .into_iter()
        .filter(|(key, _)| !RESTRICTED_ARIA2_KEYS.contains(&key.as_str()))
        .map(|(key, value)| {
            let text = match value {
				// Extract raw string without quotes
                Value::String(v) => v,
				// Safely stringify numbers, bools
                _ => value.to_string(),
            };
            (key, text)
        })
        .collect::<HashMap<String, String>>();

    // Delta modifications
    for (key, value) in safe_new_options {
        if value.trim().is_empty() {
            existing_cmd.remove(&key);
        } else {
            existing_cmd.insert(key, value);
        }
    }

    if let Err(err) = state.daemon.restart(&state.config, &existing_cmd).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Daemon launch failed: {}", err.to_string()) })),
        );
    }

    // Record mutations safely
    let final_json = serde_json::to_string(&existing_cmd).unwrap();
    let _ = sqlx::query!(
        r#"
            INSERT INTO settings (
				key,
				value,
				user_id,
				updated_at
			)
            VALUES (
				'aria2_cmd_options',
				?,
				?,
				CURRENT_TIMESTAMP
			)
            ON CONFLICT(key, user_id) DO UPDATE
            SET value = excluded.value, updated_at = CURRENT_TIMESTAMP
        "#,
        final_json,
        user.user_id
    )
    .execute(&state.db)
    .await;

	// Restore all the dl to their state, after daemon starts
	sleep(Duration::from_millis(1000)).await;
	HistoryService::restore_dl(&state).await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "cmdOptionsUpdated",
            "message": "Aria2 daemon restarted with updated arguments."
        })),
    )
}

/*
	Search (no overhead of querying all and ranking)
*/
const SEARCH_DEFAULT_PER_PAGE: u32 = 15;
const SEARCH_MAX_PER_PAGE: u32 = 100;

fn normalize_search_text(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

fn tokenize_search_text(input: &str) -> Vec<String> {
    input
        .split(|c: char| !c.is_alphanumeric() && c != '.' && c != '_' && c != '-')
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

fn normalized_extension(extension: &str) -> Option<String> {
    let ext = extension.trim().trim_start_matches('.').to_ascii_lowercase();
    if ext.is_empty() {
        None
    } else {
        Some(ext)
    }
}

/// Later use the `file-format` crate to widely map all files ext
fn file_type_extensions(file_type: &str) -> Option<&'static [&'static str]> {
    match normalize_search_text(file_type).as_str() {
        "image" | "images" => Some(&[
            "jpg", "jpeg", "png", "gif", "webp", "bmp", "svg", "ico", "tif", "tiff", "heic",
            "avif", "raw",
        ]),
        "video" | "videos" => Some(&[
            "mp4", "mkv", "avi", "mov", "webm", "flv", "m4v", "mpg", "mpeg", "ts", "m2ts",
            "wmv", "3gp",
        ]),
        "audio" | "audios" => Some(&[
            "mp3", "flac", "wav", "ogg", "aac", "m4a", "opus", "alac", "ape", "wma",
        ]),
        "document" | "documents" | "doc" | "docs" => Some(&[
            "pdf", "txt", "md", "doc", "docx", "rtf", "epub", "mobi", "djvu", "odt", "ods",
            "xls", "xlsx", "ppt", "pptx", "csv",
        ]),
        "archive" | "archives" => Some(&[
            "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "lz", "lzma", "cab",
        ]),
        "book" | "books" => Some(&[
            "epub", "mobi", "azw", "azw3", "pdf", "cbz", "cbr", "djvu",
        ]),
        "code" | "source" | "sources" => Some(&[
            "rs", "js", "ts", "py", "go", "c", "cpp", "h", "hpp", "java", "sh", "lua", "json",
            "yml", "yaml", "toml", "xml", "html", "css", "sql", "rb", "php", "swift", "kt",
            "kts", "dart", "cs", "md",
        ]),
        "subtitle" | "subtitles" => Some(&["srt", "ass", "ssa", "vtt"]),
        "font" | "fonts" => Some(&["ttf", "otf", "woff", "woff2", "eot"]),
        "torrent" | "torrents" => Some(&["torrent"]),
        "playlist" | "playlists" => Some(&["m3u", "m3u8", "pls"]),
        _ => None,
    }
}

fn push_field_like_predicate(
    qb: &mut QueryBuilder<sqlx::Sqlite>,
    column: &str,
    pattern: &str,
) {
    qb.push("LOWER(COALESCE(");
    qb.push(column);
    qb.push(", '')) LIKE ");
    qb.push_bind(pattern.to_owned());
}

fn push_text_match_predicate(
    qb: &mut QueryBuilder<sqlx::Sqlite>,
    q_norm: &str,
    tokens: &[String],
) {
    let prefix = format!("{q_norm}%");
    let contains = format!("%{q_norm}%");

    qb.push("(");
    for (idx, column) in ["name", "source_uri", "files"].iter().enumerate() {
        if idx > 0 {
            qb.push(" OR ");
        }

        qb.push("(");
        push_field_like_predicate(qb, column, &contains);
        qb.push(" OR ");
        push_field_like_predicate(qb, column, &prefix);

        for token in tokens {
            let token_contains = format!("%{token}%");
            qb.push(" OR ");
            push_field_like_predicate(qb, column, &token_contains);
        }

        qb.push(")");
    }
    qb.push(")");
}

fn push_extension_predicate(
    qb: &mut QueryBuilder<Sqlite>,
    extension: &str,
) {
    let Some(ext) = normalized_extension(extension) else {
        qb.push("1 = 0");
        return;
    };

    let pattern = format!("%.{ext}%");

    qb.push("(");
    for (idx, column) in ["name", "source_uri", "files"].iter().enumerate() {
        if idx > 0 {
            qb.push(" OR ");
        }
        push_field_like_predicate(qb, column, &pattern);
    }
    qb.push(")");
}

fn push_category_predicate(
    qb: &mut QueryBuilder<Sqlite>,
    file_type: &str,
) {
    let Some(extensions) = file_type_extensions(file_type) else {
        qb.push("1 = 0");
        return;
    };

    qb.push("(");
    for (i, ext) in extensions.iter().enumerate() {
        if i > 0 {
            qb.push(" OR ");
        }

        let pattern = format!("%.{ext}%");
        qb.push("(");
        for (j, column) in ["name", "source_uri", "files"].iter().enumerate() {
            if j > 0 {
                qb.push(" OR ");
            }
            push_field_like_predicate(qb, column, &pattern);
        }
        qb.push(")");
    }
    qb.push(")");
}

fn push_column_rank_expr(
    qb: &mut QueryBuilder<Sqlite>,
    column: &str,
    q_norm: &str,
    tokens: &[String],
) {
    let exact = q_norm.to_string();
    let prefix = format!("{q_norm}%");
    let contains = format!("%{q_norm}%");

    qb.push("CASE ");
    qb.push("WHEN LOWER(COALESCE(");
    qb.push(column);
    qb.push(", '')) = ");
    qb.push_bind(exact);
    qb.push(" THEN 120 ");

    qb.push("WHEN LOWER(COALESCE(");
    qb.push(column);
    qb.push(", '')) LIKE ");
    qb.push_bind(prefix);
    qb.push(" THEN 90 ");

    qb.push("WHEN LOWER(COALESCE(");
    qb.push(column);
    qb.push(", '')) LIKE ");
    qb.push_bind(contains);
    qb.push(" THEN 60 ");

    for token in tokens {
        let token_pattern = format!("%{token}%");
        qb.push("WHEN LOWER(COALESCE(");
        qb.push(column);
        qb.push(", '')) LIKE ");
        qb.push_bind(token_pattern);
        qb.push(" THEN 8 ");
    }

    qb.push("ELSE 0 END");
}

fn push_rank_expr(
    qb: &mut QueryBuilder<Sqlite>,
    q: Option<&str>,
) {
    let Some(q) = q.map(normalize_search_text).filter(|s| !s.is_empty()) else {
        qb.push("0");
        return;
    };

    let tokens = tokenize_search_text(&q);

    qb.push("(");
    push_column_rank_expr(qb, "name", &q, &tokens);
    qb.push(" + ");
    push_column_rank_expr(qb, "source_uri", &q, &tokens);
    qb.push(" + ");
    push_column_rank_expr(qb, "files", &q, &tokens);
    qb.push(")");
}

fn apply_search_filters(
    qb: &mut QueryBuilder<Sqlite>,
    q: Option<&str>,
    file_type: Option<&str>,
    extension: Option<&str>,
) {
    let q = q.map(str::trim).filter(|s| !s.is_empty());
    let file_type = file_type.map(str::trim).filter(|s| !s.is_empty());
    let extension = extension.map(str::trim).filter(|s| !s.is_empty());

    if let Some(q) = q {
        let q_norm = normalize_search_text(q);
        let tokens = tokenize_search_text(&q_norm);

        qb.push(" AND ");
        push_text_match_predicate(qb, &q_norm, &tokens);
    }

    if let Some(file_type) = file_type {
        qb.push(" AND ");
        push_category_predicate(qb, file_type);
    }

    if let Some(extension) = extension {
        qb.push(" AND ");
        push_extension_predicate(qb, extension);
    }
}

fn sanitize_page(page: Option<u32>) -> u32 {
    page.unwrap_or(1).max(1)
}

fn sanitize_per_page(per_page: Option<u32>) -> u32 {
    per_page
        .unwrap_or(SEARCH_DEFAULT_PER_PAGE)
        .clamp(1, SEARCH_MAX_PER_PAGE)
}

/// Search by fuzzy matching name
/// Search by fuzzy source_uri
/// Search by specific file type like (images, video, audio, document)
/// Search by specific file extension like (.mkv, .png, .wav, .pdf)
pub async fn search(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Query(params): Query<SearchQuery>,
) -> impl IntoResponse {
    let page = sanitize_page(params.page);
    let per_page = sanitize_per_page(params.per_page);
    let offset = ((page - 1) * per_page) as i64;

    let q = params.q.as_deref();
    let file_type = params.file_type.as_deref();
    let extension = params.extension.as_deref();

    let total: i64 = {
        let mut qb = QueryBuilder::<Sqlite>::new(
            "SELECT COUNT(*) FROM download_history WHERE user_id = ",
        );
        qb.push_bind(&user.user_id);

        apply_search_filters(&mut qb, q, file_type, extension);

        match qb.build_query_scalar().fetch_one(&state.db).await {
            Ok(total) => total,
            Err(e) => {
                error!(
                    "Failed to count search results for user {}: {}",
                    user.user_id, &e
                );
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": format!("Failed to retrieve search results: {:?}", e.to_string()), })),
                );
            }
        }
    };

    let results: Vec<ItemMetaData> = {
        let mut qb = QueryBuilder::<Sqlite>::new("SELECT * FROM download_history WHERE user_id = ");
        qb.push_bind(&user.user_id);

        apply_search_filters(&mut qb, q, file_type, extension);

        qb.push(" ORDER BY ");
        push_rank_expr(&mut qb, q);
        qb.push(" DESC, created_at DESC, gid DESC LIMIT ");
        qb.push_bind(per_page as i64);
        qb.push(" OFFSET ");
        qb.push_bind(offset);

        match qb.build_query_as::<ItemMetaData>().fetch_all(&state.db).await {
            Ok(rows) => rows,
            Err(e) => {
                error!("Failed to fetch search results for user {}: {}", user.user_id, e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": format!("Failed to retrieve search results: {}", e.to_string()) })),
                );
            }
        }
    };

    (
        StatusCode::OK,
        Json(json!({
            "data": results,
			"meta": {
				"page": page,
				"perPage": per_page,
				"total": total
			}
        })),
    )
}


/// Get `aria2.getVersion`
/// Get supported hash algorithms via command stdout of `aria2 --version`
/// Get libraries info, compiler info
/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.getSessionInfo
/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.getVersion
pub async fn get_aria2_info(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> impl IntoResponse {
    let rpc_version = match state.aria2.call("aria2.getVersion", vec![]).await {
        Ok(version) => version,
        Err(e) => {
            error!("Failed to get aria2 version: {}", e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": e.to_string()
                })),
            );
        }
    };

    let session_id = match state.aria2.call("aria2.getSessionInfo", vec![]).await {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to get aria2 session id: {}", e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": e.to_string()
                })),
            );
        }
    };

    let cli_info = match Command::new("aria2c").arg("--version").output().await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let parsed = parse_aria2c_version_stdout(&stdout);

            if output.status.success() {
                parsed
            } else {
                json!({
                    "error": "aria2c --version exited with a non-zero status",
                    "stdout": parsed
                })
            }
        }
        Err(e) => {
            warn!("Failed to execute `aria2c --version`: {}", e);
            json!({
                "error": e.to_string()
            })
        }
    };

    let mut data_obj = json!({});
	if let Some(obj) = cli_info.as_object() {
        for (k, v) in obj {
            data_obj[k] = v.clone();
        }
    }
    if let Some(obj) = rpc_version.as_object() {
        for (k, v) in obj {
            data_obj[k] = v.clone();
        }
    }
    if let Some(obj) = session_id.as_object() {
        for (k, v) in obj {
            data_obj[k] = v.clone();
        }
    }

    (
        StatusCode::OK,
		Json(json!({
			"data": data_obj
		})),
    )
}

fn parse_aria2c_version_stdout(stdout: &str) -> Value {
    let mut hash_algorithms: Vec<String> = Vec::new();
    let mut libraries: Vec<Value> = Vec::new();
    let mut compiler = serde_json::Map::new();
    let mut in_compiler_section = false;

    for raw_line in stdout.lines() {
        let line = raw_line.trim_end();

        if let Some(rest) = line.strip_prefix("Hash Algorithms:") {
            hash_algorithms = rest
                .split(',')
                .map(|item| item.trim())
                .filter(|item| !item.is_empty())
                .map(|item| item.to_string())
                .collect();
            continue;
        }

        if let Some(rest) = line.strip_prefix("Libraries:") {
            libraries = rest
                .split_whitespace()
                .filter_map(|item| {
                    let (name, version) = item.split_once('/')?;
                    Some(json!({
                        "name": name,
                        "version": version
                    }))
                })
                .collect();
            continue;
        }

        if let Some(rest) = line.strip_prefix("Compiler:") {
            let rest = rest.trim();
            let mut parts = rest.split_whitespace();
            if let Some(name) = parts.next() {
                compiler.insert("name".to_string(), json!(name));
            }

            let version = parts.collect::<Vec<_>>().join(" ");
            if !version.is_empty() {
                compiler.insert("version".to_string(), json!(version));
            }

            in_compiler_section = true;
            continue;
        }

        if in_compiler_section {
            let trimmed = line.trim_start();

            if let Some(rest) = trimmed.strip_prefix("built by") {
                compiler.insert("builtBy".to_string(), json!(rest.trim()));
                continue;
            }

            if let Some(rest) = trimmed.strip_prefix("on") {
                compiler.insert("builtOn".to_string(), json!(rest.trim()));
                continue;
            }

            if let Some(rest) = trimmed.strip_prefix("System:") {
                compiler.insert("system".to_string(), json!(rest.trim()));
                continue;
            }

            if trimmed.is_empty()
                || trimmed.starts_with("Report bugs to")
                || trimmed.starts_with("Visit ")
            {
                in_compiler_section = false;
            }
        }
    }

    json!({
        "hashAlgorithms": hash_algorithms,
        "libraries": libraries,
        "compiler": compiler
    })
}

fn his_fltr(
	query: &mut QueryBuilder<Sqlite>,
	status: Option<&str>,
	category: Option<&str>,
) {
	if let Some(s) = status {
		let trimmed = s.trim().to_lowercase();
		// Ignore "all" or empty string -> fetch all statuses
		if !trimmed.is_empty() && trimmed != "all" {
			query.push(" AND status = ");
			query.push_bind(trimmed);
		}
	}

	if let Some(c) = category {
		let trimmed = c.trim().to_lowercase();
		// Ignore "everything", "all" or empty string -> fetch all categories
		if !trimmed.is_empty() && trimmed != "everything" && trimmed != "all" {
			query.push(" AND category = ");
			query.push_bind(trimmed);
		}
	}
}

/// Get dl history based on query
/// Query should also include the status criteria
pub async fn get_his(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Query(params): Query<PaginationQuery>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(15).max(1);
    let offset = (page - 1) * limit;

	// Resolve column name direction from enums
	let order_column = params
		.sort_by
		.as_ref()
		.map(|s| s.to_sql_column())
		.unwrap_or("created_at");

	let order_direction = params
		.order
		.as_ref()
		.map(|o| o.to_sql_keyword())
		.unwrap_or("DESC");

	// Total items count matching the exact filters
	let mut count_query: QueryBuilder<Sqlite> = QueryBuilder::new(
		"SELECT COUNT(*) as count FROM download_history WHERE user_id = "
	);

	count_query.push_bind(&user.user_id);
	his_fltr(&mut count_query, params.status.as_deref(),
	params.category.as_deref());

	#[derive(sqlx::FromRow)]
	struct CountRow {
		count: i64,
	}

	let total_items = count_query
		.build_query_as::<CountRow>()
		.fetch_one(&state.db)
		.await
		.map(|row| row.count as u32)
		.unwrap_or(0);

    let total_pages = if total_items == 0 {
        0
    } else {
        (total_items as f64 / limit as f64).ceil() as u32
    };

    let has_more = page < total_pages;

	let mut query: QueryBuilder<Sqlite> = QueryBuilder::new(
        r#"
        SELECT
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
			user_id
        FROM download_history
        WHERE user_id = "#
    );

	query.push_bind(&user.user_id);
	his_fltr(&mut query, params.status.as_deref(),
    params.category.as_deref());


    // Dynamically append the sort params
    query.push(" ORDER BY ");
    query.push(order_column);
    query.push(" ");
    query.push(order_direction);
    query.push(" LIMIT ");
    query.push_bind(limit);
    query.push(" OFFSET ");
    query.push_bind(offset);

	let history: Vec<ItemMetaData> = match query
		.build_query_as::<ItemMetaData>()
		.fetch_all(&state.db)
		.await
	{
		Ok(data) => data,
		Err(err) => {
			error!("Failed to fetch download history for user {}: {:?}", user.username, &err);
			return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to retrieve history: {}", err.to_string())
                })),
            ).into_response();
		}
	};

    (
        StatusCode::OK,
        Json(json!({
            "data": history,
            "meta": {
                "currentPage": page,
                "perPage": limit,
                "totalItems": total_items,
                "totalPages": total_pages,
                "hasMore": has_more
            }
        }))
    ).into_response()
}


/// Remove dl along with deleting file
pub async fn delete(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<RemoveHisReq>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let gids: Vec<String> = payload
        .gids
        .iter()
        .filter_map(|gid| {
            let gid = gid.trim();
            if gid.is_empty() {
                None
            } else {
                Some(gid.to_string())
            }
        })
        .collect();

    if gids.is_empty() {
        return Ok(Json(serde_json::json!({
			"error": "No GID provided for deletion."
        })));
    }

	// `forceRemove` first
    let mut elgbl_res: Vec<String> = Vec::new();

    for chunk in gids.chunks(MAX_MULTICALL_BATCH) {
        let mut force_remove_batch = Vec::new();

        for gid in chunk {
            force_remove_batch.push(serde_json::json!({
                "methodName": "aria2.forceRemove",
                "params": [gid]
            }));
        }

        let response = match state.aria2.call("system.multicall", vec![serde_json::json!(force_remove_batch)]).await {
            Ok(v) => v,
            Err(e) => {
                warn!("Method `aria2.forceRemove` multicall failed: {:?}", e);
                continue;
            }
        };

        let results = match response.as_array() {
            Some(v) => v,
            None => {
                warn!("Method `aria2.forceRemove` multicall returned unexpected payload: {:?}", response);
                continue;
            }
        };

		for (idx, item) in results.iter().enumerate() {
			let gid = match chunk.get(idx) {
				Some(g) => g.to_string(),
				None => continue,
			};

			let is_cannot_remove = match item {
				serde_json::Value::Object(obj) => obj
					.get("message")
					.and_then(serde_json::Value::as_str)
					.map(|msg| {
						debug!("Method `aria2.forceRemove` error message: {:?}", msg);
						msg.to_lowercase().contains("cannot be removed now")
					})
					.unwrap_or(false),

				_ => false,
			};

			if is_cannot_remove {
				warn!(
					"Skipping GID {:?} because `aria2.forceRemove` says it cannot be removed now",
					gid
				);
				continue;
			}

			// Any other forceRemove result means this GID is still eligible:
			// - success array like ["gid"]
			// - error object like {"code":1,"message":"..."}
			// - other non-"cannot be removed now" failures
			elgbl_res.push(gid);
		}
    }

    // `removeDownloadResult` only for the eligible set
    if !elgbl_res.is_empty() {
        for chunk in elgbl_res.chunks(MAX_MULTICALL_BATCH) {
            let mut remove_result_batch = Vec::new();

            for gid in chunk {
                remove_result_batch.push(serde_json::json!({
                    "methodName": "aria2.removeDownloadResult",
                    "params": [gid]
                }));
            }

            // Do not care whether this returns OK or an error.
            // It is only for formality so aria2 does not keep the removed GIDs around.
            let _ = state
                .aria2
                .call("system.multicall", vec![serde_json::json!(remove_result_batch)])
                .await;
        }
    }


    for gid in &elgbl_res {
        if !elgbl_res.contains(gid) {
            warn!("Skipping DB deletion for GID {:?} because it was not freed from Aria2.", gid);
            continue;
        }

        if payload.delete_file {
            let record = sqlx::query!(
                "SELECT name, files, dir FROM download_history
                WHERE gid = ? AND user_id = ?",
                gid, user.user_id
            )
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;

            debug!("Found gid to delete: {:?}", record);

            if let Some(rec) = record {
                let download_dir = rec.dir.as_deref().map(Path::new);
                if let Some(name) = rec.name.as_ref() {
                    let name = name.to_string();
                    if let Some(files_str) = rec.files {
                        let file_paths: Vec<String> = serde_json::from_str(&files_str).unwrap_or_default();
						debug!("File paths to delete: {:?}", file_paths);
                        for path in file_paths {
                            let path = Path::new(&path);
                            debug!("Removing file: {:?}", path);
                            if !path.exists() {
                                continue;
                            }
                            if path.is_dir() {
								match std::fs::remove_dir_all(&path) {
									Ok(_) => {
										debug!("Successfully deleted: {:?}", path);
									}
									Err(e) => {
										error!("Failed to delete `{:?}`: {:?}", path, e);
									}
								}
                            } else {
								match std::fs::remove_file(&path) {
									Ok(_) => {
										debug!("Successfully deleted: {:?}", path);
									}
									Err(e) => {
										error!("Failed to delete `{:?}`: {:?}", path, e);
									}
								}
                            }
                            if let Some(root) = download_dir {
                                let aria_name = format!("{}.aria2", name);
                                let aria2_path = root.join(Path::new(&aria_name));
								let _ = std::fs::remove_file(&aria2_path)
									.map(|_| debug!("Successfully deleted .aria2 file: {:?}", &aria2_path))
									.map_err(|e| error!("Failed to delete .aria2 file: `{:?}`: {:?}", aria2_path, e));
                                let mut current = path.parent();
                                while let Some(dir) = current {
                                    if dir == root {
                                        break;
                                    }
                                    if !dir.exists() || !dir.is_dir() {
                                        break;
                                    }
                                    let is_empty = std::fs::read_dir(dir)
                                        .map(|mut entries| entries.next().is_none())
                                        .unwrap_or(false);

                                    if !is_empty {
                                        break;
                                    }

									let _ = std::fs::remove_dir(dir)
										.map(|_| debug!("Successfully deleted: {:?}", path))
										.map_err(|e| error!("Failed to delete `{:?}`: {:?}", path, e));
                                    current = dir.parent();
                                }
                            }
                        }
                    }
                } else {
                    debug!("Skipping file deletion for gid {:?}: name is NULL", gid);
                }
            } else {
                return Err((
                    StatusCode::NOT_FOUND,
                    Json(json!({ "error": format!("No download history found from GID {}", gid) })),
                ));
            }
        }

        /* Ensure `user_id` matches history */
        let res = sqlx::query!(
            "DELETE FROM download_history WHERE gid = ? AND user_id = ?",
            gid, user.user_id
        )
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;

        if res.rows_affected() == 0 {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": format!("Download history record not found from GID: {:?}", gid)
                })),
            ));
        }

        info!("Deleted gid: {:?}", gid);
    }

    Ok(Json(json!({
		"status": "deleted",
		"deletionCount": elgbl_res.len()
	})))
}


/// Get count based on `Category` and `GidStatus`
pub async fn get_items_count(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let mut stats = ItemCoundStatsRes::default();
    /*
	  * Get minimal required col from `download_history`
	  * Based on user
	*/
    let rows = sqlx::query!(
        "SELECT name, status, kind, category FROM download_history WHERE user_id = ?",
		user.user_id
    )
	.fetch_all(&state.db)
    .await
	.map_err(|e|
		(StatusCode::INTERNAL_SERVER_ERROR,
		Json(json!({ "error": format!("Failed to query download row: {}", e.to_string())
	}))))?;

    // Process counts
    for row in rows {
        // Increment 'all' and 'everything' base counts
        stats.by_status.all += 1;
        stats.by_category.everything += 1;

        // Status mapping
        match row.status.as_str() {
            "active" => stats.by_status.active += 1,
            "paused" => stats.by_status.paused += 1,
            "waiting" => stats.by_status.waiting += 1,
            "complete" => stats.by_status.complete += 1,
            "removed" => stats.by_status.removed += 1,
            "stopped" => stats.by_status.stopped += 1,
            "error" => stats.by_status.error += 1,
            "seeding" => stats.by_status.seeding += 1,
            _ => {}												// Fallback
        }

		let cat_str = row.category.unwrap_or_else(|| {
			match Category::determine(row.name.as_deref(), &row.kind) {
				Category::Torrent => "torrent".to_string(),
				Category::Music => "music".to_string(),
				Category::Photos => "photos".to_string(),
				Category::Programs => "programs".to_string(),
				Category::Videos => "videos".to_string(),
				Category::Subtitles => "subtitles".to_string(),
				Category::Compressed => "compressed".to_string(),
				Category::Others => "others".to_string(),
			}
		});

		match cat_str.as_str()  {
			"torrent" => stats.by_category.torrent += 1,
			"music" => stats.by_category.music += 1,
			"photos" => stats.by_category.photos += 1,
			"programs" => stats.by_category.programs += 1,
			"videos" => stats.by_category.videos += 1,
			"subtitles" => stats.by_category.subtitles += 1,
			"compressed" => stats.by_category.compressed += 1,
			_ => stats.by_category.others += 1,
		}
    }

    Ok(Json(json!({ "data": stats })))
}

impl Category {
    pub fn determine(name: Option<&str>, kind: &str) -> Self {
		if kind == "torrent" {
            return Category::Torrent;
        }

		let Some(filename) = name else {
            return Category::Others;
		};

		if filename.ends_with(".torrent") {
            return Category::Torrent;
        }

        // Guess the format by extension without reading the file from disk
		let ext = std::path::Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

		if let Some(format) = FileFormat::from_extension(ext).first() {
            match format.kind() {
                FileKind::Audio => Category::Music,
                FileKind::Image => Category::Photos,
                FileKind::Executable | FileKind::Package | FileKind::Rom => Category::Programs,
                FileKind::Video => Category::Videos,
                FileKind::Subtitle => Category::Subtitles,
                FileKind::Archive | FileKind::Compressed => Category::Compressed,
                _ => Category::Others,
            }
        } else {
            Category::Others
        }
    }
}




/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.changeUri
/// Change the URI by GID
pub async fn change_uri(
    State(state): State<AppState>,
    Json(payload): Json<ChangeUriReq>,
) -> impl IntoResponse {

}

// Handler for seeding
/// Only complete dl can be seeded,
/// take multiple gids & an optional arg to stop seeding
pub async fn manage_seeding(
    State(state): State<AppState>,
    Json(payload): Json<ChangeUriReq>,
) -> impl IntoResponse {

}

/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.purgeDownloadResult
/// Purge `completed, error, stopped` dl and free aria2 memory
pub async fn purge_all(
	State(state): State<AppState>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    let gids: HashSet<String> = sqlx::query_scalar!(
        r#"
        SELECT gid
        FROM download_history
        WHERE user_id = ?
          AND status IN ('complete', 'error', 'stopped')
        "#,
        user.user_id
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })
    .into_iter()
    .flatten()
    .collect();

	debug!("Purging GIDs: {:?}", gids);

    if gids.is_empty() {
		return (
			StatusCode::BAD_REQUEST,
			Json(json!({ "error": "No complete, error, stopped downloads to purge." })),
		);
    }

    match state.aria2.call("aria2.purgeDownloadResult", vec![]).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "status": "purgedAll" }))),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(json!({ "error": e.to_string() }))),
    }
}

pub async fn move_position(
    State(state): State<AppState>,
    Json(payload): Json<MoveReq>,
) -> impl IntoResponse {
    let params = vec![
        json!(payload.gid),
        json!(payload.pos),
        json!(payload.how)
    ];

    state.aria2.call("aria2.changePosition", params)
        .await
        .inspect(|res| info!("Successfully executed multicall: {:?}", res))
        .inspect_err(|e| info!("failed to add uri: {:?}", e))
        .map(|results| (StatusCode::OK, Json(json!({ "newPosition": results }))))
        .unwrap_or_else(|e| (StatusCode::BAD_GATEWAY, Json(json!({ "error": e.to_string() }))))
}

