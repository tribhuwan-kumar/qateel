use std::path::Path;
use serde_json::Value;
use tracing::{error, debug};
use crate::aria2::types::{
	Kind,
    Aria2Res,
    GidStatus,
    ItemMetaData,
    TorrentMagnetMeta
};

pub struct Extraction;

impl Extraction {
    pub fn extract(
        aria2_res: &Value, /* Res of `aria2.tellStatus` */
        gid: &str
    ) -> ItemMetaData {
        let info: Aria2Res = match serde_json::from_value(aria2_res.clone()) {
            Ok(v) => v,
            Err(e) => {
                /* If empty response, return skeleton, something diasater happened */
                error!("Failed to parse aria2 status for gid: {:?}, error: {:?}", gid, e);
                return Self::skeleton(gid);
            }
        };

        let file_paths: Vec<String> = info.files.iter()
            .map(|f| f.path.clone())
            .filter(|p| !p.is_empty())
            .collect();
        let files_json = serde_json::to_string(&file_paths).ok();

        let mut source_uri: Option<String> = None;
        if let Some(file) = info.files.first() {
            // Find first non-empty uri
            source_uri = file.uris.iter()
                .find(|u| !u.uri.is_empty())
                .map(|u| u.uri.clone());
        }

        let is_magnet = source_uri.as_deref().map_or(false, |uri| uri.starts_with("magnet:"));
        let is_torrent = info.bittorrent.is_some() || info.info_hash.is_some() || is_magnet;

        debug!("Is it a torrent: {:?}", is_torrent);
        debug!("Files json: {:?}", files_json);

		let is_seeder: Option<bool> = info.seeder
			.as_deref()
			.and_then(|s| match s {
				"true" => Some(true),
				"false" => Some(false),
				_ => None,
			});

		let is_verify_pending: Option<bool> = info.verify_integrity_pending
			.as_deref()
			.and_then(|s| match s {
				"true" => Some(true),
				"false" => Some(false),
				_ => None,
			});

        let status = match info.status.as_str() {
            "active" => {
                if is_torrent &&
                    info.completed_length == info.total_length &&
                    info.total_length != "0"
                    || is_seeder.is_some_and(|s| s) {
                    GidStatus::Seeding
                } else {
                    GidStatus::Active
                }
            },
            "waiting"  => GidStatus::Waiting,
            "paused"   => GidStatus::Paused,
            "error"    => GidStatus::Error,
            "complete" => GidStatus::Complete,
            "removed"  => {
                let exists = file_paths.iter().any(|p| Path::new(p).exists());
                if exists { GidStatus::Stopped } else { GidStatus::Removed }
            },
            _ => GidStatus::Stopped,
        };

        // Determine the kind
		let kind = source_uri
			.as_deref()
			.and_then(|uri| url::Url::parse(uri).ok().map(|u| (uri, u)))
			.map(|(_uri, parsed_url)| {
				match parsed_url.scheme().to_ascii_lowercase().as_str() {
					"magnet" => Kind::Torrent,
					"http" | "https" | "ftp" | "sftp" => Kind::Uri,
					_ => Kind::Unknown,
				}
			})
			.unwrap_or_else(|| {
				if is_torrent {
					Kind::Torrent
				} else {
					Kind::Unknown
				}
			});

		let name: Option<String> = if is_torrent {
			// Try to extract the name from the bittorrent info metadata if it exists
			info.bittorrent.as_ref()
				.and_then(|bt| bt.info.as_ref())
				.and_then(|i| i.name.clone())
		} else {
			// If it's a standard URI, get the name from the first file path
			info.files.first().and_then(|file| {
				if file.path.is_empty() {
					None
				} else {
					Path::new(&file.path)
						.file_name()
						.and_then(|name| name.to_str())
						.map(|s| s.to_string())
				}
			})
		};
		debug!("File name extracted: {:?}", name);

        let torrent = info.bittorrent.as_ref().map(|bt| TorrentMagnetMeta {
            announce_list: bt.announce_list.clone().unwrap_or_default(),
            comment: bt.comment.clone(),
            creation_date: bt.creation_date,
            mode: bt.mode.clone(),
            name: bt.info.as_ref().and_then(|i| i.name.clone()),
        });

        debug!("Item kind: {:?}", kind);

        if torrent.is_some() {
            debug!("Torrent info: {:?}", torrent);
        }

        if source_uri.is_some() {
            debug!("Extracting metadata for uri {:?}", source_uri);
        }

        ItemMetaData {
            gid: gid.to_string(),
            name: name,											// Getting saved in proxy handler
            kind,
            status,
            dir: Some(info.dir),
			files: files_json,
            total_length: Some(info.total_length),
            completed_length: Some(info.completed_length),
            upload_length: Some(info.upload_length),
            source_uri,
			info_hash: info.info_hash,
			seeder: is_seeder,
			content_hash: None,
			error_code: info.error_code.and_then(|c| c.parse().ok()),
			error_message: info.error_message,
			torrent: torrent.map(sqlx::types::Json),
			options: None,                                             // Getting saved in proxy handler
			is_resume_supported: None,									// Getting saved in proxy handler
			connections: info.connections,
			num_pieces: info.num_pieces,
			num_seeders: info.num_seeders,
			piece_length: info.piece_length,
			verified_length: info.verified_length,
			verify_integrity_pending: is_verify_pending,
            created_at: None,
            completed_at: None,
            user_id: "".to_string(),
        }
    }

    fn skeleton(gid: &str) -> ItemMetaData {
        ItemMetaData {
			gid: gid.to_string(),
            name: Some(gid.to_string()),								// Keep `gid` as `name` substitute
			kind: Kind::Unknown,
			status: GidStatus::Waiting,
			dir: None,
			files: None,
			total_length: Some("0".into()),
			completed_length: Some("0".into()),
			upload_length: Some("0".into()),
			source_uri: None,
			info_hash: None,
			seeder: None,
			content_hash: None,
			error_code: None,
			error_message: None,
			torrent: None,
			options: None,
			is_resume_supported: Some(false),
			connections: Some("0".into()),
			num_pieces: Some("0".into()),
			num_seeders: Some("0".into()),
			piece_length: Some("0".into()),
			verified_length: Some("0".into()),
			verify_integrity_pending: Some(false),
            created_at: None,
            completed_at: None,
            user_id: "".to_string(),
        }
    }

	/// Extracts magnet info hash
	pub fn extract_mgn_info_hash(uri: &str) -> Option<String> {
		if uri.starts_with("magnet:") {
			if let Ok(u) = url::Url::parse(uri) {
				for (k, v) in u.query_pairs() {
					if k == "xt" && v.starts_with("urn:btih:") {
						return Some(v.trim_start_matches("urn:btih:").to_ascii_lowercase());
					}
				}
			}
		}
		None
	}

	/// Extracts magnet file name
	pub fn extract_mgn_name(uri: &str) -> Option<String> {
		if let Ok(u) = url::Url::parse(uri) {
			for (k, v) in u.query_pairs() {
				if k == "dn" && !v.trim().is_empty() {
					return Some(v.into_owned());
				}
			}
		}
		None
	}

	/// Extracts url filename
	pub fn extract_url_filename(uri: &str) -> Option<String> {
		if let Ok(u) = url::Url::parse(uri) {
			if let Some(s) = u.path_segments().and_then(|seg| seg.last()) {
				if let Ok(d) = urlencoding::decode(s) {
					if !d.trim().is_empty() {
						return Some(d.into_owned());
					}
				}
			}
		}
		None
	}
}

