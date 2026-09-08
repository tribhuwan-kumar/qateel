use serde_json::Value;
use chrono::NaiveDateTime;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use std::sync::{atomic::{AtomicU64}, Arc };
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};

/*
	Those value are getting serialize are in `camelCase`

*/
/// For aria2 worker
#[derive(Debug, Clone, Serialize)]
pub struct Aria2JsonRpcReq {
    pub id: String,
    pub jsonrpc: String,
    pub method: String,
    pub params: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aria2JsonRpcResp {
    pub id: Option<String>,
    pub method: Option<String>,
    pub params: Option<Vec<Value>>,
    pub result: Option<Value>,
    pub error: Option<Value>,
}

#[derive(Debug)]
pub enum Command {
     Call {
        method: String,
        params: Vec<Value>,
        reply: oneshot::Sender<Result<Value, String>>,
    },
}

#[derive(Clone)]
pub struct Aria2Client {
    pub command_tx: mpsc::Sender<Command>,
    /// Broadcast channel for aria2 events
    pub events: broadcast::Sender<Aria2JsonRpcResp>,
}

#[derive(Debug)]
pub struct Aria2Worker {
    pub url: String,
    pub secret: String,
    pub id_counter: AtomicU64,
    /// Maps request id to reply channel
    pub pending_requests: Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>>,
}


#[derive(Deserialize, Debug)]
pub struct GidReq {
    pub gid: String,
}

/// Pause multiple ddl
#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct PauseReq {
    pub gids: Option<Vec<String>>,
    pub pause_all: Option<bool>,
    pub force_pause: Option<bool>,
    pub force_pause_all: Option<bool>,
}

/// Resume multiple ddl from pause
#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct ResumeReq {
    pub gids: Option<Vec<String>>,
    pub resume_all: Option<bool>,
}

/// Stop multiple dl
#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct StopReq {
    pub gids: Option<Vec<String>>,
    pub force_stop: Option<bool>,
}

/// Retry request
#[derive(Deserialize, Debug)]
pub struct RetryReq {
    pub gid: String,
}

/// Wrapper for aria2 options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aria2Options(pub serde_json::Map<String, Value>);

/// Set gid specific options
#[derive(Deserialize, Debug)]
pub struct SetOptionReq {
    pub gid: String,
    pub options: Aria2Options,
}

/// Set global options
#[derive(Deserialize, Debug)]
pub struct SetGlobalOptionReq {
    pub options: Aria2Options
}

/// Set cmd options
#[derive(Deserialize)]
pub struct SetCmdOptionsReq {
    pub options: Aria2Options,
}

/// Seed back
#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SeedReq {
    pub gid: String,
    pub seed_all: Option<bool>,
}

/// Torrent Items
#[derive(Deserialize, Debug)]
pub struct TorrentItem {
    pub torrent: String,
    #[serde(default)]
    pub options: Option<Value>,
}

/// Add multiple torrents
#[derive(Deserialize, Debug)]
pub struct BatchAddTorrentReq {
    pub torrents: Vec<TorrentItem>,
}

// Delete from history
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveHisReq {
    pub gids: Vec<String>,
    pub delete_file: bool,
}

/// Sort in get history
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SortBy {
    Size,
    Name,
	Time,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Order {
	Asc,
	Desc
}

/// Get history pagination
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginationQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub sort_by: Option<SortBy>,
    pub order: Option<Order>,
}

/// Search
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    #[serde(default, alias = "q", alias = "query", alias = "search", alias = "term")]
    pub q: Option<String>,
    #[serde(default, alias = "type", alias = "kind")]
    pub file_type: Option<String>,
    #[serde(default, alias = "ext", alias = "extension")]
    pub extension: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub per_page: Option<u32>,
}

/// Uri metadata
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UriMeta {
    pub name: String,
    pub resumable: bool,
	pub total_length: String,
}

/// Accept URIs
#[derive(Deserialize, Debug)]
pub struct UriInfoReq {
    pub uris: Vec<String>,
}

/// Info of conflicted dl
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateConflict {
    pub gid: Option<String>,
	pub status: Option<GidStatus>,
    pub name: String,
	pub message: String,
	pub conflict_type: String,					/*	"hash", "uri", "file", or "untrackedFile", i'm tired of making enums*/
}

/// Uri info res
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UriInfoRes {
    pub uri: String,
    pub name: String,
    pub total_length: String,
    pub is_resume_supported: bool,
    pub is_magnet: bool,
    pub is_ftp: bool,
    pub is_sftp: bool,
    pub is_duplicate: bool,
    pub conflict: Option<DuplicateConflict>,		// Reusing your existing struct
	pub probe_error: Option<String>,				// "No internet connection" or any probing error!!
}

