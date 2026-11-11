import { type SortBy, type Order } from '$lib/sort';

/** Sub structures for download info */
export interface Aria2Uri {
  uri: string;
  status: "used" | "waiting";
}

export type Kind =
  "uri" |
  "torrent" |
  "unknown";

export type GidStatus =
  "error" |
  "paused" |
  "active" |
  "waiting" |
  "removed" |
  "stopped" |
	"seeding" |
  "complete";

export type Category =
  "torrent" |
  "music" |
  "photos" |
  "programs" |
  "videos" |
  "subtitles" |
	"compressed" |
  "others";

export interface StatusCounts {
    all: number;
    active: number;
    paused: number;
    waiting: number;
    complete: number;
    removed: number;
    stopped: number;
    error: number;
    seeding: number;
}

export interface CategoryCounts {
    everything: number;
    torrent: number;
    music: number;
    photos: number;
    programs: number;
    videos: number;
    subtitles: number;
    compressed: number;
    others: number;
}

export interface ItemCoundStatsRes {
    by_status: StatusCounts;
    by_category: CategoryCounts;
}

export interface TorrentMagnetMeta {
  announceList: string[][],
  comment: string | null,
  creationDate: number | null
  mode: "single" | "multi" | null;
  name: string | null
}


export interface Aria2File {
  index: string;                                  /* Aria2 returns numbers as strings •᷄_•᷅ */
  path: string;
	/* Bytes as string */
  length: string;
  completedLength: string;
  selected: Aria2Boolean;
  uris: Aria2Uri[];
}

export interface Aria2BitTorrentInfo {
  announceList?: string[][];
  comment?: string;
  creationDate?: number;
  mode?: "single" | "multi";
  info?: {
    name?: string;
  };
}

/** Result of `tellStatus` */
export interface Aria2Res {
  gid: string;
  status: GidStatus;
  totalLength: string;
  completedLength: string;
  uploadLength: string;
  bitfield?: string;
  downloadSpeed: string;
  uploadSpeed: string;
  infoHash?: string;
  numSeeders?: string;
  seeder?: string;                                  /* "true" | "false" */
  pieceLength: string;
  numPieces: string;
  connections: string;
  errorCode?: string;
  errorMessage?: string;
  followedBy?: string[];
  following?: string;
  belongsTo?: string;
  dir: string;
  files: Aria2File[];
  bittorrent?: Aria2BitTorrentInfo;
  verifiedLength?: string;
  verifyIntegrityPending?: string;                  /* "true" | "false" */
}

export interface Aria2Peer {
  peerId: string;
  ip: string;
  port: string;
  bitfield: string;
  amChoking: Aria2Boolean;                          /* "true" | "false" */
  peerChoking: Aria2Boolean;                        /* "true"  | "false" */
  downloadSpeed: string;
  uploadSpeed: string;
  seeder: Aria2Boolean;                             /* "true" | "false" */
}

export interface Aria2Server {
  index: string;
  servers: {
    uri: string;
    currentUri: string;
    downloadSpeed: string;
  }[];
}

/** Result of `getVersion` */
export interface Aria2Version {
  version: string;
  enabledFeatures: string[];
}

/** Result of `getSessionInfo` */
export interface Aria2Session {
  sessionId: string;
}

export interface PauseReq {
  gids?: string[];
  forcePause?: boolean;
}

export interface ResumeReq {
  gids: string[];
}

export interface StopReq {
  gids?: string[];
  forceStop?: boolean;
}

export interface SearchParams {
  q?: string | null;
  fileType?: string | null;
  extension?: string | null;
  page?: number;
  perPage?: number;
}

export interface SearchResponse {
  data: ItemMetaData[];
  meta: {
    page: number;
    perPage: number;
    total: number;
  };
}

export interface HistoryQueryParams {
	page?: number;
	limit?: number;
	sortBy?: SortBy;
	order?: Order;
	status?: string;    // `GidStatus` with "all"
	category?: string;  // `Category` with "everything"
}

/** BOOM everthing is string */
export interface ItemMetaData {
  gid: string;
  name: string | null;
  kind: Kind,
	category: Category | null;
  status: GidStatus;
  dir: string | null;
  files: string | null;                                 /* json string */
  totalLength: string | null;
  completedLength: string | null;
  uploadLength: string | null;
  sourceUri: string | null;
  infoHash: string | null;
  seeder: string | null;														/* string bool */
  contentHash: string | null;
  errorCode: number | null;
  errorMessage: string | null;
  torrent: TorrentMagnetMeta | null;
	options: Partial<Aria2Options> | null;
  isResumeSupported: string | null;									/* string bool */
	numPieces: string | null;
	pieceLength: string | null;
	verifiedLength: string | null;
	verifyIntegrityPending: string | null;					/* string bool */
  createdAt: string | null;
  completedAt: string | null;
  /**
		Merging these values manually, from tick ws
	*/
  downloadSpeed?: string;
  uploadSpeed?: string;
	numSeeders?: string | null;
	connections?: string | null;
}

