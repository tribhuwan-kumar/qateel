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

impl ItemMetaData {
    /// Returns true if the download has reached a finished state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            GidStatus::Complete | GidStatus::Stopped | GidStatus::Error | GidStatus::Removed
        )
    }

    /// Merges dynamic telemetry fields from aria2 into `self`
    /// While preserving static database metadata
    pub fn update_from_telemetry(&mut self, tele: ItemMetaData) {
        self.status = tele.status;
        self.dir = tele.dir.or_else(|| self.dir.take());
        self.files = tele.files.or_else(|| self.files.take());
        self.total_length = tele.total_length.or_else(|| self.total_length.take());
        self.completed_length = tele.completed_length.or_else(|| self.completed_length.take());
        self.upload_length = tele.upload_length.or_else(|| self.upload_length.take());
        self.connections = tele.connections.or_else(|| self.connections.take());
        self.num_pieces = tele.num_pieces.or_else(|| self.num_pieces.take());
        self.num_seeders = tele.num_seeders.or_else(|| self.num_seeders.take());
        self.piece_length = tele.piece_length.or_else(|| self.piece_length.take());
        self.verified_length = tele.verified_length.or_else(|| self.verified_length.take());
        self.verify_integrity_pending = tele.verify_integrity_pending.or(self.verify_integrity_pending);
        self.seeder = tele.seeder.or(self.seeder);
        self.error_code = tele.error_code.or(self.error_code);
        self.error_message = tele.error_message.or_else(|| self.error_message.take());

        // Fill discovered metadata if previously missing
        if self.name.is_none() && tele.name.is_some() {
            self.name = tele.name;
        }
        if self.source_uri.is_none() && tele.source_uri.is_some() {
            self.source_uri = tele.source_uri;
        }
        if self.torrent.is_none() && tele.torrent.is_some() {
            self.torrent = tele.torrent;
        }
        if self.kind == Kind::Unknown && tele.kind != Kind::Unknown {
            self.kind = tele.kind;
        }

        if self.status == GidStatus::Complete && self.completed_at.is_none() {
            self.completed_at = Some(chrono::Utc::now().naive_utc());
        }
    }

    /// Extract raw dynamic telemetry from `aria2.tellStatus`
    pub fn extract(aria2_res: &Value, gid: &str) -> ItemMetaData {
        let info: Aria2Res = match serde_json::from_value(aria2_res.clone()) {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to parse aria2 status for GID '{:?}': {:?}", gid, e);
                return Self::skeleton(gid);
            }
        };
		debug!("Aria2 info res: {:?}", info);
        let file_paths: Vec<String> = info
            .files
            .iter()
            .map(|f| f.path.clone())
            .filter(|p| !p.is_empty())
            .collect();
        let files_json = serde_json::to_string(&file_paths).ok();

        let source_uri = info
            .files
            .first()
            .and_then(|file| file.uris.iter().find(|u| !u.uri.is_empty()).map(|u| u.uri.clone()));

        let is_magnet = source_uri.as_deref().map_or(false, |uri| uri.starts_with("magnet:"));
        let is_torrent = info.bittorrent.is_some() || info.info_hash.is_some() || is_magnet;

        let is_seeder = info.seeder.as_deref().and_then(|s| match s {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        });

        let is_verify_pending = info.verify_integrity_pending.as_deref().and_then(|s| match s {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        });

        let status = match info.status.as_str() {
            "active" => {
                if (is_torrent
                    && info.completed_length == info.total_length
                    && info.total_length != "0")
                    || is_seeder.is_some_and(|s| s)
                {
                    GidStatus::Seeding
                } else {
                    GidStatus::Active
                }
            }
            "waiting" => GidStatus::Waiting,
            "paused" => GidStatus::Paused,
            "error" => GidStatus::Error,
            "complete" => GidStatus::Complete,
            "removed" => {
                let exists = file_paths.iter().any(|p| Path::new(p).exists());
                if exists {
                    GidStatus::Stopped
                } else {
                    GidStatus::Removed
                }
            }
            _ => GidStatus::Stopped,
        };

        let kind = source_uri
            .as_deref()
            .and_then(|uri| url::Url::parse(uri).ok().map(|u| (uri, u)))
            .map(|(_uri, parsed_url)| match parsed_url.scheme().to_ascii_lowercase().as_str() {
                "magnet" => Kind::Torrent,
                "http" | "https" | "ftp" | "sftp" => Kind::Uri,
                _ => Kind::Unknown,
            })
            .unwrap_or_else(|| if is_torrent { Kind::Torrent } else { Kind::Unknown });

        let name = if is_torrent {
            info.bittorrent
                .as_ref()
                .and_then(|bt| bt.info.as_ref())
                .and_then(|i| i.name.clone())
        } else {
            info.files.first().and_then(|file| {
                if file.path.is_empty() {
                    None
                } else {
                    Path::new(&file.path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                }
            })
        };

        let torrent = info.bittorrent.as_ref().map(|bt| TorrentMagnetMeta {
            announce_list: bt.announce_list.clone().unwrap_or_default(),
            comment: bt.comment.clone(),
            creation_date: bt.creation_date,
            mode: bt.mode.clone(),
            name: bt.info.as_ref().and_then(|i| i.name.clone()),
        });

        ItemMetaData {
            gid: gid.to_string(),
            name,
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
            content_hash: None,									// Later get updated
            error_code: info.error_code.and_then(|c| c.parse().ok()),
            error_message: info.error_message,
            torrent: torrent.map(sqlx::types::Json),
            options: None,										// Later get updated
            is_resume_supported: None,							// Later get updated
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
}

