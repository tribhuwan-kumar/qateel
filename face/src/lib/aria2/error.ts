export const ARIA2_ERROR_MAP: Record<number, string> = {
  0: "Success",
  1: "Error occurred.",
  2: "Connection timed out.",
  3: "Resource not found.",
  4: "Max 'file not found' limit reached.",
  5: "Download too slow, aborted.",
  6: "Network connectivity issue.",
  7: "Download unfinished, interrupted.",
  8: "Server does not support resume.",
  9: "Not enough disk space.",
  10: "Piece length mismatch (.aria2 file conflict).",
  11: "This file is already downloading.",
  12: "This torrent is already active.",
  13: "File already exists on disk.",
  14: "Failed to rename file.",
  15: "Could not open existing file.",
  16: "Failed to create or modify file.",
  17: "File I/O error (disk/permission).",
  18: "Failed to create directory.",
  19: "DNS resolution failed.",
  20: "Invalid Metalink file.",
  21: "FTP command failed.",
  22: "Bad HTTP response header.",
  23: "Too many redirects.",
  24: "Authorization failed (401).",
  25: "Invalid torrent file (bencoding error).",
  26: "Corrupted torrent file.",
  27: "Invalid Magnet URI.",
  28: "Invalid options provided.",
  29: "Server overloaded/maintenance (503).",
  30: "Invalid JSON-RPC request.",
  31: "Unknown error (Reserved).",
  32: "Checksum validation failed.",
  33: "Session lost.",
};

/**
 * Returns a user-friendly error message.
 * @param code The Aria2 error code.
 * @param backendMessage Optional specific message from the server/Aria2.
 */
export function getFriendlyError(code: number | null | undefined, backendMessage?: string | null): string {
  if (code === null || code === undefined || code === 0) return "";
  const friendly = ARIA2_ERROR_MAP[code];
  if (backendMessage && backendMessage.trim().length > 0 && !backendMessage.includes("Exit status")) {
      return `${friendly} (${backendMessage})`;
  }

  return friendly || `Unknown Error (Code: ${code})`;
}

/**
 * Helper to determine if an error is recoverable (e.g. network issues can be retried)
 */
export function isRetriable(code: number | null): boolean {
    if (code === null) return false;
    // Timeouts, Network, Server Overload, 404s (sometimes temporary)
    return [2, 6, 21, 29].includes(code);
}

