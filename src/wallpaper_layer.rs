use crate::image::Artwork;
use crate::wallpaper::Wallpaper;
use smithay_client_toolkit::reexports::client::globals::GlobalList;
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::shell::wlr_layer::LayerSurface;
use smithay_client_toolkit::{output::OutputState, registry::RegistryState};

use smithay_client_toolkit::{
    compositor::CompositorState,
    shell::{
        WaylandSurface,
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
    },
};

mod handlers;

pub struct WallpaperLayer {
    pub wallpaper: Wallpaper,

    registry_state: RegistryState,
    _compositor_state: CompositorState,
    _layer_shell: LayerShell,
    output_state: OutputState,
    pub layer: LayerSurface,

    output: Option<WlOutput>,
    scale: i32,
    logical_size: Option<(u32, u32)>,

    pub exit: bool,
}

impl WallpaperLayer {
    pub fn new(
        conn: &Connection,
        globals: &GlobalList,
        qh: &QueueHandle<Self>,
    ) -> eyre::Result<Self> {
        let compositor = CompositorState::bind(globals, qh).expect("wl_compositor not available");
        let layer_shell = LayerShell::bind(globals, qh).expect("layer shell not available");

        let surface = compositor.create_surface(qh);
        let layer =
            layer_shell.create_layer_surface(qh, surface, Layer::Background, Some("bgLayer"), None);

        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.set_anchor(Anchor::all());
        layer.set_exclusive_zone(-1);
        layer.commit();

        let wallpaper = Wallpaper::new(conn, layer.wl_surface())?;
        let registry_state = RegistryState::new(globals);
        let output_state = OutputState::new(globals, qh);

        Ok(Self {
            wallpaper,
            registry_state,
            _compositor_state: compositor,
            _layer_shell: layer_shell,
            output_state,
            layer,
            output: None,
            scale: 1,
            logical_size: None,
            exit: false,
        })
    }

    pub fn draw(&mut self) {
        self.wallpaper.draw();
    }

    pub fn set_artwork(&mut self, artwork: Artwork) {
        self.wallpaper.set_artwork(artwork);
    }

    pub(crate) fn logical_size_from_output(&self) -> Option<(u32, u32)> {
        let outputs: Vec<WlOutput> = match &self.output {
            Some(output) => vec![output.clone()],
            None => self.output_state.outputs().collect(),
        };

        for output in outputs {
            if let Some(info) = self.output_state.info(&output)
                && let Some((width, height)) = info.logical_size
                && width > 0
                && height > 0
            {
                return Some((width as u32, height as u32));
            }
        }
        None
    }

    pub(crate) fn scale_for_output(&self, output: &WlOutput) -> i32 {
        self.output_state
            .info(output)
            .map(|info| info.scale_factor)
            .unwrap_or(1)
            .max(1)
    }

    pub(crate) fn apply_size(&mut self, conn: &Connection) {
        let Some((width, height)) = self.logical_size else {
            return;
        };

        let scale = self.scale.max(1);
        self.layer.wl_surface().set_buffer_scale(scale);
        if let Err(err) = conn.flush() {
            log::warn!("failed to flush the buffer-scale request: {err}");
        }

        self.wallpaper
            .resize(width * scale as u32, height * scale as u32);
    }
}
