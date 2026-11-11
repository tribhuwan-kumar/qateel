use std::path::Path;
use crate::aria2::types::{
	Kind,
	Category,
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
	/// Stop updating the DB & streaming WS, on these status
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            GidStatus::Complete | GidStatus::Stopped | GidStatus::Error | GidStatus::Removed
        )
    }

	/// Limited for streaming on WS
    pub fn is_streaming(&self) -> bool {
        matches!(self.status, GidStatus::Active | GidStatus::Seeding)
    }

	#[allow(unused)]
    pub fn is_trackable(&self) -> bool {
        matches!(self.status, GidStatus::Active | GidStatus::Seeding | GidStatus::Waiting | GidStatus::Paused)
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
        self.num_pieces = tele.num_pieces.or_else(|| self.num_pieces.take());
        self.piece_length = tele.piece_length.or_else(|| self.piece_length.take());
        self.verified_length = tele.verified_length.or_else(|| self.verified_length.take());
        self.verify_integrity_pending = tele.verify_integrity_pending.or(self.verify_integrity_pending.clone());
        self.seeder = tele.seeder.or(self.seeder.clone());
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
    pub fn extract(aria2_res: Aria2Res, gid: &str) -> ItemMetaData {
        let mut info = aria2_res;
		let source_uri = info.files.first_mut().and_then(|file| {
			file.uris
				.iter_mut()
				.find(|u| !u.uri.is_empty())
				.map(|u| std::mem::take(&mut u.uri))
		});

		let fallback_name = info.files.first().and_then(|file| {
            if file.path.is_empty() {
                None
            } else {
                Path::new(&file.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
            }
        });

		let file_paths: Vec<String> = info
            .files
            .into_iter()
            .filter(|f| !f.path.is_empty())
            .map(|f| f.path)					// Direct ownership transfer
            .collect();

        let files_json = serde_json::to_string(&file_paths).ok();

        let is_magnet = source_uri.as_deref().map_or(false, |uri| uri.starts_with("magnet:"));
        let is_torrent = info.bittorrent.is_some() || info.info_hash.is_some() || is_magnet;

        let is_seeder = info.seeder.as_deref().and_then(|s| match s {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        });

        let status = match info.status.as_str() {
            "active" => {
                if (is_torrent
                    && info.completed_length == info.total_length
                    && info.total_length != "0")
                    && is_seeder.is_some_and(|s| s)
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

        let mut bittorrent_info_name = None;

		let torrent = info.bittorrent.map(|mut bt| {
            if let Some(ref mut bt_info) = bt.info {
                bittorrent_info_name = std::mem::take(&mut bt_info.name);
            }

            TorrentMagnetMeta {
                announce_list: bt.announce_list.unwrap_or_default(),
                comment: bt.comment,
                creation_date: bt.creation_date,
                mode: bt.mode,
                name: bittorrent_info_name.clone(),
            }
        });

        let name = if is_torrent { bittorrent_info_name } else { fallback_name };
        let torrent_str = serde_json::to_string(&torrent).unwrap_or_default();

        ItemMetaData {
            gid: gid.to_string(),
            name,
            kind,
			category: None,										// Getting updated while adding in dl
            status,
            dir: Some(info.dir),
            files: files_json,
            total_length: Some(info.total_length),
            completed_length: Some(info.completed_length),
            upload_length: Some(info.upload_length),
            source_uri,
            info_hash: info.info_hash,
            seeder: info.seeder,
            content_hash: None,									// Later gets updated
            error_code: info.error_code.and_then(|c| c.parse().ok()),
            error_message: info.error_message,
            torrent: Some(torrent_str),
            options: None,										// Later gets updated
            is_resume_supported: None,							// Getting updated while add in dl
            num_pieces: info.num_pieces,
            piece_length: info.piece_length,
            verified_length: info.verified_length,
            verify_integrity_pending: info.verify_integrity_pending,
            created_at: None,
            completed_at: None,
            user_id: "".to_string(),
        }
    }

    fn skeleton(gid: &str) -> ItemMetaData {
        ItemMetaData {
			gid: gid.to_string(),
            name: Some(gid.to_string()),						// Keep `gid` as `name` substitute
			kind: Kind::Unknown,
			category: None,
			status: GidStatus::Waiting,
			dir: None,
			files: None,
			total_length: Some("0".into()),
			completed_length: Some("0".into()),
			upload_length: Some("0".into()),
			source_uri: None,
			info_hash: None,
			seeder: Some(false.to_string()),                    // Initiate with "false"
			content_hash: None,
			error_code: None,
			error_message: None,
			torrent: None,
			options: None,
			is_resume_supported: Some(false.to_string()),		// Initiate with "false"
			num_pieces: Some("0".into()),
			piece_length: Some("0".into()),
			verified_length: Some("0".into()),
			verify_integrity_pending: Some(false.to_string()), // Initiate with "false"
            created_at: None,
            completed_at: None,
            user_id: "".to_string(),
        }
    }
}

