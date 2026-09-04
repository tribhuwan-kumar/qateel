use reqwest::Client;
use std::time::Duration;
use std::sync::OnceLock;
use reqwest::header::{
	RANGE,
	ACCEPT_RANGES,
	CONTENT_LENGTH,
	CONTENT_DISPOSITION,
};

use crate::{aria2::types::UriMeta};

pub const UNTITLED: &str = "<Untitled>";
const HTTP_CLIENT_TIMEOUT_DURATION: u64 = 15;
static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

pub fn http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(HTTP_CLIENT_TIMEOUT_DURATION))
            .build()
            .unwrap_or_default()
    })
}

pub fn extract_filename(header_val: &str) -> Option<String> {
    if let Some(idx) = header_val.find("filename=\"") {
        let start = idx + 10;
        if let Some(end) = header_val[start..].find('"') {
            return Some(header_val[start..start+end].to_string());
        }
    }

    if let Some(idx) = header_val.find("filename=") {
        let start = idx + 9;
        let end = header_val[start..].find(';').unwrap_or(header_val[start..].len());
        let clean = header_val[start..start+end].trim_matches(|c| c == '"' || c == '\'');
        return Some(clean.to_string());
    }

    None
}

/// Probes the url synchronously to extract Headers.
pub async fn uri_metadata(uri: &str) -> Result<UriMeta, (String, UriMeta)> {
    let client = http_client();

    let internet_ok = client.get("http://clients3.google.com/generate_204")
        .timeout(Duration::from_secs(5))
        .send().await
        .map(|res| res.status() == 204)
        .unwrap_or(false);

    if !internet_ok {
        let offline_meta = UriMeta {
            name: uri.to_string(),
            total_length: "0".to_string(),
            resumable: false,
        };
        return Err(("No internet connection".to_string(), offline_meta));
    }

    let mut filename = None;
    let mut total_length = "0".to_string();
    let mut resumable = false;
    let mut success = false;
    let max_retries = 3;

    // Target probe with retries
    for attempt in 1..=max_retries {
        let mut head_success = false;

        if let Ok(res) = client.head(uri).send().await {
            // Check if status is 2xx success
            if res.status().is_success() {
                head_success = true;
                if let Some(val) = res.headers().get(ACCEPT_RANGES) {
                    resumable = val.to_str().map(|v| v.eq_ignore_ascii_case("bytes")).unwrap_or(false);
                }
                if let Some(val) = res.headers().get(CONTENT_LENGTH) {
                    if let Ok(s) = val.to_str() { total_length = s.to_string(); }
                }
                if let Some(val) = res.headers().get(CONTENT_DISPOSITION) {
                    if let Ok(s) = val.to_str() { filename = extract_filename(s); }
                }
            }
        }

        if !head_success || !resumable || filename.is_none() {
            if let Ok(res_get) = client.get(uri).header(RANGE, "bytes=0-0").send().await {
                // Ensure the GET request returned a successful or partial success status
                if res_get.status().is_success() || res_get.status() == reqwest::StatusCode::PARTIAL_CONTENT {
                    if !resumable {
                        resumable = res_get.status() == reqwest::StatusCode::PARTIAL_CONTENT;
                        if !resumable {
                            if let Some(val) = res_get.headers().get(ACCEPT_RANGES) {
                                resumable = val.to_str().map(|v| v.eq_ignore_ascii_case("bytes")).unwrap_or(false);
                            }
                        }
                    }
                    if total_length == "0" {
                        if let Some(val) = res_get.headers().get(CONTENT_LENGTH) {
                            if let Ok(s) = val.to_str() { total_length = s.to_string(); }
                        }
                    }
                    if filename.is_none() {
                        if let Some(val) = res_get.headers().get(CONTENT_DISPOSITION) {
                            if let Ok(s) = val.to_str() { filename = extract_filename(s); }
                        }
                    }
                    success = true;
                }
            }
        } else {
            success = true;
        }

        if success { break; }
        if attempt < max_retries {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    // Determine the name based on the server reachability status
    let mut final_name = if !success {
        // Server completely unreachable or returned HTTP errors, Use full uri
        uri.to_string()
    } else {
        // Server is reachable, prioritize header, then fall back to last URL segment
        filename.unwrap_or_else(|| {
            url::Url::parse(uri)
                .ok()
                .and_then(|u| u.path_segments().and_then(|seg| seg.last().map(|s| s.to_string())))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| uri.to_string())
        })
    };

    final_name = urlencoding::decode(&final_name).map(|d| d.into_owned()).unwrap_or(final_name);

    let meta = UriMeta {
        name: final_name,
        total_length,
        resumable
    };

    if !success {
        return Err(("Server is unreachable!!".to_string(), meta));
    }

    Ok(meta)
}


