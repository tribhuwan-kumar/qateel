import type { Aria2Options } from "$lib/aria2/types";

// Reuse the same icons from global settings
export type SettingsIconKey =
	| "http"
	| "link"
	| "settings"
	| "bittorrent"
	| "advanced"
	| "performance";

export type FieldType = "text" | "textarea" | "boolean" | "select" | "slider";

export interface BaseField {
	key: keyof Aria2Options;
	label: string;
	description: string;
	type: FieldType;
	showFor?: ("uri" | "torrent" | "unknown")[]; // If undefined, show for all
}

export interface TextField extends BaseField {
	type: "text";
	placeholder?: string;
}

export interface TextAreaField extends BaseField {
	type: "textarea";
	placeholder?: string;
	rows?: number;
}

export interface BooleanField extends BaseField {
	type: "boolean";
}

export interface SelectField extends BaseField {
	type: "select";
	options: readonly string[];
}

export interface RangeField extends BaseField {
	type: "slider";
	min: number;
	max: number;
	step: number;
	unit?: string;
	encode?: (v: number) => string;
	decode?: (v: string | undefined) => number;
	format?: (v: number) => string;
}

export type AnyField =
	| TextField
	| RangeField
	| SelectField
	| BooleanField
	| TextAreaField;

export interface OptionSection {
	id: string;
	title: string;
	description: string;
	icon: SettingsIconKey;
	fields: AnyField[];
}

// Keeping these fn aside from global options schema
export const encodeSpeed = (val: number): string => (val === 0 ? "0" : `${val}K`);
export const decodeSpeed = (val: string | undefined): number => {
	if (!val || val === "0") return 0;
	let multiplier = 1;
	const upper = val.toUpperCase();
	if (upper.endsWith("M")) multiplier = 1024;
	else if (!upper.endsWith("K")) multiplier = 1 / 1024;
	const parsed = Number(val.replace(/[^0-9.]/g, ""));
	return Number.isFinite(parsed) ? Math.round(parsed * multiplier) : 0;
};

export const formatSpeed = (val: number): string => {
	if (val === 0) return "Unlimited";
	if (val < 1024) return `${val} KiB/s`;
	return `${(val / 1024).toFixed(2).replace(/\.00$/, "")} MiB/s`;
};

export const encodeSizeMiB = (val: number): string => `${Math.max(1, Math.round(val))}M`;
export const decodeSizeMiB = (val: string | undefined, defaultVal = 1): number => {
	if (!val || val === "0") return defaultVal;
	let multiplier = 1;
	const upper = val.toUpperCase();
	if (upper.endsWith("K")) multiplier = 1 / 1024;
	else if (!upper.endsWith("M")) multiplier = 1 / (1024 * 1024);
	const parsed = Number(val.replace(/[^0-9.]/g, ""));
	return Number.isFinite(parsed) ? parsed * multiplier : defaultVal;
};

export const formatSizeMiB = (val: number): string =>
	`${val < 1 ? val.toFixed(2) : Math.round(val)} MiB`;

