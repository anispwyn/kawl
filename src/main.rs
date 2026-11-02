mod image;
mod renderer;
mod wallpaper;
mod wallpaper_layer;

use std::time::Duration;

use eyre::Result;
use smithay_client_toolkit::reexports::calloop::{
    EventLoop, channel,
    timer::{TimeoutAction, Timer},
};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::reexports::client::{Connection, globals::registry_queue_init};

use wallpaper_layer::WallpaperLayer;

const FRAME_INTERVAL: Duration = Duration::from_millis(33);

fn main() -> Result<()> {
    env_logger::init();

    let conn = Connection::connect_to_env()?;

    let (globals, event_queue) = registry_queue_init::<WallpaperLayer>(&conn)?;
    let qh = event_queue.handle();

    let mut state = WallpaperLayer::new(&conn, &globals, &qh)?;

    let mut event_loop: EventLoop<WallpaperLayer> = EventLoop::try_new()?;
    let handle = event_loop.handle();

    WaylandSource::new(conn.clone(), event_queue)
        .insert(handle.clone())
        .map_err(|err| eyre::eyre!("failed to insert the Wayland source: {}", err.error))?;

    let timer = Timer::from_duration(FRAME_INTERVAL);
    handle
        .insert_source(timer, |_, _, state: &mut WallpaperLayer| {
            if state.exit {
                return TimeoutAction::Drop;
            }
            state.draw();
            TimeoutAction::ToDuration(FRAME_INTERVAL)
        })
        .map_err(|err| eyre::eyre!("failed to insert the frame timer: {}", err.error))?;

    let (art_sender, art_channel) = channel::channel::<image::Artwork>();
    handle
        .insert_source(
            art_channel,
            |event, _, state: &mut WallpaperLayer| match event {
                channel::Event::Msg(art) => state.set_artwork(art),
                channel::Event::Closed => log::debug!("artwork channel closed"),
            },
        )
        .map_err(|err| eyre::eyre!("failed to insert the artwork channel: {}", err.error))?;

    image::spawn_art_watcher(art_sender)?;

    while !state.exit {
        event_loop.dispatch(None, &mut state)?;
    }

    Ok(())
}
