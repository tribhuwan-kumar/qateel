// use std::sync::Arc;
// use librqbit::Magnet;
// use librqbit::Session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let magnet_url = "magnet:?xt=urn:btih:566a77c6a193406e8655733618e431e6b515001c&dn=Heat.1995.BluRay.x264-MuraTurk&tr=udp%3A//tracker.publictracker.xyz%3A6969/announce&tr=udp%3A//tracker.opentrackr.org%3A1337/announce&tr=udp%3A//open.demonii.com%3A1337/announce&tr=udp%3A//open.stealth.si%3A80/announce&tr=udp%3A//tracker2.dler.org%3A80/announce&tr=udp%3A//tracker.torrent.eu.org%3A451/announce&tr=udp%3A//tracker.t-1.org%3A6969/announce&tr=udp%3A//tracker.qu.ax%3A6969/announce&tr=udp%3A//tracker.opentorrent.top%3A6969/announce&tr=udp%3A//tracker.gmi.gd%3A6969/announce&tr=udp%3A//tracker.flatuslifir.is%3A6969/announce&tr=udp%3A//tracker.ducks.party%3A1984/announce&tr=udp%3A//tracker.corpscorp.online%3A80/announce&tr=udp%3A//tracker.auctor.tv%3A6969/announce&tr=udp%3A//tracker.004430.xyz%3A1337/announce&tr=udp%3A//tracker-udp.gbitt.info%3A80/announce&tr=udp%3A//tr4ck3r.duckdns.org%3A6969/announce&tr=udp%3A//torrents.tmtime.dev%3A6969/announce&tr=udp%3A//torrentclub.online%3A54123/announce&tr=udp%3A//t.overflow.biz%3A6969/announce";
//     let magnet = Magnet::parse(magnet_url)?;
//     println!("File Display Name from String: {:?}", magnet.dn);
//
//     let session = Arc::new(Session::new(std::Path::new("./downloads")).await?);
//
//     let handle = session.with_torrents(magnet, None).await?;
//
//     println!("Connecting to peers to fetch metadata sizes...");
//     let torrent_info = handle.wait_for_metadata().await?;
//     println!("Total Torrent Size: {} bytes", torrent_info.total_length());
//
//     for file in torrent_info.files() {
//         println!("File Path: {:?} | Size: {} bytes", file.path, file.length);
//     }
//
    Ok(())
}

