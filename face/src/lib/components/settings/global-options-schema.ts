import type {	Aria2GlobalOptions, Aria2CmdOptions } from "$lib/aria2/types";

import Http from "../../../assets/svgs/http.svg"
import Link from "../../../assets/svgs/link.svg"
import Settins from "../../../assets/svgs/settings.svg"
import Bittorrent from "../../../assets/svgs/bittorrent.svg"
import Advanced from "../../../assets/svgs/advanced.svg"
import Performance from "../../../assets/svgs/performance.svg"

import HttpLight from "../../../assets/svgs/http-light.svg"
import LinkLight from "../../../assets/svgs/link-light.svg"
import SettinsLight from "../../../assets/svgs/settings-light.svg"
import BittorrentLight from "../../../assets/svgs/bittorrent-light.svg"
import AdvancedLight from "../../../assets/svgs/advanced-light.svg"
import PerformanceLight from "../../../assets/svgs/performance-light.svg"

export const SettingIcons = {
	http: { light: Http, dark: HttpLight },
	link: { light: Link, dark: LinkLight },
	settings: { light: Settins, dark: SettinsLight },
	bittorrent: { light: Bittorrent, dark: BittorrentLight },
	advanced: { light: Advanced, dark: AdvancedLight },
	performance: { light: Performance, dark: PerformanceLight },
};

export type ApiTarget = "global" | "cmd" | "readonly";
export type SettingsIconKey = keyof typeof SettingIcons;
export type FieldType = "text" | "textarea" | "boolean" | "select" | "slider";