// Adding uri in queue
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddUriTask {
    pub uri: String,
    pub name: String,
    pub total_length: String,
    pub is_resume_supported: bool,
    pub options: Option<serde_json::Map<String, Value>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddUriReq {
	pub tasks: Vec<AddUriTask>,
}

/// If name has conflict
#[derive(Deserialize, Debug)]
pub struct CheckNameReq {
    pub name: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CheckNameRes {
    pub is_conflict: bool,
    pub msg: Option<String>,
}

/// Move dl up and down in aria2 queue
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MoveReq {
    pub gid: String,
    /// Relative change (+1, -1) or absolute index
    pub pos: i32,
    /// "POS_SET", "POS_CUR", "POS_END"
    pub how: String,
}

///
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChangeUriReq {
    pub gid: String,
	pub uri: String,
}

/// Torrent and Magnet metadata
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TorrentMagnetMeta {
    pub announce_list: Vec<Vec<String>>,
    pub comment: Option<String>,
    pub creation_date: Option<i64>,
    pub mode: Option<BitTorrentMode>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum GidStatus {
    /// onDownloadError
    Error,
    /// onDownloadPause
    Paused,
    /// onDownloadStart, `aria2.tellActive`
    Active,
    /// onDownloadStart, `aria2.tellActive`
    Waiting,
    /// onDownloadStop also file is deleted
    Removed,
    /// onDownloadStop but the `file-allocation` exists
    Stopped,
    /// When `is_torrent` and download is `complete` & `seeder`, then we're seeding the torrent
    Seeding,
    /// onDownloadComplete or onBtDownloadComplete
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Metalink is just uri
    Uri,
    /// Magnet too
    Torrent,
    /// For others
    Unknown,
}

impl From<String> for Kind {
    fn from(value: String) -> Self {
        match value.to_lowercase().as_str() {
            "uri" => Kind::Uri,
            "torrent" => Kind::Torrent,
            _ => Kind::Unknown,
        }
    }
}

impl From<&str> for Kind {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "uri" => Kind::Uri,
            "torrent" => Kind::Torrent,
            _ => Kind::Unknown,
        }
    }
}

/// Emits on events
/// These structs are defined based on the `1DM Manager android`, for saving in DB
#[derive(Serialize, Debug, Clone, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ItemMetaData {
    pub gid: String,
    /// DDl actual name
    pub name: Option<String>,
    /// what type of download
    pub kind: Kind,
    /// What's the status
    pub status: GidStatus,
    /// Directory where its being downloaded
    pub dir: Option<String>,
    /// Acutal path of the content being downloaded
    /// It can be multiple for torrents; storing json array
    pub files: Option<String>,
    /// Total length of content it gonna download
    pub total_length: Option<String>,
    /// How much its completed
    pub completed_length: Option<String>,
    /// How much its uploaded
    pub upload_length: Option<String>,
    /// Source only for uri based ddl `Download link`
    pub source_uri: Option<String>,
    /// Hash of the only (magnets or torrent)
    pub info_hash: Option<String>,
    /// If it was a seeder; so on restart start as seeder
    pub seeder: Option<bool>,
    /// Hash for anything
    pub content_hash: Option<String>,
    /// If the download failed for some reason
    pub error_code: Option<i64>,
    /// Message for that error code
    pub error_message: Option<String>,
    /// torrent/magnet metadata
    pub torrent: Option<sqlx::types::Json<TorrentMagnetMeta>>,
	/// GID specific options
	pub options: Option<String>,
    /// Is resume supported??
    pub is_resume_supported: Option<bool>,
    /// When does that dl created
    pub connections: Option<String>,
	/// Number of pieces
    pub num_pieces: Option<String>,
	/// Number of seeders
    pub num_seeders: Option<String>,
	/// A piece length
    pub piece_length: Option<String>,
	/// Hash checked length
    pub verified_length: Option<String>,
	/// Hash check pending
    pub verify_integrity_pending: Option<bool>,
    /// When does that dl created
    pub created_at: Option<NaiveDateTime>,
    /// When does that dl completed
    pub completed_at: Option<NaiveDateTime>,
    /// Belonging, only server side
    #[serde(skip)]
    pub user_id: String,
}

/// Stateless vars with including basic info
/// This will get streamed via ws
#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TickTask {
    pub gid: String,
    pub name: Option<String>,
    pub kind: Kind,
    pub status: GidStatus,
	pub dir: Option<String>,
    pub source_uri: Option<String>,
	pub is_resume_supported: Option<bool>,
	/// JSON array string
    pub files: Option<String>,
	/// Values those need streaming updates
	pub upload_length: String,
    pub completed_length: String,
	pub total_length: String,
	pub verified_length: Option<String>,
	pub connections: Option<String>,
    pub download_speed: String,
    pub upload_speed: String,
	pub num_seeders: Option<String>,
	pub seeder: Option<bool>,
	pub verify_integrity_pending: Option<bool>,
}

