use std::fs::File;
use std::io::Write;
use std::path::Path;
use fx_torrent::{FxTorrentSession, SessionConfig, TorrentFlags};
use lava_torrent::torrent::v1::TorrentBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = SessionConfig::builder().
        // .base_path("./tmp_downloads") 
        // .client_name("MetadataFetcher")
        // .build();

    let session = FxTorrentSession::builder()
        .config(config)
        .default_extensions()
        .build()?;

    let magnet_uri = "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=Sintel";

    println!("Connecting to the swarm to fetch metadata...");

    let torrent_handle = session
        .add_torrent_from_uri(magnet_uri, TorrentFlags::default())
        .await?;

    let metadata = loop {
        if let Some(meta) = torrent_handle.metadata().await {
            break meta;
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    };

    println!("Metadata successfully fetched!");

    let raw_metadata_bytes = metadata.to_bytes()?;

    let mut file = File::create("output.torrent")?;
    file.write_all(&raw_metadata_bytes)?;

    println!("Successfully saved to output.torrent");
    Ok(())
}