/** Instant data */
export interface TickTask {
  gid: string;
  name: string | null;
  kind: Kind;
  status: GidStatus;
	dir: string | null;
  sourceUri: string | null;
	isResumeSupported: string | null;				/* string bool   */
	files: string | null;
	uploadLength: string;
	completedLength: string;
	totalLength: string;
	verifiedLength: string | null;
  connections: string | null;
	downloadSpeed: string;
	uploadSpeed: string;
  numSeeders: string | null;
	seeder: string | null;									/* string bool   */
  verifyIntegrityPending: string | null;	/* string bool   */
}

/// Coming over websocket
export type DlWsMessage =
  | { type: "tick"; tasks: TickTask[] }
  | { type: "event"; data: ItemMetaData[] };

/** History api response */
export interface HistoryResponse {
  data: ItemMetaData[];
  meta: {
    currentPage: number;
    perPage: number;
    totalItems: number;
    totalPages: number;
    hasMore: boolean,
  };
}

/// Result of `getGlobalStat`
export interface GlobalStat {
  downloadSpeed: string;
  numActive: string;
  numStopped: string;
  numStoppedTotal: string,
  numWaiting: string;
  uploadSpeed: string,
}


/// Global stat websocket
export type GlobalStatWsMessage =
  | { type: "global"; data: GlobalStat };

export interface AddUriReq {
  tasks: AddUriTask[];
}

export interface AddUriTask {
  uri: string;
  name: string;
  totalLength: string;
  isResumeSupported: string;				/* string bool   */
  options?: Partial<Aria2Options>;
}

export interface UriInfoReq {
  uris: string[];
}

export interface DuplicateConflict {
	name: string,
	gid: string | null,
	message: string | null,
	status: GidStatus | null,
	conflictType: "hash" | "uri" | "file" | "untrackedFile" | null;
}

export interface UriInfoRes {
  uri: string;
  name: string;
  totalLength: string;
  isResumeSupported: string;			/* string bool   */
  isMagnet: boolean;
  isFtp: boolean;
  isSftp: boolean;
  isDuplicate: boolean;
  conflict: DuplicateConflict | null,
  probeError: string | null;											// Any error
}

/** Aria2 passes string bool */
type Aria2Boolean = "true" | "false";
type Aria2FtpType = "binary" | "ascii";
type Aria2ProxyMethod = "get" | "tunnel";
type Aria2BtMinCryptoLevel = "plain" | "arc4";
type Aria2DownloadResult = "default" | "full" | "hide";
type Aria2UriSelector = "feedback" | "inorder" | "adaptive";
type Aria2FileAllocation = "none" | "prealloc" | "trunc" | "falloc";
type Aria2LogLevel = "debug" | "info" | "notice" | "warn" | "error";
type Aria2TlsVersion = "TLSv1.0" | "TLSv1.1" | "TLSv1.2" | "TLSv1.3";
type Aria2EventPoll = "epoll" | "kqueue" | "port" | "poll" | "select";
type Aria2MetalinkPreferredProtocol = "none" | "http" | "https" | "ftp";
type Aria2StreamPieceSelector = "default" | "inorder" | "random" | "geom";

