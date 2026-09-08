use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use bendy::serde::to_bytes;
use librqbit::{Session, AddTorrent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let magnet_uri = "magnet:?xt=urn:btih:cab507494d02ebb1178b38f2e9d7be299c86b862";
    let output_path = PathBuf::from("output.torrent");

    println!("Initializing BitTorrent session...");
    let session = Session::new("/tmp/torrents".into()).await?;

    println!("Parsing magnet link and discovering peers...");
    let torrent_handle = session
        .add_torrent(AddTorrent::from_url(magnet_uri), None)
        .await?
        .into_handle()
        .ok_or("Failed to get torrent handle")?;

    println!("Waiting for metadata to download from peers...");
    let shared_meta = torrent_handle.wait_for_metadata().await?;

    let torrent_meta = shared_meta.read();

    println!("Metadata successfully fetched! Encoding to .torrent format...");
    let bencoded_bytes = to_bytes(&*torrent_meta)?;

    let mut file = File::create(&output_path)?;
    file.write_all(&bencoded_bytes)?;

    println!("Success! Created your torrent file at: {:?}", output_path);
    Ok(())
}

