use std::ptr::NonNull;

use smithay_client_toolkit::reexports::client::{Connection, Proxy, protocol::wl_surface};
use wgpu::{
    CompositeAlphaMode, Device, DeviceDescriptor, Instance, InstanceDescriptor, PresentMode, Queue,
    Surface, SurfaceConfiguration, SurfaceTargetUnsafe, TextureFormat, TextureUsages,
    TextureViewDescriptor,
    rwh::{RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle},
};

use crate::renderer::Renderer;

pub struct Wallpaper {
    surface: Surface<'static>,
    device: Device,
    queue: Queue,
    config: Option<SurfaceConfiguration>,
    format: TextureFormat,
    alpha_mode: CompositeAlphaMode,
    renderer: Renderer,
}

impl Wallpaper {
    pub fn new(conn: &Connection, wl_surface: &wl_surface::WlSurface) -> eyre::Result<Self> {
        let instance = Instance::new(InstanceDescriptor::new_without_display_handle_from_env());
        let raw_display_handle = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
            NonNull::new(conn.backend().display_ptr() as *mut _).unwrap(),
        ));
        let raw_window_handle = RawWindowHandle::Wayland(WaylandWindowHandle::new(
            NonNull::new(wl_surface.id().as_ptr() as *mut _).unwrap(),
        ));
        let surface = unsafe {
            instance.create_surface_unsafe(SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_display_handle),
                raw_window_handle,
            })?
        };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
            label: Some("bgstuff device"),
            ..Default::default()
        }))?;

        let caps = surface.get_capabilities(&adapter);
        if caps.formats.is_empty() {
            eyre::bail!("the adapter cannot present to this Wayland surface");
        }
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let alpha_mode = if caps.alpha_modes.contains(&CompositeAlphaMode::Opaque) {
            CompositeAlphaMode::Opaque
        } else {
            CompositeAlphaMode::Auto
        };

        let renderer = Renderer::new(device.clone(), queue.clone(), format);

        Ok(Self {
            surface,
            device,
            queue,
            config: None,
            format,
            alpha_mode,
            renderer,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        let mut config = self.config.take().unwrap_or_else(|| SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: self.format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width,
            height,
            present_mode: PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: self.alpha_mode,
            view_formats: vec![],
        });

        config.width = width;
        config.height = height;
        self.surface.configure(&self.device, &config);
        self.config = Some(config);

        self.renderer.resize(width, height);
    }

    fn reconfigure(&mut self) {
        let Some(config) = self.config.clone() else {
            return;
        };
        self.surface.configure(&self.device, &config);
    }

    pub fn draw(&mut self) {
        let Some((width, height)) = self.config.as_ref().map(|c| (c.width, c.height)) else {
            return;
        };

        let (frame, suboptimal) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                log::debug!("suboptimal swapchain image, reconfiguring after presenting");
                (frame, true)
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.reconfigure();
                return;
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                log::warn!("surface lost, reconfiguring");
                self.reconfigure();
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
            wgpu::CurrentSurfaceTexture::Validation => {
                log::error!("surface validation error while acquiring the next image");
                return;
            }
        };

        let view = frame.texture.create_view(&TextureViewDescriptor::default());
        self.renderer.render(&view, width, height);
        self.queue.present(frame);

        // `configure` must not run while a `SurfaceTexture` is alive.
        if suboptimal {
            self.reconfigure();
        }
    }

    pub fn set_artwork(&mut self, artwork: crate::image::Artwork) {
        self.renderer
            .set_artwork(artwork.width, artwork.height, &artwork.rgba);
    }
}