/**
  * All available options from Input file
  * Ref: https://aria2.github.io/manual/en/html/_sources/aria2c.rst.txt
*/
export interface Aria2InputFile {
  "all-proxy"?: string,
  "all-proxy-passwd"?: string,
  "all-proxy-user"?: string,
  "allow-overwrite"?: Aria2Boolean;
  "allow-piece-length-change"?: Aria2Boolean;
  "always-resume"?: Aria2Boolean;
  "async-dns"?: Aria2Boolean;
  "auto-file-renaming"?: Aria2Boolean;
  "bt-enable-hook-after-hash-check"?: Aria2Boolean;
  "bt-enable-lpd"?: Aria2Boolean;
  "bt-exclude-tracker"?: string,
  "bt-external-ip"?: string,
  "bt-force-encryption"?: Aria2Boolean;
  "bt-hash-check-seed"?: Aria2Boolean;
  "bt-load-saved-metadata"?: Aria2Boolean;
  "bt-max-peers"?: string,
  "bt-metadata-only"?: Aria2Boolean;
  "bt-min-crypto-level"?: Aria2BtMinCryptoLevel;
  "bt-prioritize-piece"?: string,
  "bt-remove-unselected-file"?: Aria2Boolean;
  "bt-request-peer-speed-limit"?: string,
  "bt-require-crypto"?: Aria2Boolean;
  "bt-save-metadata"?: Aria2Boolean;
  "bt-seed-unverified"?: Aria2Boolean;
  "bt-stop-timeout"?: string,
  "bt-tracker"?: string,
  "bt-tracker-connect-timeout"?: string,
  "bt-tracker-interval"?: string,
  "bt-tracker-timeout"?: string,
  "check-integrity"?: Aria2Boolean;
  "checksum"?: string,
  "conditional-get"?: Aria2Boolean;
  "connect-timeout"?: string,
  "content-disposition-default-utf8"?: Aria2Boolean;
  "continue"?: Aria2Boolean;
  "dir"?: string,
  "dry-run"?: Aria2Boolean,
  "enable-http-keep-alive"?: Aria2Boolean;
  "enable-http-pipelining"?: Aria2Boolean;
  "enable-mmap"?: Aria2Boolean;
  "enable-peer-exchange"?: Aria2Boolean;
  "file-allocation"?: Aria2FileAllocation;
  "follow-metalink"?: "true" | "false" | "mem";
  "follow-torrent"?: "true" | "false" | "mem";
  "force-save"?: Aria2Boolean;
  "ftp-passwd"?: string,
  "ftp-pasv"?: Aria2Boolean;
  "ftp-proxy"?: string,
  "ftp-proxy-passwd"?: string,
  "ftp-proxy-user"?: string,
  "ftp-reuse-connection"?: Aria2Boolean;
  "ftp-type"?: Aria2FtpType;
  "ftp-user"?: string,
  "gid"?: string,
  "hash-check-only"?: Aria2Boolean;
  "header"?: string | string[];
  "http-accept-gzip"?: Aria2Boolean;
  "http-auth-challenge"?: Aria2Boolean;
  "http-no-cache"?: Aria2Boolean;
  "http-passwd"?: string,
  "http-proxy"?: string,
  "http-proxy-passwd"?: string,
  "http-proxy-user"?: string,
  "http-user"?: string,
  "https-proxy"?: string,
  "https-proxy-passwd"?: string,
  "https-proxy-user"?: string,
  "index-out"?: string | string[];
  "lowest-speed-limit"?: string,
  "max-connection-per-server"?: string,
  "max-download-limit"?: string,
  "max-file-not-found"?: string,
  "max-mmap-limit"?: string,
  "max-resume-failure-tries"?: string,
  "max-tries"?: string,
  "max-upload-limit"?: string,
  "metalink-base-uri"?: string,
  "metalink-enable-unique-protocol"?: Aria2Boolean;
  "metalink-language"?: string,
  "metalink-location"?: string,
  "metalink-os"?: string,
  "metalink-preferred-protocol"?: Aria2MetalinkPreferredProtocol;
  "metalink-version"?: string,
  "min-split-size"?: string,
  "no-file-allocation-limit"?: string,
  "no-netrc"?: Aria2Boolean,
	"no-proxy"?: string,																						// comma separated
  "out"?: string,
  "parameterized-uri"?: Aria2Boolean,
  "pause"?: Aria2Boolean,
  "pause-metadata"?: Aria2Boolean,
  "piece-length"?: string,
  "proxy-method"?: Aria2ProxyMethod;
  "realtime-chunk-checksum"?: Aria2Boolean,
  "referer"?: string,
  "remote-time"?: string,
  "remove-control-file"?: Aria2Boolean,
  "retry-wait"?: string,
  "reuse-uri"?: Aria2Boolean,
  "rpc-save-upload-metadata"?: Aria2Boolean,
  "seed-ratio"?: string,
  "seed-time"?: string,
  "select-file"?: string,
  "split"?: string,
  "ssh-host-key-md"?: string,
  "stream-piece-selector"?: Aria2StreamPieceSelector,
  "timeout"?: string,
  "uri-selector"?: Aria2UriSelector,
  "use-head"?: Aria2Boolean,
  "user-agent"?: string,
}

/**
  * Ommiting, those options that aren't available in `aria2.ChangeOption`
  * Except for the following options, changing the other options of active download makes it restart
  * Restart itself is managed by aria2, and no user intervention is required
  // "bt-max-peers"
  // "bt-request-peer-speed-limit"
  // "bt-remove-unselected-file"
  // "force-save"
  // "max-download-limit"
  // "max-upload-limit"
  * Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.changeOption
*/
export type Aria2Options = Omit<
  Aria2InputFile,
    "dry-run"
  | "metalink-base-uri"
  | "parameterized-uri"
  | "pause"
  | "piece-length"
  | "rpc-save-upload-metadata"