/// Ddl stats
#[derive(Clone, Serialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum DdlWsMessage {
    #[serde(rename = "tick")]
    Tick {
        #[serde(skip)]
        user_id: String,
        tasks: Vec<TickTask>,
    },
    #[serde(rename = "event")]
    Event {
        #[serde(skip)]
        user_id: String,
        data: ItemMetaData,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GlobalStat {
    pub download_speed: String,
    pub upload_speed: String,
    pub num_active: String,
    pub num_waiting: String,
    pub num_stopped: String,
    pub num_stopped_total: String,
}

/// Global stats emited by ws
#[derive(Serialize, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum GlobalStatWsMessage {
    #[serde(rename = "global")]
    Global {
        #[serde(skip)]
        user_id: String,								// Ulid string
        data: GlobalStat,
    },
}

/*
  * Aria2 rpc json response, mapping tellStatus
  * For `uris`, `torrents`, `magnets` initial and after (handling following and followed_by)
*/
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UriStatus {
    Used,
    Waiting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitTorrentInfo {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BitTorrentMode {
    Single,
    Multi
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aria2Uri {
    pub uri: String,
    pub status: UriStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Aria2File {
    pub index: String,
    pub path: String,
    pub length: String,
    pub completed_length: String,
    pub selected: String,
    pub uris: Vec<Aria2Uri>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitTorrent {
    pub announce_list: Option<Vec<Vec<String>>>,
    pub comment: Option<String>,
    pub creation_date: Option<i64>,
    pub mode: Option<BitTorrentMode>,
    pub info: Option<BitTorrentInfo>,
}

/// Actual mirror of `aria2.tellStatus`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Aria2Res {
    pub gid: String,
    pub status: String,
    pub dir: String,
    pub download_speed: String,
    pub upload_speed: String,
    pub total_length: String,
    pub completed_length: String,
    pub upload_length: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub bitfield: Option<String>,
    pub info_hash: Option<String>,
    pub bittorrent: Option<BitTorrent>,
    pub files: Vec<Aria2File>,
    pub connections: Option<String>,
    pub num_pieces: Option<String>,
    pub num_seeders: Option<String>,
    pub piece_length: Option<String>,
    pub following: Option<String>,
    pub followed_by: Option<Vec<String>>,
    pub belongs_to: Option<String>,
    /// Res is string: "true" | "false"
    pub seeder: Option<String>,
    pub verified_length: Option<String>,
    /// Res is string: "true" | "false"
    pub verify_integrity_pending: Option<String>
}

/// Categorize items
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemCategory {
    Everything,
    Torrent,
    Music,
    Photos,
    Programs,
    Videos,
    Subtitles,
    Compressed,
    Others,
}

#[derive(Serialize, Default, Clone)]
pub struct StatusCounts {
    pub all: usize,
    pub active: usize,
    pub paused: usize,
    pub waiting: usize,
    pub complete: usize,
    pub removed: usize,
    pub stopped: usize,
    pub error: usize,
    pub seeding: usize,
}

#[derive(Serialize, Default, Clone)]
pub struct CategoryCounts {
    pub everything: usize,
    pub torrent: usize,
    pub music: usize,
    pub photos: usize,
    pub programs: usize,
    pub videos: usize,
    pub subtitles: usize,
    pub compressed: usize,
    pub others: usize,
}

#[derive(Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ItemCoundStatsRes {
    pub by_status: StatusCounts,
    pub by_category: CategoryCounts,
}

#[derive(sqlx::FromRow)]
pub struct DownloadRowForItemCount {
    pub name: Option<String>,
    pub status: String,
    pub kind: String,
}

/// Torrent or Magnet probing metadata
#[derive(Debug, Deserialize)]
pub struct TorrentMeta {
    pub info: TorrentInfo,
}

#[derive(Debug, Deserialize)]
pub struct TorrentFile {
    pub length: u64,
	pub path: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct TorrentInfo {
    pub name: Option<String>,
    pub private: Option<u8>,
    pub length: Option<u64>,
    pub files: Option<Vec<TorrentFile>>,
}

impl TorrentInfo {
    /// Returns total size in bytes
	/// Single file or multi file torrents
    pub fn total_size(&self) -> u64 {
        if let Some(len) = self.length {
            len
        } else if let Some(files) = &self.files {
            files.iter().map(|f| f.length).sum()
        } else {
            0
        }
    }

    pub fn is_private(&self) -> bool {
        self.private.unwrap_or(0) == 1
    }
}

#[derive(Debug)]
pub struct ParsedTorrent {
    pub name: String,
    pub total_size: u64,
    pub is_private: bool,
    pub info_hash: String,
}

#[derive(Deserialize)]
pub struct TorsProbeReq {
    pub torrents: Vec<String>,				// Array of Base64 encoded torrent strings
}

#[derive(Serialize, Debug)]
pub struct TorsProbeRes {
    pub index: usize,
    pub name: Option<String>,
    pub info_hash: Option<String>,
    pub total_length: Option<String>,
	pub is_duplicate: Option<bool>,
	pub is_private: Option<bool>,
    pub conflict: Option<DuplicateConflict>,
    pub probe_error: Option<String>,				 // Any probe error
}

