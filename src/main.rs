use std::{env, error::Error, fs, path::PathBuf};

use librqbit::{AddTorrent, Session};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "simple-torrent".into());

    let Some(source) = args.next() else {
        eprintln!("Використання: {program} <файл.torrent | magnet-посилання> [папка]");
        std::process::exit(2);
    };
    let output = args.next().unwrap_or_else(|| "downloads".into());
    if args.next().is_some() {
        eprintln!("Використання: {program} <файл.torrent | magnet-посилання> [папка]");
        std::process::exit(2);
    }

    let torrent = AddTorrent::from_cli_argument(&source)?;
    let output = PathBuf::from(output);
    fs::create_dir_all(&output)?;

    let session = Session::new(output.clone()).await?;
    let handle = session
        .add_torrent(torrent, None)
        .await?
        .into_handle()
        .ok_or("не вдалося почати завантаження")?;

    println!("Завантаження в {}", output.display());
    handle.wait_until_completed().await?;
    println!("Готово.");
    session.stop().await;
    Ok(())
}