>;

// Req payload
export interface ChangeOptionReq {
  gid: string;
  options: Aria2Options;
}

/**
	* Options for `aria2.changeGlobalOption`
	* Inherits all standard input file
	* Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.changeGlobalOption
*/
export interface Aria2GlobalOptions extends Omit<
	Aria2InputFile,
	"checksum" |
	"index-out" |
	"out" |
	"select-file" |
	"pause"
> {
  "bt-max-open-files"?: string;
	"download-result"?: Aria2DownloadResult;
  "keep-unfinished-download-result"?: Aria2Boolean;
  "log"?: string;
  "log-level"?: Aria2LogLevel;
  "max-concurrent-downloads"?: string;
  "max-download-result"?: string;
  "max-overall-download-limit"?: string;
  "max-overall-upload-limit"?: string;
  "optimize-concurrent-downloads"?: Aria2Boolean | string;
  "save-cookies"?: string;
  "save-session"?: string;
  "server-stat-of"?: string;
}

// Req payload
export interface ChangeGlobalOptionReq {
  options: Aria2GlobalOptions;
}

/**
 * CLI Options
 * These are the options that are not meant to be changed through JSON-RPC
 * Some of them may still be accepted by aria2 in other contexts, but they are
 * not part of the changeOption / changeGlobalOption payloads.
 */
export interface Aria2CmdOptions {
	/**
	 * keep these two aside
		// "help"?: string | true;
		// "version"?: true;
	 */

  "input-file"?: string;
  "async-dns-server"?: string;
  "save-session"?: string;
  "no-conf"?: Aria2Boolean;
  "no-file-allocation-limit"?: string;
  "auto-save-interval"?: string;
  "ca-certificate"?: string;
  "certificate"?: string;
  "check-certificate"?: Aria2Boolean;
  "conf-path"?: string;
  "console-log-level"?: Aria2LogLevel;
  "daemon"?: Aria2Boolean;
  "deferred-input"?: Aria2Boolean;
  "disable-ipv6"?: Aria2Boolean;
  "disk-cache"?: string;
  "dscp"?: string;
  "enable-color"?: Aria2Boolean;
  "event-poll"?: Aria2EventPoll;
  "human-readable"?: Aria2Boolean;
  "interface"?: string;
  "load-cookies"?: string;
  "min-tls-version"?: Aria2TlsVersion;
  "multiple-interface"?: string;
  "netrc-path"?: string;
  "no-want-digest-header"?: Aria2Boolean;
  "on-bt-download-complete"?: string;
  "on-download-complete"?: string;
  "on-download-error"?: string;
  "on-download-pause"?: string;
  "on-download-start"?: string;
  "on-download-stop"?: string;
  "quiet"?: Aria2Boolean;
  "rlimit-nofile"?: string;
  "save-not-found"?: Aria2Boolean;
  "save-session-interval"?: string;
  "server-stat-if"?: string;
  "server-stat-timeout"?: string;
  "show-console-readout"?: Aria2Boolean;
  "socket-recv-buffer-size"?: string;
  "stderr"?: Aria2Boolean;
  "stop"?: string;
  "stop-with-process"?: string;
  "summary-interval"?: string;
	"force-sequential"?: Aria2Boolean;
  "truncate-console-readout"?: Aria2Boolean;

  "bt-detach-seed-only"?: Aria2Boolean;
  "bt-lpd-interface"?: string;
  "dht-entry-point"?: string;
  "dht-entry-point6"?: string;
  "dht-file-path"?: string;
  "dht-file-path6"?: string;
  "dht-listen-addr6"?: string;
  "dht-listen-port"?: string;
  "dht-message-timeout"?: string;
  "enable-dht"?: Aria2Boolean;
  "enable-dht6"?: Aria2Boolean;
  "enable-rpc"?: Aria2Boolean;
  "listen-port"?: string;
  "metalink-file"?: string;
  "peer-agent"?: string;
  "peer-id-prefix"?: string;
  "private-key"?: string;
  "rpc-allow-origin-all"?: Aria2Boolean;
  "rpc-certificate"?: string;
  "rpc-listen-all"?: Aria2Boolean;
  "rpc-listen-port"?: string;
  "rpc-max-request-size"?: string;
  "rpc-passwd"?: string;
  "rpc-private-key"?: string;
  "rpc-secure"?: Aria2Boolean;
  "rpc-secret"?: string;
  "rpc-user"?: string;
  "torrent-file"?: string;
}

// Req payload
export interface ChangeCmdOptionReq {
  options: Aria2CmdOptions;
}

export interface SpeedRecord {
  timestamp: number;
  speed: number;
}

