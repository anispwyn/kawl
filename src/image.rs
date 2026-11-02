use std::thread;
use std::time::Duration;

use base64::Engine as _;
use eyre::{Context, Result};
use mpris::PlayerFinder;
use percent_encoding::percent_decode_str;
use smithay_client_toolkit::reexports::calloop;
use ureq::Agent;

const POLL_INTERVAL: Duration = Duration::from_millis(750);
pub struct Artwork {
    pub key: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
pub fn spawn_art_watcher(sender: calloop::channel::Sender<Artwork>) -> Result<()> {
    thread::Builder::new()
        .name("mpris-art-watcher".into())
        .spawn(move || run(sender))
        .context("failed to spawn the MPRIS artwork thread")?;
    Ok(())
}
fn run(sender: calloop::channel::Sender<Artwork>) {
    let agent: Agent = Agent::config_builder().build().into();
    let mut finder: Option<PlayerFinder> = None;
    let mut last_key: Option<String> = None;

    loop {
        if finder.is_none() {
            match PlayerFinder::new() {
                Ok(new_finder) => finder = Some(new_finder),
                Err(err) => {
                    log::warn!("cannot connect to D-Bus yet: {err}");
                    thread::sleep(Duration::from_secs(2));
                    continue;
                }
            }
        }

        let finder_ref = finder.as_ref().expect("finder was just initialised");
        match current_artwork(finder_ref, &agent, &mut last_key) {
            Ok(Some(artwork)) => {
                let key = artwork.key.clone();
                if sender.send(artwork).is_err() {
                    break;
                }
                log::info!("loaded artwork for {key}");
            }
            Ok(None) => {}
            Err(err) => log::debug!("artwork lookup failed: {err:#}"),
        }

        thread::sleep(POLL_INTERVAL);
    }
}
fn current_artwork(
    finder: &PlayerFinder,
    agent: &Agent,
    last_key: &mut Option<String>,
) -> Result<Option<Artwork>> {
    let Some(player) = finder.find_active().ok() else {
        return Ok(None);
    };

    let metadata = player
        .get_metadata()
        .context("failed to read MPRIS metadata")?;
    let Some(url) = metadata.art_url() else {
        return Ok(None);
    };
    let track = metadata
        .track_id()
        .map(|id| id.to_string())
        .unwrap_or_default();
    let title = metadata.title().unwrap_or_default().to_owned();
    let key = format!("{track}|{title}|{url}");

    if last_key.as_deref() == Some(key.as_str()) {
        return Ok(None);
    }

    let bytes = read_art_bytes(url, agent)
        .with_context(|| format!("failed to fetch album art from {url}"))?;
    let image = image::load_from_memory(&bytes).context("failed to decode album art")?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    *last_key = Some(key.clone());

    Ok(Some(Artwork {
        key,
        width,
        height,
        rgba: rgba.into_raw(),
    }))
}
fn read_art_bytes(url: &str, agent: &Agent) -> Result<Vec<u8>> {
    if let Some(rest) = url.strip_prefix("data:") {
        let (meta, data) = rest
            .split_once(',')
            .ok_or_else(|| eyre::eyre!("malformed data: URL (no comma)"))?;

        if meta.ends_with(";base64") {
            base64::engine::general_purpose::STANDARD
                .decode(data)
                .context("invalid base64 in data: URL")
        } else {
            Ok(percent_decode_str(data).collect())
        }
    } else if let Some(path) = url.strip_prefix("file://") {
        let decoded = percent_decode_str(path).decode_utf8_lossy();
        let path = if decoded.starts_with('/') {
            decoded.into_owned()
        } else if let Some(slash) = decoded.find('/') {
            decoded[slash..].to_owned()
        } else {
            decoded.into_owned()
        };
        std::fs::read(&path).with_context(|| format!("failed to read {path}"))
    } else if url.starts_with("http://") || url.starts_with("https://") {
        let mut response = agent
            .get(url)
            .call()
            .with_context(|| format!("HTTP request for {url} failed"))?;
        response
            .body_mut()
            .read_to_vec()
            .context("failed to read the HTTP response body")
    } else if url.starts_with('/') {
        // A few players (notably some terminals) put a bare path in artUrl.
        std::fs::read(url).with_context(|| format!("failed to read {url}"))
    } else {
        eyre::bail!("unsupported artwork URL scheme: {url}");
    }
}