export interface BaseField {
	key: keyof Aria2GlobalOptions | keyof Aria2CmdOptions;
	label: string;
	description: string;
	target: ApiTarget;
	type: FieldType;
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

export type AnyField = TextField | TextAreaField | BooleanField | SelectField | RangeField;

export interface OptionSection {
	id: string;
	title: string;
	description: string;
	icon: SettingsIconKey;
	fields: AnyField[];
}

// Speed: Internal value = KiB/s. Payload = "K". UI = KiB/s (<1024) or MiB/s (>=1024)
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

export const formatSizeMiB = (val: number): string => `${val < 1 ? val.toFixed(2) : Math.round(val)} MiB`;

// Schema Definition
export const settingsSections: OptionSection[] = [
	{
		id: "basic",
		title: "Basic options",
		description: "Core configurations for the aria2 daemon and global behavior.",
		icon: "settings",
		fields: [
			// Keep global dir readonly
			{
				key: "dir",
				label: "dir",
				description: "Default global download directory. Only can be change from `config.toml` file",
				type: "text",
				target: "readonly",
			},
			// Editable
			{
				key: "log",
				label: "log",
				description: "Path to the aria2 log file. Empty disables logging.",
				type: "text",
				target: "global",
			},
			// session managed by qateel internally
			// {
			// 	key: "save-session",
			// 	label: "save-session",
			// 	description: "File where aria2 saves session data. This same file will be used as `input-file`",
			// 	type: "text",
			// 	target: "global",
			// },
			{
				key: "server-stat-of",
				label: "server-stat-of",
				description: "File to store server performance statistics.",
				type: "text",
				target: "global",
			},
			{
				key: "server-stat-if",
				label: "server-stat-if",
				description: "File from which server performance stats are loaded.",
				type: "text",
				target: "cmd",
			},
			{
				key: "conf-path",
				label: "conf-path",
				description: "Change the configuration file path.",
				type: "text",
				target: "cmd",
			},
			// let the session managed by Qateel
			// {
			// 	key: "input-file",
			// 	label: "input-file",
			// 	description: "Downloads URIs found in this file.",
			// 	type: "text",
			// 	target: "cmd",
			// },
			// {
			// 	key: "gid",
			// 	label: "gid",
			// 	description: "Assign specific GID.",
			// 	type: "text",
			// 	target: "global",
			// 	placeholder: "0000000000000000"
			// },
			{
				key: "optimize-concurrent-downloads",
				label: "optimize-concurrent-downloads",
				description: "Optimizes the number of concurrent downloads according to the bandwidth available. The default values (A=5, B=25) lead to using typically 5 parallel downloads on 1Mbps networks and above 50 on 100Mbps networks.",
				type: "text",
				target: "global",
				placeholder: "true or 1:5"
			},
			{
				key: "max-mmap-limit",
				label: "max-mmap-limit",
				description: "Set the maximum file size to enable mmap. (see `enable-mmap` option).",
				type: "text",
				target: "global",
				placeholder: "92233...."
			},
			{
				key: "stop-with-process",
				label: "stop-with-process",
				description: "Stop aria2 when the specified PID terminates.",
				type: "text",
				target: "cmd",
				placeholder: "PID"
			},
			{
				key: "log-level",
				label: "log-level",
				description: "Controls how much aria2 writes to the log.",
				type: "select",
				target: "global",
				options: ["debug", "info", "notice", "warn", "error"]
			},
			{
				key: "console-log-level",
				label: "console-log-level",
				description: "Log level to output to console.",
				type: "select",
				target: "cmd",
				options: ["debug", "info", "notice", "warn", "error"]
			},
			{
				key: "file-allocation",
				label: "file-allocation",
				description: "How files are allocated before download starts.",
				type: "select",
				target: "global",
				options: ["none", "prealloc", "trunc", "falloc"]
			},
			{
				key: "download-result",
				label: "download-result",
				description: "How much download result detail should be kept.",
				type: "select",
				target: "global",
				options: ["default", "full", "hide"]
			},
			{
				key: "stream-piece-selector",
				label: "stream-piece-selector",
				description: "How streaming pieces are selected.",
				type: "select",
				target: "global",
				options: ["default", "inorder", "random", "geom"]
			},
			{
				key: "uri-selector",
				label: "uri-selector",
				description: "How aria2 selects which URI to try first.",
				type: "select",
				target: "global",
				options: ["inorder", "feedback", "adaptive"]
			},
			{
				key: "event-poll",
				label: "event-poll",
				description: "Specify the method for polling events.",
				type: "select",
				target: "cmd",
				options: ["epoll", "kqueue", "port", "poll", "select"]
			},
			{
				key: "allow-overwrite",
				label: "allow-overwrite",
				description: "Allow aria2 to overwrite existing files.",
				type: "boolean",
				target: "global"
			},
			{
				key: "allow-piece-length-change",
				label: "allow-piece-length-change",
				description: "Allow piece length changes during downloads.",
				type: "boolean",
				target: "global"
			},
			{
				key: "always-resume",
				label: "always-resume",
				description: "Always resume download. Fails if resume is not possible.",
				type: "boolean",
				target: "global"
			},
			{
				key: "auto-file-renaming",
				label: "auto-file-renaming",
				description: "Automatically append numbers to duplicate files.",
				type: "boolean",
				target: "global"
			},
			{
				key: "check-integrity",
				label: "check-integrity",
				description: "Check file integrity using piece hashes after download.",
				type: "boolean",
				target: "global"
			},
			{
				key: "continue",
				label: "continue",
				description: "Continue downloading a partially downloaded file.",
				type: "boolean",
				target: "global"
			},
			{
				key: "deferred-input",
				label: "deferred-input",
				description: "Read input file URIs one by one to save memory.",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "dry-run",
				label: "dry-run",
				description: "Check if the remote file is available without downloading data.",
				type: "boolean",
				target: "global"
			},
			{
				key: "enable-color",
				label: "enable-color",
				description: "Enable color output for a terminal.",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "enable-mmap",
				label: "enable-mmap",
				description: "Use memory-mapped file I/O. This option may not work if the file space is not pre-allocated (see `file-allocation`)",
				type: "boolean",
				target: "global"
			},
			{
				key: "force-save",
				label: "force-save",
				description: "Force saving downloads in session even if completed.",
				type: "boolean",
				target: "global"
			},
			{
				key: "hash-check-only",
				label: "hash-check-only",
				description: "Only verify hashes without downloading missing pieces.",
				type: "boolean",
				target: "global"
			},
			{
				key: "human-readable",
				label: "human-readable",
				description: "Print sizes in human readable format.",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "keep-unfinished-download-result",
				label: "keep-unfinished-download-result",
				description: "Keep results for downloads that did not finish.",
				type: "boolean",
				target: "global"
			},
			{
				key: "no-conf",
				label: "no-conf",
				description: "Disable loading the aria2.conf file.",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "parameterized-uri",
				label: "parameterized-uri",
				description: "Enable parameterized URI handling.",
				type: "boolean",
				target: "global"
			},
			{
				key: "pause-metadata",
				label: "pause-metadata",
				description: "Pause subsequent downloads created by metadata.",
				type: "boolean",
				target: "global"
			},
			{
				key: "quiet",
				label: "quiet",
				description: "Make aria2 quiet (no console output).",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "realtime-chunk-checksum",
				label: "realtime-chunk-checksum",
				description: "Verify chunk checksums while downloading.",
				type: "boolean",
				target: "global"
			},
			{
				key: "remove-control-file",
				label: "remove-control-file",
				description: "Remove control file before download.",
				type: "boolean",
				target: "global"
			},
			{
				key: "reuse-uri",
				label: "reuse-uri",
				description: "Reuse URIs that were already tried.",
				type: "boolean",
				target: "global"
			},
			{
				key: "save-not-found",
				label: "save-not-found",
				description: "Save download to session even if file was 404.",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "show-console-readout",
				label: "show-console-readout",
				description: "Show progress readout in the console.",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "stderr",
				label: "stderr",
				description: "Redirect console output to stderr.",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "truncate-console-readout",
				label: "truncate-console-readout",
				description: "Truncate readout to fit in a single line.",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "force-sequential",
				label: "force-sequential",
				description: "Fetch URIs in the command-line sequentially.",
				type: "boolean",
				target: "cmd"
			},
		]
	},
	{
		id: "limits",
		title: "Limits & Performance",
		description: "Download and upload speed limits, disk allocations, and resource caps.",
		icon: "performance",
		fields: [
			{
				key: "max-concurrent-downloads",
				label: "max-concurrent-downloads",
				description: "Maximum simultaneous downloads.",
				type: "slider",
				target: "global",
				min: 1,
				max: 1000,
				step: 1
			},
			{
				key: "max-overall-download-limit",
				label: "max-overall-download-limit",
				description: "Global max download speed.",
				type: "slider",
				target: "global",
				min: 0,
				max: 16384,
				step: 4,
				encode: encodeSpeed,
				decode: decodeSpeed,
				format: formatSpeed
			},
			{
				key: "max-overall-upload-limit",
				label: "max-overall-upload-limit",
				description: "Global max upload speed.",
				type: "slider",
				target: "global",
				min: 0,
				max: 16384,
				step: 4,
				encode: encodeSpeed,
				decode: decodeSpeed,
				format: formatSpeed
			},
			{
				key: "max-download-limit",
				label: "max-download-limit",
				description: "Per-download maximum speed limit.",
				type: "slider",
				target: "global",
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
				description: "Per-download maximum upload speed limit.",
				type: "slider",
				target: "global",
				min: 0,
				max: 16384,
				step: 4,
				encode: encodeSpeed,
				decode: decodeSpeed,
				format: formatSpeed
			},
			{
				key: "lowest-speed-limit",
				label: "lowest-speed-limit",
				description: "Drop connections if speed falls below this.",
				type: "slider",
				target: "global",
				min: 0,
				max: 4096,
				step: 4,
				encode: encodeSpeed,
				decode: decodeSpeed,
				format: formatSpeed
			},
			// its playload should be in `M`
			{
				key: "piece-length",
				label: "piece-length",
				description: "Piece size used for HTTP/FTP chunking.",
				type: "slider",
				target: "global",
				min: 1,
				max: 1024,
				step: 1,
				encode: encodeSizeMiB,
				decode: (val) => decodeSizeMiB(val, 1),
				format: formatSizeMiB
			},
			{
				key: "max-download-result",
				label: "max-download-result",
				description: "Max number of completed/error results kept in memory. Specifying 0 means no download result is kept.",
				type: "slider",
				target: "global",
				min: 0,
				max: 10000,
				step: 5
			},
			// disable on `0`
			{
				key: "max-file-not-found",
				label: "max-file-not-found",
				description: "Max 404/410 failures before stopping.",
				type: "slider",
				target: "global",
				min: 0,
				max: 100,
				step: 1,
				format: (v) => (v === 0 ? "Disabled" : `${v}`)
			},
			{
				key: "max-resume-failure-tries",
				label: "max-resume-failure-tries",
				description: "Resume failures allowed before restarting. If `0`, aria2 downloads file from scratch when URIs do not support resume (see `always-resume`)",
				type: "slider",
				target: "global",
				min: 0,
				max: 100,
				step: 1
			},
			// `0` means unlimited
			{
				key: "max-tries",
				label: "max-tries",
				description: "Maximum number of connection retry attempts.",
				type: "slider",
				target: "global",
				min: 0,
				max: 100,
				step: 1,
				format: (v) => (v === 0 ? "Unlimited" : `${v}`)
			},
			{
				key: "no-file-allocation-limit",
				label: "no-file-allocation-limit",
				description: "File size limit under which allocation is skipped.",
				type: "slider",
				target: "global",
				min: 0,
				max: 1024,
				step: 1,
				encode: encodeSizeMiB,
				decode: (val) => decodeSizeMiB(val, 5),
				format: formatSizeMiB
			},
			{
				key: "retry-wait",
				label: "retry-wait",
				description: "Delay between retries in seconds.",
				type: "slider",
				target: "global",
				min: 0,
				max: 600,
				step: 1,
				unit: "sec"
			},
			{
				key: "auto-save-interval",
				label: "auto-save-interval",
				description: "Save .aria2 control file every SEC seconds. If `0` is given, a control file is not saved during download. aria2 saves a control file when it stops regardless of the value.",
				type: "slider",
				target: "cmd",
				min: 0,
				max: 600,
				step: 1,
				unit: "sec"
			},
			// {
			// 	key: "save-session-interval",
			// 	label: "save-session-interval",
			// 	description: "Save session every SEC seconds. If `0` is given, file will be saved only when aria2 exits",
			// 	type: "slider",
			// 	target: "cmd",
			// 	min: 0,
			// 	max: 300,
			// 	step: 2,
			// 	unit: "sec"
			// },
			{
				key: "server-stat-timeout",
				label: "server-stat-timeout",
				description: "Timeout to invalidate performance profile of servers.",
				type: "slider",
				target: "cmd",
				min: 0,
				max: 864000,
				step: 1,
				unit: "sec"
			},
			{
				key: "summary-interval",
				label: "summary-interval",
				description: "Interval to output download progress summary. Setting 0 suppresses the output",
				type: "slider",
				target: "cmd",
				min: 0,
				max: 600,
				step: 5,
				unit: "sec"

			},
			{
				key: "rlimit-nofile",
				label: "rlimit-nofile",
				description: "Soft limit of open file descriptors (Posix only).",
				type: "slider",
				target: "cmd",
				min: 0,
				max: 102400,
				step: 32
			},
			{
				key: "stop",
				label: "stop",
				description: "Stop aria2 after seconds has passed. 0 = disabled.",
				type: "slider",
				target: "cmd",
				min: 0,
				max: 864000,
				step: 60,
				unit: "sec",
				format: (v) => (v === 0 ? "Disabled" : `${v} sec`)
			},
		]
	},
	{
		id: "http-ftp",
		title: "HTTP & FTP",
		description: "Transport, proxy, header, and authentication related settings.",
		icon: "http",
		fields: [
			{
				key: "all-proxy",
				label: "all-proxy",
				description: "Proxy used for all protocols.",
				type: "text",
				target: "global",
			},
			{
				key: "all-proxy-user",
				label: "all-proxy-user",
				description: "Username for the global proxy.",
				type: "text",
				target: "global"
			},
			{
				key: "all-proxy-passwd",
				label: "all-proxy-passwd",
				description: "Password for the global proxy.",
				type: "text",
				target: "global",
			},
			{
				key: "http-proxy",
				label: "http-proxy",
				description: "Proxy used for HTTP downloads.",
				type: "text",
				target: "global"
			},
			{
				key: "http-proxy-user",
				label: "http-proxy-user",
				description: "Proxy user for HTTP downloads.",
				type: "text",
				target: "global"
			},
			{
				key: "http-proxy-passwd",
				label: "http-proxy-passwd",
				description: "Proxy password used for HTTP downloads.",
				type: "text",
				target: "global"
			},
			{
				key: "https-proxy",
				label: "https-proxy",
				description: "Proxy used for HTTPS downloads.",
				type: "text",
				target: "global"
			},
			{
				key: "https-proxy-user",
				label: "https-proxy-user",
				description: "Proxy user for HTTPS downloads.",
				type: "text",
				target: "global"
			},
			{
				key: "https-proxy-passwd",
				label: "https-proxy-passwd",
				description: "Proxy password used for HTTPS downloads.",
				type: "text",
				target: "global"
			},
			{
				key: "ftp-proxy",
				label: "ftp-proxy",
				description: "Proxy used for FTP downloads.",
				type: "text",
				target: "global"
			},
			{
				key: "ftp-proxy-user",
				label: "ftp-proxy-user",
				description: "Proxy user for FTP downloads.",
				type: "text",
				target: "global"
			},
			{
				key: "ftp-proxy-passwd",
				label: "ftp-proxy-passwd",
				description: "Proxy password for FTP downloads.",
				type: "text",
				target: "global"
			},
			{
				key: "http-user",
				label: "http-user",
				description: "Username for HTTP authentication.",
				type: "text",
				target: "global"
			},
			{
				key: "http-passwd",
				label: "http-passwd",
				description: "Password for HTTP authentication.",
				type: "text",
				target: "global"
			},
			{
				key: "ftp-user",
				label: "ftp-user",
				description: "Username for FTP authentication.",
				type: "text",
				target: "global"
			},
			{
				key: "ftp-passwd",
				label: "ftp-passwd",
				description: "Password for FTP authentication.",
				type: "text",
				target: "global"
			},
			{
				key: "no-proxy",
				label: "no-proxy",
				description: "Hosts that should bypass proxy settings.",
				type: "text",
				target: "global",
			},
			{
				key: "user-agent",
				label: "user-agent",
				description: "Custom user agent string.",
				type: "text",
				target: "global",
				placeholder: "Mozilla/5.0...."
			},
			{
				key: "referer",
				label: "referer",
				description: "HTTP Referer header value.",
				type: "text",
				target: "global",
				placeholder: "https://example.com"
			},
			{
				key: "save-cookies",
				label: "save-cookies",
				description: "File to save cookies to.",
				type: "text",
				target: "global",
			},
			{
				key: "load-cookies",
				label: "load-cookies",
				description: "Load Cookies from file in Mozilla/Firefox or Netscape format.",
				type: "text",
				target: "cmd",
			},
			{
				key: "netrc-path",
				label: "netrc-path",
				description: "Specify the path to the netrc file.",
				type: "text",
				target: "cmd",
			},
			{
				key: "ssh-host-key-md",
				label: "ssh-host-key-md",
				description: "Expected SSH host key fingerprint.",
				type: "text",
				target: "global",
				placeholder: "sha-1=..."
			},
			{
				key: "async-dns-server",
				label: "async-dns-server",
				description: "Comma-separated list of DNS servers.",
				type: "text",
				target: "cmd",
				placeholder: "8.8.8.8,8.8.4.4"
			},
			{
				key: "header",
				label: "header",
				description: "Custom HTTP headers, one per line.",
				type: "textarea",
				target: "global",
				rows: 2,
				placeholder: "Authorization: Bearer ..."
			},
			{
				key: "ftp-type",
				label: "ftp-type",
				description: "Transfer mode used for FTP downloads.",
				type: "select",
				target: "global",
				options: ["binary", "ascii"]
			},
			{
				key: "proxy-method",
				label: "proxy-method",
				description: "Method used when connecting through a proxy.",
				type: "select",
				target: "global",
				options: ["get", "tunnel"]
			},
			{
				key: "async-dns",
				label: "async-dns",
				description: "Resolve hostnames asynchronously.",
				type: "boolean",
				target: "global"
			},
			{
				key: "conditional-get",
				label: "conditional-get",
				description: "Use conditional HTTP requests (If-Modified-Since).",
				type: "boolean",
				target: "global"
			},
			{
				key: "content-disposition-default-utf8",
				label: "content-disposition-default-utf8",
				description: "Assume UTF-8 when parsing Content-Disposition strings.",
				type: "boolean",
				target: "global"
			},
			{
				key: "enable-http-keep-alive",
				label: "enable-http-keep-alive",
				description: "Keep HTTP/1.1 connections alive.",
				type: "boolean",
				target: "global"
			},
			{
				key: "enable-http-pipelining",
				label: "enable-http-pipelining",
				description: "Enable HTTP/1.1 pipelining.",
				type: "boolean",
				target: "global"
			},
			{
				key: "ftp-pasv",
				label: "ftp-pasv",
				description: "Use passive mode for FTP.",
				type: "boolean",
				target: "global"
			},
			{
				key: "ftp-reuse-connection",
				label: "ftp-reuse-connection",
				description: "Reuse FTP connections when possible.",
				type: "boolean",
				target: "global"
			},
			{
				key: "http-accept-gzip",
				label: "http-accept-gzip",
				description: "Send Accept-Encoding: deflate, gzip.",
				type: "boolean",
				target: "global"
			},
			{
				key: "http-auth-challenge",
				label: "http-auth-challenge",
				description: "Send HTTP auth header only when challenged.",
				type: "boolean",
				target: "global"
			},
			{
				key: "http-no-cache",
				label: "http-no-cache",
				description: "Send Cache-Control: no-cache header.",
				type: "boolean",
				target: "global"
			},
			{
				key: "no-netrc",
				label: "no-netrc",
				description: "Disable netrc support.",
				type: "boolean",
				target: "global"
			},
			{
				key: "use-head",
				label: "use-head",
				description: "Use HEAD method for the first HTTP request.",
				type: "boolean",
				target: "global"
			},
			{
				key: "no-want-digest-header",
				label: "no-want-digest-header",
				description: "Disable Want-Digest header when making requests.",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "remote-time",
				label: "remote-time",
				description: "Retrieve timestamp of the remote file.",
				type: "boolean",
				target: "global"
			},
			{
				key: "connect-timeout",
				label: "connect-timeout",
				description: "Timeout to establish connection.",
				type: "slider",
				target: "global",
				min: 0,
				max: 120,
				step: 1,
				unit: "sec"
			},
			{
				key: "timeout",
				label: "timeout",
				description: "Timeout for socket read/write.",
				type: "slider",
				target: "global",
				min: 0,
				max: 600,
				step: 1,
				unit: "sec"
			},
			{
				key: "split",
				label: "split",
				description: "Download a file using N connections.",
				type: "slider",
				target: "global",
				min: 1,
				max: 32,
				step: 1
			},
			{
				key: "max-connection-per-server",
				label: "max-connection-per-server",
				description: "Maximum number of connections to one server.",
				type: "slider",
				target: "global",
				min: 1,
				max: 32,
				step: 1
			},
			{
				key: "min-split-size",
				label: "min-split-size",
				description: "Minimum split size.",
				type: "slider",
				target: "global",
				min: 1,
				max: 1024,
				step: 1,
				encode: encodeSizeMiB,
				decode: (val) => decodeSizeMiB(val, 20),
				format: formatSizeMiB
			}
		]
	},
	{
		id: "bittorrent",
		title: "BitTorrent & DHT",
		description: "Options related to torrent downloads, peers, trackers, and seeding.",
		icon: "bittorrent",
		fields: [
			{
				key: "bt-detach-seed-only",
				label: "bt-detach-seed-only",
				description: "Exclude seed-only downloads when counting concurrent downloads.",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "bt-enable-hook-after-hash-check",
				label: "bt-enable-hook-after-hash-check",
				description: "Run hooks after hash checking completes.",
				type: "boolean",
				target: "global"
			},
			{
				key: "bt-enable-lpd",
				label: "bt-enable-lpd",
				description: "Enable Local Peer Discovery (LPD).",
				type: "boolean",
				target: "global"
			},
			{
				key: "bt-force-encryption",
				label: "bt-force-encryption",
				description: "Require BitTorrent payload encryption (arc4).",
				type: "boolean",
				target: "global"
			},
			{
				key: "bt-hash-check-seed",
				label: "bt-hash-check-seed",
				description: "Check file integrity before seeding.",
				type: "boolean",
				target: "global"
			},
			{
				key: "bt-load-saved-metadata",
				label: "bt-load-saved-metadata",
				description: "Try to read saved .torrent metadata before DHT.",
				type: "boolean",
				target: "global"
			},
			{
				key: "bt-metadata-only",
				label: "bt-metadata-only",
				description: "Download only metadata (Magnet to Torrent).",
				type: "boolean",
				target: "global"
			},
			{
				key: "bt-remove-unselected-file",
				label: "bt-remove-unselected-file",
				description: "Remove unselected files upon completion.",
				type: "boolean",
				target: "global"
			},
			{
				key: "bt-require-crypto",
				label: "bt-require-crypto",
				description: "Reject legacy handshakes, only accept encrypted.",
				type: "boolean",
				target: "global"
			},
			{
				key: "bt-save-metadata",
				label: "bt-save-metadata",
				description: "Save metadata as .torrent file (for Magnets).",
				type: "boolean",
				target: "global"
			},
			{
				key: "bt-seed-unverified",
				label: "bt-seed-unverified",
				description: "Seed previously downloaded files without hash checking.",
				type: "boolean",
				target: "global"
			},
			{
				key: "enable-dht",
				label: "enable-dht",
				description: "Enable IPv4 DHT & UDP tracker support.",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "enable-dht6",
				label: "enable-dht6",
				description: "Enable IPv6 DHT functionality.",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "enable-peer-exchange",
				label: "enable-peer-exchange",
				description: "Enable Peer Exchange extension (PEX).",
				type: "boolean",
				target: "global"
			},
			{
				key: "bt-exclude-tracker",
				label: "bt-exclude-tracker",
				description: "Tracker URIs to remove.",
				type: "text",
				target: "global",
				placeholder: "tracker1,tracker2"
			},
			{
				key: "bt-external-ip",
				label: "bt-external-ip",
				description: "External IP address to report to peers and DHT.",
				type: "text",
				target: "global",
			},
			{
				key: "bt-prioritize-piece",
				label: "bt-prioritize-piece",
				description: "Prioritize specific pieces (e.g., head/tail).",
				type: "text",
				target: "global",
				placeholder: "head=1M,tail=1M"
			},
			{
				key: "listen-port",
				label: "listen-port",
				description: "TCP port number for incoming BitTorrent connections.",
				type: "text",
				target: "cmd",
			},
			{
				key: "peer-id-prefix",
				label: "peer-id-prefix",
				description: "Specify the prefix of peer ID.",
				type: "text",
				target: "cmd",
				placeholder: "A2-1-35-0-"
			},
			{
				key: "peer-agent",
				label: "peer-agent",
				description: "Specify the string used during the bittorrent extended handshake.",
				type: "text",
				target: "cmd",
			},
			{
				key: "torrent-file",
				label: "torrent-file",
				description: "The path to the .torrent file.",
				type: "text",
				target: "cmd",
			},
			{
				key: "bt-lpd-interface",
				label: "bt-lpd-interface",
				description: "Use given interface for Local Peer Discovery.",
				type: "text",
				target: "cmd",
			},
			{
				key: "dht-listen-port",
				label: "dht-listen-port",
				description: "UDP listening port used by DHT.",
				type: "text",
				target: "cmd",
				placeholder: "6881-6999"
			},
			{
				key: "dht-file-path",
				label: "dht-file-path",
				description: "Path to IPv4 DHT routing table file.",
				type: "text",
				target: "cmd",
			},
			{
				key: "dht-file-path6",
				label: "dht-file-path6",
				description: "Path to IPv6 DHT routing table file.",
				type: "text",
				target: "cmd",
			},
			{
				key: "dht-entry-point",
				label: "dht-entry-point",
				description: "IPv4 DHT entry point node.",
				type: "text",
				target: "cmd",
			},
			{
				key: "dht-entry-point6",
				label: "dht-entry-point6",
				description: "IPv6 DHT entry point node.",
				type: "text",
				target: "cmd",
			},
			{
				key: "dht-listen-addr6",
				label: "dht-listen-addr6",
				description: "Specify address to bind for IPv6 DHT.",
				type: "text",
				target: "cmd",
				placeholder: "::"
			},
			{
				key: "bt-tracker",
				label: "bt-tracker",
				description: "Additional Tracker URIs, one per line.",
				type: "textarea",
				target: "global",
				rows: 2,
			},
			{
				key: "bt-min-crypto-level",
				label: "bt-min-crypto-level",
				description: "Minimum encryption level required.",
				type: "select",
				target: "global",
				options: ["plain", "arc4"]
			},
			{
				key: "follow-torrent",
				label: "follow-torrent",
				description: "How torrent files should be treated when added.",
				type: "select",
				target: "global",
				options: ["true", "false", "mem"]
			},
			{
				key: "bt-max-open-files",
				label: "bt-max-open-files",
				description: "Maximum number of files opened for BitTorrent.",
				type: "slider",
				target: "global",
				min: 0,
				max: 1000,
				step: 1,
			},
			{
				key: "bt-max-peers",
				label: "bt-max-peers",
				description: "Maximum number of peers per torrent. 0 = unlimited.",
				type: "slider",
				target: "global",
				min: 0,
				max: 1000,
				step: 1,
				format: (v) => (v === 0 ? "Unlimited" : String(v))
			},
			{
				key: "bt-request-peer-speed-limit",
				label: "bt-request-peer-speed-limit",
				description: "Request more peers if speed drops below this.",
				type: "slider",
				target: "global",
				min: 0,
				max: 51200,
				step: 50,
				encode: encodeSpeed,
				decode: decodeSpeed,
				format: formatSpeed
			},
			{
				key: "bt-stop-timeout",
				label: "bt-stop-timeout",
				description: "Stop BitTorrent if speed is 0 for this many seconds.",
				type: "slider",
				target: "global",
				min: 0,
				max: 600,
				step: 1,
				unit: "sec",
				format: (v) => (v <= 1 ? "Disabled" : `${v} sec`)
			},
			{
				key: "seed-ratio",
				label: "seed-ratio",
				description: "Stop seeding after reaching this ratio.",
				type: "slider",
				target: "global",
				min: 0,
				max: 10,
				step: 0.1,
				format: (v) => (v === 0 ? "Indefinitely" : `${v}`)
			},
			{
				key: "seed-time",
				label: "seed-time",
				description: "Stop seeding after this many minutes.",
				type: "slider",
				target: "global",
				min: 0,
				max: 7200,
				step: 10,
				unit: "min",
				format: (v) => (v === 0 ? "Disabled" : `${v} sec`)
			},
			{
				key: "bt-tracker-connect-timeout",
				label: "bt-tracker-connect-timeout",
				description: "Timeout for connecting to trackers.",
				type: "slider",
				target: "global",
				min: 0,
				max: 300,
				step: 1,
				unit: "sec"
			},
			{
				key: "bt-tracker-interval",
				label: "bt-tracker-interval",
				description: "Interval between tracker requests. 0 = auto.",
				type: "slider",
				target: "global",
				min: 0,
				max: 3600,
				step: 10,
				unit: "sec",
				format: (v) => (v === 0 ? "Auto" : `${v} sec`)
			},
			{
				key: "bt-tracker-timeout",
				label: "bt-tracker-timeout",
				description: "Timeout for tracker responses.",
				type: "slider",
				target: "global",
				min: 0,
				max: 300,
				step: 1,
				unit: "sec"
			},
			{
				key: "dht-message-timeout",
				label: "dht-message-timeout",
				description: "Timeout for DHT messages.",
				type: "slider",
				target: "cmd",
				min: 0,
				max: 600,
				step: 1,
				unit: "sec"
			},
		]
	},
	// No metalink impl
	// {
	// 	id: "metalink",
	// 	title: "Metalink",
	// 	description: "Mirror selection and metalink-specific options.",
	// 	icon: "link",
	// 	fields: [
	// 		{
	// 			key: "metalink-file",
	// 			label: "metalink-file",
	// 			description: "The path to the Metalink file.",
	// 			type: "text",
	// 			target: "cmd",
	// 		},
	// 		{
	// 			key: "metalink-language",
	// 			label: "metalink-language",
	// 			description: "Preferred language in metalink metadata.",
	// 			type: "text",
	// 			target: "global",
	// 			placeholder: "en-US"
	// 		},
	// 		{
	// 			key: "metalink-location",
	// 			label: "metalink-location",
	// 			description: "Preferred location code for metalink mirrors.",
	// 			type: "text",
	// 			target: "global",
	// 			placeholder: "jp,us"
	// 		},
	// 		{
	// 			key: "metalink-os",
	// 			label: "metalink-os",
	// 			description: "Preferred OS in metalink metadata.",
	// 			type: "text",
	// 			target: "global",
	// 			placeholder: "linux"
	// 		},
	// 		{
	// 			key: "metalink-version",
	// 			label: "metalink-version",
	// 			description: "Preferred version in metalink metadata.",
	// 			type: "text",
	// 			target: "global",
	// 		},
	// 		{
	// 			key: "metalink-base-uri",
	// 			label: "metalink-base-uri",
	// 			description: "Base URI to resolve relative URIs.",
	// 			type: "text",
	// 			target: "global",
	// 		},
	// 		{
	// 			key: "follow-metalink",
	// 			label: "follow-metalink",
	// 			description: "How aria2 should handle metalink files.",
	// 			type: "select",
	// 			target: "global",
	// 			options: ["true", "false", "mem"]
	// 		},
	// 		{
	// 			key: "metalink-preferred-protocol",
	// 			label: "metalink-preferred-protocol",
	// 			description: "Preferred protocol when multiple mirrors exist.",
	// 			type: "select",
	// 			target: "global",
	// 			options: ["none", "http", "https", "ftp"]
	// 		},
	// 		{
	// 			key: "metalink-enable-unique-protocol",
	// 			label: "metalink-enable-unique-protocol",
	// 			description: "Only use unique protocols from the metalink.",
	// 			type: "boolean",
	// 			target: "global"
	// 		}
	// 	]
	// },
	{
		id: "advanced",
		title: "Advance RPC & Networking",
		description: "RPC server settings, IPv6, TLS, Hooks, Cache, and low-level tweaks.",
		icon: "advanced",
		fields: [
			// Server Hooks
			{
				key: "on-download-start",
				label: "on-download-start",
				description: "Command to execute when download starts.",
				type: "text",
				target: "cmd",
				placeholder: ""
			},
			{
				key: "on-download-pause",
				label: "on-download-pause",
				description: "Command to execute when download pauses.",
				type: "text",
				target: "cmd",
				placeholder: ""
			},
			{
				key: "on-download-stop",
				label: "on-download-stop",
				description: "Command to execute when download stops.",
				type: "text",
				target: "cmd",
				placeholder: ""
			},
			{
				key: "on-download-complete",
				label: "on-download-complete",
				description: "Command to execute when download completes.",
				type: "text",
				target: "cmd",
				placeholder: ""
			},
			{
				key: "on-download-error",
				label: "on-download-error",
				description: "Command to execute when download fails.",
				type: "text",
				target: "cmd",
				placeholder: ""
			},
			{
				key: "on-bt-download-complete",
				label: "on-bt-download-complete",
				description: "Command to execute when torrent completes.",
				type: "text",
				target: "cmd",
				placeholder: ""
			},

			// Advanced Text & Networking
			{
				key: "rpc-certificate",
				label: "rpc-certificate",
				description: "Path to the certificate for RPC SSL/TLS.",
				type: "text",
				target: "cmd"
			},
			{
				key: "rpc-private-key",
				label: "rpc-private-key",
				description: "Path to the private key for RPC SSL/TLS.",
				type: "text",
				target: "cmd"
			},
			{
				key: "ca-certificate",
				label: "ca-certificate",
				description: "Path to CA certs for SSL verification.",
				type: "text",
				target: "cmd",
			},
			{
				key: "certificate",
				label: "certificate",
				description: "Path to the client certificate for SSL/TLS.",
				type: "text",
				target: "cmd"
			},
			{
				key: "private-key",
				label: "private-key",
				description: "Path to the client private key.",
				type: "text",
				target: "cmd"
			},
			{
				key: "dscp",
				label: "dscp",
				description: "DSCP value in outgoing IP packets for QoS.",
				type: "text",
				target: "cmd",
			},
			{
				key: "interface",
				label: "interface",
				description: "Bind sockets to given interface.",
				type: "text",
				target: "cmd",
			},
			{
				key: "multiple-interface",
				label: "multiple-interface",
				description: "Comma-separated list of interfaces to bind.",
				type: "text",
				target: "cmd",
			},

			{
				key: "min-tls-version",
				label: "min-tls-version",
				description: "Minimum SSL/TLS version to enable.",
				type: "select",
				target: "cmd",
				options: ["TLSv1.0", "TLSv1.1", "TLSv1.2", "TLSv1.3"]
			},

			{
				key: "rpc-listen-all",
				label: "rpc-listen-all",
				description: "Listen on all network interfaces for aria2 RPC.",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "rpc-allow-origin-all",
				label: "rpc-allow-origin-all",
				description: "Add Access-Control-Allow-Origin: * to RPC response.",
				type: "boolean",
				target: "cmd"
			},
			// {
			// 	key: "rpc-save-upload-metadata",
			// 	label: "rpc-save-upload-metadata",
			// 	description: "Save uploaded torrent/metalink metadata via RPC.",
			// 	type: "boolean",
			// 	target: "global"
			// },
			{
				key: "rpc-secure",
				label: "rpc-secure",
				description: "RPC over SSL/TLS.",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "disable-ipv6",
				label: "disable-ipv6",
				description: "Disable IPv6 resolution and usage.",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "check-certificate",
				label: "check-certificate",
				description: "Verify SSL/TLS peers against CA certificates.",
				type: "boolean",
				target: "cmd"
			},
			{
				key: "rpc-max-request-size",
				label: "rpc-max-request-size",
				description: "Max size of RPC request. (e.g. 2M)",
				type: "slider",
				target: "cmd",
				min: 0,
				max: 64,
				step: 1,
				encode: encodeSizeMiB,
				decode: (val) => decodeSizeMiB(val, 20),
				format: formatSizeMiB
			},
			{
				key: "disk-cache",
				label: "disk-cache",
				description: "Disk cache size. 0 disables caching.",
				type: "slider",
				target: "cmd",
				min: 0,
				max: 1024,
				step: 1,
				encode: encodeSizeMiB,
				decode: (val) => decodeSizeMiB(val, 16),
				format: (v) => (v <= 1 ? "Disabled" : `${v} MiB`)
			},
			{
				key: "socket-recv-buffer-size",
				label: "socket-recv-buffer-size",
				description: "`SO_RCVBUF` limit in bytes.  Specifying 0 will disable this option. This value will be set to socket file descriptor using `SO_RCVBUF` socket option with `setsockopt()` call.",
				type: "slider",
				target: "cmd",
				min: 0,
				max: 10485760,
				step: 65536,
				unit: "bytes",
				format: (v) => (v === 0 ? "Disabled" : `${v}`)
			},
			// Readonly
			{
				key: "daemon",
				label: "daemon",
				description: "Run as background daemon process. Automatically managed by Qateel!!",
				type: "boolean",
				target: "readonly"
			},
			{
				key: "enable-rpc",
				label: "enable-rpc",
				description: "Enable JSON-RPC server for Qateel.",
				type: "boolean",
				target: "readonly"
			},
			{
				key: "rpc-listen-port",
				label: "rpc-listen-port",
				description: "RPC server listening port.",
				type: "text",
				target: "readonly",
				placeholder: "6800"
			},
			{
				key: "rpc-secret",
				label: "rpc-secret",
				description: "RPC Secret token.",
				type: "text",
				target: "readonly",
				placeholder: "••••••••"
			},
			{
				key: "rpc-user",
				label: "rpc-user",
				description: "Username for RPC basic authentication.",
				type: "text",
				target: "readonly",
				placeholder: "••••••••"
			},
			{
				key: "rpc-passwd",
				label: "rpc-passwd",
				description: "Password for RPC basic authentication.",
				type: "text",
				target: "readonly",
				placeholder: "••••••••"
			},
		]
	}
];

