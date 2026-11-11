-- All ids' are just for db side
-- USERS
CREATE TABLE IF NOT EXISTS users (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id TEXT NOT NULL UNIQUE,                                       -- Keep ULID
  username TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL,
  role TEXT NOT NULL CHECK(role IN ('admin', 'user')) DEFAULT 'user',
  token_version INTEGER NOT NULL DEFAULT 0,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- API KEYS
-- Maybe in future integrate with Radarr :?
CREATE TABLE IF NOT EXISTS api_keys (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id TEXT NOT NULL,                                              -- Belongs to
  label TEXT NOT NULL,                                                -- Radarr or Sonarr
  key_hash TEXT NOT NULL,                                             -- Hashed api key
  prefix TEXT NOT NULL,                                               -- Some chars to display
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,

  FOREIGN KEY(user_id) REFERENCES users(user_id) ON DELETE CASCADE
);

-- DOWNLOAD HISTORY
CREATE TABLE IF NOT EXISTS download_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  gid TEXT NOT NULL UNIQUE,
  name TEXT,
  kind TEXT NOT NULL CHECK(kind IN (
    'uri',
    'torrent',
    'unknown'
  )),
	category TEXT CHECK(category IN (
    'torrent',
    'music',
    'photos',
    'programs',
    'videos',
    'subtitles',
    'compressed',
    'others'
  )),
  status TEXT NOT NULL CHECK(status IN (
    'error',
    'paused',
    'active',
    'waiting',
    'removed',
    'stopped',
    'seeding',
    'complete'
  )),
	dir TEXT,
	files TEXT CHECK(files IS NULL OR json_valid(files)),          -- Json array of file paths
	total_length TEXT,
	completed_length TEXT,
	upload_length TEXT,                                            -- For torrents seeding
	source_uri TEXT,                                               -- The uri to perform retries
	info_hash TEXT,                                                -- Hex string for torrent identification
	seeder TEXT,																									 -- BOOL
	content_hash TEXT,                                             -- Blake3 hash of the final downloaded content
	error_code INTEGER,
	error_message TEXT,
	torrent TEXT CHECK(torrent IS NULL OR json_valid(torrent)),    -- Json encoded `TorrentMagnetMeta`
	options TEXT CHECK(options IS NULL OR json_valid(options)),		 -- Json encoded `options`
	is_resume_supported TEXT,																			 -- BOOL
	num_pieces TEXT,
	piece_length TEXT,
	verified_length TEXT,
	verify_integrity_pending TEXT,																 -- BOOL
	created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
	updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
	completed_at DATETIME,
	user_id TEXT NOT NULL,                                         -- Match user ULID

	FOREIGN KEY(user_id) REFERENCES users(user_id) ON DELETE CASCADE
);

-- SETTINGS or call it OPTIONS
CREATE TABLE IF NOT EXISTS settings (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  user_id TEXT NOT NULL,                                          -- User ULID
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,

  UNIQUE(key, user_id),
  FOREIGN KEY(user_id) REFERENCES users(user_id) ON DELETE CASCADE
);

-- SOME INDEXES
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_download_history_user_id ON download_history(user_id);
CREATE INDEX IF NOT EXISTS idx_download_history_info_hash ON download_history(info_hash);
CREATE INDEX IF NOT EXISTS idx_download_history_content_hash ON download_history(content_hash);
CREATE INDEX IF NOT EXISTS idx_download_history_kind ON download_history(kind);
CREATE INDEX IF NOT EXISTS idx_download_history_status ON download_history(status);