// GID specific options schema
export const gidOptionsSections: OptionSection[] = [
	{
		id: "basic",
		title: "Basic options",
		description: "Core configurations like names, directories, and resume behaviors.",
		icon: "settings",
		fields: [
			{
				key: "dir",
				label: "dir",
				type: "text",
				description: "The directory to store the downloaded file.",
				placeholder: ""
			},
			{
				key: "out",
				label: "out",
				type: "text",
				description: "The file name of the downloaded file. (Ignored for Torrents)",
				placeholder: "",
				showFor: ["uri", "unknown"]
			},
			{
				key: "continue",
				label: "continue",
				type: "boolean",
				description: "Continue downloading a partially downloaded file."
			},
			{
				key: "always-resume",
				label: "always-resume",
				type: "boolean",
				description: "Always resume download. Fails if resume is not supported."
			},
			{
				key: "auto-file-renaming",
				label: "auto-file-renaming",
				type: "boolean",
				description: "Append .1, .2 to duplicate files instead of failing.",
				showFor: ["uri", "unknown"]
			},
			{
				key: "allow-overwrite",
				label: "allow-overwrite",
				type: "boolean",
				description: "Restart download from scratch if the file already exists.",
				showFor: ["uri", "unknown"]
			},
			{
				key: "check-integrity",
				label: "check-integrity",
				type: "boolean",
				description: "Verify file integrity using piece hashes."
			},
			{
				key: "checksum",
				label: "checksum",
				type: "text",
				description: "Verify file using hash",
				placeholder: "sha-256=...",
				showFor: ["uri", "unknown"]
			},
			{
				key: "select-file",
				label: "select-file",
				type: "text",
				description: "Comma-separated list of file indices to download (e.g. 1,4-5).",
				placeholder: "1,4-5",
				showFor: ["torrent", "unknown"]
			},
			{
				key: "index-out",
				label: "index-out",
				type: "text",
				description: "Specify output file name for specific index (e.g. 1=file.txt).",
				placeholder: "1=custom.txt",
				showFor: ["torrent", "unknown"]
			}
		]
	},
	{
		id: "limits",
		title: "Limits & Connections",
		description: "Configure speeds, retries, and concurrent connections.",
		icon: "performance",
		fields: [
			{
				key: "split",
				label: "split",
				type: "slider",
				description: "Download a file using N connections.",
				min: 1,
				max: 32,
				step: 1,
				showFor: ["uri", "unknown"]
			},
			{
				key: "min-split-size",
				label: "min-split-size",
				type: "slider",
				description: "Minimum size required to split a file.",
				min: 1,
				max: 1024,
				step: 1,
				encode: encodeSizeMiB,
				decode: (val) => decodeSizeMiB(val, 20),
				format: formatSizeMiB,
				showFor: ["uri", "unknown"]
			},
			{
				key: "max-connection-per-server",
				label: "max-connection-per-server",
				type: "slider",
				description: "Maximum number of connections to a single server.",
				min: 1,
				max: 16,
				step: 1,
				showFor: ["uri", "unknown"]
			},
			{
				key: "max-download-limit",
				label: "max-download-limit",
				type: "slider",
				description: "Per-download maximum download speed limit.",
				min: 0,
				max: 16384,
				step: 4,
				encode: encodeSpeed,
				decode: decodeSpeed,
				format: formatSpeed
			},
			{
				key: "max-upload-limit",
				label: "max-upload-limit",
				type: "slider",
				description: "Per-download maximum upload speed limit.",
				min: 0,
				max: 16384,
				step: 4,
				encode: encodeSpeed,
				decode: decodeSpeed,
				format: formatSpeed,
				showFor: ["torrent", "unknown"]
			},
			{
				key: "lowest-speed-limit",
				label: "lowest-speed-limit",
				type: "slider",
				description: "Drop connections if speed falls below this limit.",
				min: 0,
				max: 4096,
				step: 4,
				encode: encodeSpeed,
				decode: decodeSpeed,
				format: formatSpeed,
				showFor: ["uri", "unknown"]
			},
			{
				key: "max-tries",
				label: "max-tries",
				type: "slider",
				description: "Maximum number of connection retry attempts.",
				min: 0,
				max: 100,
				step: 1,
				format: (v) => (v === 0 ? "Unlimited" : `${v}`)
			},
			{
				key: "retry-wait",
				label: "retry-wait",
				type: "slider",
				description: "Delay between retries in seconds.",
				min: 0,
				max: 600,
				step: 1,
				unit: "sec"
			},
			{
				key: "max-resume-failure-tries",
				label: "max-resume-failure-tries",
				type: "slider",
				description: "Resume failures allowed before restarting.",
				min: 0,
				max: 100,
				step: 1
			},
			{
				key: "connect-timeout",
				label: "connect-timeout",
				type: "slider",
				description: "Timeout to establish connection.",
				min: 0,
				max: 120,
				step: 1,
				unit: "sec"
			},
			{
				key: "timeout",
				label: "timeout",
				type: "slider",
				description: "Timeout for socket read/write.",
				min: 0,
				max: 600,
				step: 1,
				unit: "sec"
			}
		]
	},
	{
		id: "http-ftp",
		title: "HTTP / FTP / Proxy",
		description: "Headers, proxies, and protocol-specific authentication.",
		icon: "http",
		fields: [
			{
				key: "user-agent",
				label: "user-agent",
				type: "text",
				description: "Custom user agent string.",
				placeholder: "Mozilla/5.0...",
				showFor: ["uri", "unknown"]
			},
			{
				key: "referer",
				label: "referer",
				type: "text",
				description: "HTTP Referer header value.",
				placeholder: "https://example.com",
				showFor: ["uri", "unknown"]
			},
			{
				key: "header",
				label: "header",
				type: "textarea",
				description: "Custom HTTP headers, one per line.",
				placeholder: "Authorization: Bearer ...\nX-Custom: value",
				rows: 2,
				showFor: ["uri", "unknown"]
			},
			{
				key: "http-user",
				label: "http-user",
				type: "text",
				description: "Username for HTTP Basic authentication.",
				showFor: ["uri", "unknown"]
			},
			{
				key: "http-passwd",
				label: "http-passwd",
				type: "text",
				description: "Password for HTTP Basic authentication.",
				showFor: ["uri", "unknown"]
			},
			{
				key: "ftp-user",
				label: "ftp-user",
				type: "text",
				description: "Username for FTP connection.",
				showFor: ["uri", "unknown"]
			},
			{
				key: "ftp-passwd",
				label: "ftp-passwd",
				type: "text",
				description: "Password for FTP connection.",
				showFor: ["uri", "unknown"]
			},
			{
				key: "ftp-type",
				label: "ftp-type",
				type: "select",
				description: "Set FTP transfer type.",
				options: ["binary", "ascii"],
				showFor: ["uri", "unknown"]
			},
			{
				key: "http-no-cache",
				label: "http-no-cache",
				type: "boolean",
				description: "Send Cache-Control: no-cache header.",
				showFor: ["uri", "unknown"]
			},
			{
				key: "http-accept-gzip",
				label: "http-accept-gzip",
				type: "boolean",
				description: "Send Accept-Encoding: deflate, gzip.",
				showFor: ["uri", "unknown"]
			},
			{
				key: "enable-http-keep-alive",
				label: "enable-http-keep-alive",
				type: "boolean",
				description: "Keep HTTP/1.1 connections alive.",
				showFor: ["uri", "unknown"]
			}
		]
	},
	{
		id: "proxy",
		title: "Proxy configs",
		description: "Set proxies configurations...",
		icon: "http",
		// Show proxy for all
		fields: [
			{
				key: "all-proxy",
				label: "all-proxy",
				type: "text",
				description: "Proxy used for all protocols (http://...)."
			},
			{
				key: "all-proxy-user",
				label: "all-proxy-user",
				type: "text",
				description: "Username for the global proxy."
			},
			{
				key: "all-proxy-passwd",
				label: "all-proxy-passwd",
				type: "text",
				description: "Password for the global proxy."
			},
			{
				key: "no-proxy",
				label: "no-proxy",
				type: "text",
				description: "Comma-separated domains to bypass proxy."
			},
		]
	},
	{
		id: "torrent",
		title: "BitTorrent",
		description: "Peers, trackers, seeding, and metadata configurations.",
		icon: "bittorrent",
		fields: [
			{
				key: "bt-max-peers",
				label: "bt-max-peers",
				type: "slider",
				description: "Maximum number of peers. 0 = unlimited.",
				min: 0,
				max: 255,
				step: 1,
				format: (v) => (v === 0 ? "Unlimited" : String(v)),
				showFor: ["torrent", "unknown"]
			},
			{
				key: "bt-request-peer-speed-limit",
				label: "bt-request-peer-speed-limit",
				type: "slider",
				description: "Request more peers if active peer speed drops below this.",
				min: 0,
				max: 51200,
				step: 50,
				encode: encodeSpeed,
				decode: decodeSpeed,
				format: formatSpeed,
				showFor: ["torrent", "unknown"]
			},
			{
				key: "seed-ratio",
				label: "seed-ratio",
				type: "text",
				description: "Stop seeding after reaching this ratio (e.g. 1.0).",
				placeholder: "1.0",
				showFor: ["torrent", "unknown"]
			},
			{
				key: "seed-time",
				label: "seed-time",
				type: "slider",
				description: "Stop seeding after this many minutes.",
				min: 0,
				max: 7200,
				step: 10,
				unit: "min",
				format: (v) => (v === 0 ? "Disabled" : `${v} min`),
				showFor: ["torrent", "unknown"]
			},
			{
				key: "bt-stop-timeout",
				label: "bt-stop-timeout",
				type: "slider",
				description: "Stop BitTorrent if speed is 0 for this many seconds.",
				min: 0,
				max: 600,
				step: 1,
				unit: "sec",
				format: (v) => (v <= 1 ? "Disabled" : `${v} sec`),
				showFor: ["torrent", "unknown"]
			},
			{
				key: "bt-tracker",
				label: "bt-tracker",
				type: "textarea",
				description: "Additional Tracker URIs, one per line.",
				rows: 2,
				showFor: ["torrent", "unknown"]
			},
			{
				key: "bt-exclude-tracker",
				label: "bt-exclude-tracker",
				type: "textarea",
				description: "Tracker URIs to exclude, one per line.",
				rows: 2,
				showFor: ["torrent", "unknown"]
			},
			{
				key: "enable-peer-exchange",
				label: "enable-peer-exchange",
				type: "boolean",
				description: "Enable Peer Exchange extension (PEX).",
				showFor: ["torrent", "unknown"]
			},
			{
				key: "bt-enable-lpd",
				label: "bt-enable-lpd",
				type: "boolean",
				description: "Enable Local Peer Discovery (LPD).",
				showFor: ["torrent", "unknown"]
			},
			{
				key: "bt-force-encryption",
				label: "bt-force-encryption",
				type: "boolean",
				description: "Require BitTorrent payload encryption (arc4).",
				showFor: ["torrent", "unknown"]
			},
			{
				key: "bt-require-crypto",
				label: "bt-require-crypto",
				type: "boolean",
				description: "Reject legacy handshakes, only accept encrypted.",
				showFor: ["torrent", "unknown"]
			},
			{
				key: "bt-min-crypto-level",
				label: "bt-min-crypto-level",
				type: "select",
				description: "Minimum encryption level required.",
				options: ["plain", "arc4"],
				showFor: ["torrent", "unknown"]
			},
			{
				key: "bt-save-metadata",
				label: "bt-save-metadata",
				type: "boolean",
				description: "Save metadata as .torrent file (for Magnets).",
				showFor: ["torrent", "unknown"]
			},
			{
				key: "bt-metadata-only",
				label: "bt-metadata-only",
				type: "boolean",
				description: "Download only metadata (Magnet to Torrent).",
				showFor: ["torrent", "unknown"]
			},
			{
				key: "bt-seed-unverified",
				label: "bt-seed-unverified",
				type: "boolean",
				description: "Seed previously downloaded files without hash checking.",
				showFor: ["torrent", "unknown"]
			}
		]
	},
	// No metalink impl
	// {
	// 	id: "metalink",
	// 	title: "Metalink",
	// 	description: "Metalink handling and mirror selection.",
	// 	icon: "link",
	// 	fields: [
	// 		{
	// 			key: "follow-metalink",
	// 			label: "follow-metalink",
	// 			type: "select",
	// 			description: "How aria2 should handle metalink files.",
	// 			options: ["true", "false", "mem"],
	// 			showFor: ["uri", "unknown"]
	// 		},
	// 		{
	// 			key: "metalink-preferred-protocol",
	// 			label: "metalink-preferred-protocol",
	// 			type: "select",
	// 			description: "Preferred protocol when multiple mirrors exist.",
	// 			options: ["none", "http", "https", "ftp"],
	// 			showFor: ["uri", "unknown"]
	// 		},
	// 		{
	// 			key: "metalink-language",
	// 			label: "metalink-language",
	// 			type: "text",
	// 			description: "Preferred language in metalink metadata.",
	// 			placeholder: "en-US",
	// 			showFor: ["uri", "unknown"]
	// 		},
	// 		{
	// 			key: "metalink-location",
	// 			label: "metalink-location",
	// 			type: "text",
	// 			description: "Preferred location code for metalink mirrors.",
	// 			placeholder: "jp,us",
	// 			showFor: ["uri", "unknown"]
	// 		},
	// 		{
	// 			key: "metalink-os",
	// 			label: "metalink-os",
	// 			type: "text",
	// 			description: "Preferred OS in metalink metadata.",
	// 			placeholder: "linux",
	// 			showFor: ["uri", "unknown"]
	// 		}
	// 	]
	// },
	{
		id: "advanced",
		title: "Advanced",
		description: "File allocation, disk caching, and low-level aria2 parameters.",
		icon: "advanced",
		fields: [
			{
				key: "file-allocation",
				label: "file-allocation",
				type: "select",
				description: "Specify file allocation method.",
				options: ["none", "prealloc", "trunc", "falloc"]
			},
			{
				key: "no-file-allocation-limit",
				label: "no-file-allocation-limit",
				type: "slider",
				description: "File size limit under which allocation is skipped.",
				min: 0,
				max: 1024,
				step: 1,
				encode: encodeSizeMiB,
				decode: (val) => decodeSizeMiB(val, 5),
				format: formatSizeMiB
			},
			{
				key: "allow-piece-length-change",
				label: "allow-piece-length-change",
				type: "boolean",
				description: "Allow piece length change when resuming."
			},
			{
				key: "async-dns",
				label: "async-dns",
				type: "boolean",
				description: "Enable asynchronous DNS."
			},
			{
				key: "enable-mmap",
				label: "enable-mmap",
				type: "boolean",
				description: "Map files into memory. May reduce disk I/O."
			},
			{
				key: "max-file-not-found",
				label: "max-file-not-found",
				type: "slider",
				description: "Abort if 'file not found' is received this many times.",
				min: 0,
				max: 100,
				step: 1,
				format: (v) => (v === 0 ? "Disabled" : `${v}`),
				showFor: ["uri", "unknown"]
			},
			{
				key: "hash-check-only",
				label: "hash-check-only",
				type: "boolean",
				description: "If true, aria2 will check file hashes and exit."
			},
			{
				key: "realtime-chunk-checksum",
				label: "realtime-chunk-checksum",
				type: "boolean",
				description: "Validate chunk checksums in real-time.",
				showFor: ["uri", "unknown"]
			},
			{
				key: "uri-selector",
				label: "uri-selector",
				type: "select",
				description: "Specify URI selection algorithm.",
				options: ["inorder", "feedback", "adaptive"],
				showFor: ["uri", "unknown"]
			},
			{
				key: "stream-piece-selector",
				label: "stream-piece-selector",
				type: "select",
				description: "Specify piece selection algorithm.",
				options: ["default", "inorder", "random", "geom"]
			},
			{
				key: "force-save",
				label: "force-save",
				type: "boolean",
				description: "Save download to control file even if the download is completed."
			},
			{
				key: "no-netrc",
				label: "no-netrc",
				type: "boolean",
				description: "Disable .netrc parsing.",
				showFor: ["uri", "unknown"]
			},
			{
				key: "ssh-host-key-md",
				label: "ssh-host-key-md",
				type: "text",
				description: "Checksum of the remote SSH host key.",
				showFor: ["uri", "unknown"]
			}
		]
	}
];

