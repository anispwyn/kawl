use smithay_client_toolkit::{
    compositor::CompositorHandler,
    output::{OutputHandler, OutputState},
    reexports::client::{
        Connection, QueueHandle, protocol::wl_output, protocol::wl_surface::WlSurface,
    },
    registry::{ProvidesRegistryState, RegistryState},
    shell::{
        WaylandSurface,
        wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    },
};

use crate::wallpaper_layer::WallpaperLayer;

impl LayerShellHandler for WallpaperLayer {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let (mut width, mut height) = configure.new_size;

        if (width == 0 || height == 0)
            && let Some((output_width, output_height)) = self.logical_size_from_output()
        {
            width = output_width;
            height = output_height;
        }

        if width == 0 || height == 0 {
            log::warn!("layer configure did not provide a usable size");
            return;
        }

        self.logical_size = Some((width, height));
        log::info!(
            "layer configured: {width}x{height} logical at scale {}",
            self.scale.max(1)
        );
        self.apply_size(conn);

        self.draw();
    }
}

impl CompositorHandler for WallpaperLayer {
    fn scale_factor_changed(
        &mut self,
        conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &WlSurface,
        new_factor: i32,
    ) {
        if surface != self.layer.wl_surface() {
            return;
        }

        let new_scale = new_factor.max(1);
        if new_scale != self.scale {
            self.scale = new_scale;
            self.apply_size(conn);
        }
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &WlSurface,
        output: &wl_output::WlOutput,
    ) {
        if surface != self.layer.wl_surface() {
            return;
        }

        self.output = Some(output.clone());
        let new_scale = self.scale_for_output(output);
        if new_scale != self.scale {
            self.scale = new_scale;
            self.apply_size(conn);
        }
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &WlSurface,
        output: &wl_output::WlOutput,
    ) {
        if surface != self.layer.wl_surface() {
            return;
        }
        if self.output.as_ref() == Some(output) {
            self.output = None;
        }
    }
}

impl OutputHandler for WallpaperLayer {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let is_ours = self.output.as_ref() == Some(&output) || self.output.is_none();
        if !is_ours {
            return;
        }

        let new_scale = self.scale_for_output(&output).max(1);
        let scale_changed = new_scale != self.scale;
        if scale_changed {
            self.scale = new_scale;
        }

        if self.logical_size.is_some() && (scale_changed || self.output.is_none()) {
            self.output = Some(output.clone());
            self.apply_size(conn);
        }
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if self.output.as_ref() == Some(&output) {
            self.output = None;
        }
    }
}

impl ProvidesRegistryState for WallpaperLayer {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    smithay_client_toolkit::registry_handlers![OutputState];
}

smithay_client_toolkit::delegate_registry!(WallpaperLayer);
smithay_client_toolkit::delegate_dispatch2!(WallpaperLayer);
