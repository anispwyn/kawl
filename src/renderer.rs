use std::time::Instant;

use wgpu::{
    AddressMode, BindGroupDescriptor, BindGroupEntry, BindingResource, BlendState, Buffer,
    BufferDescriptor, BufferUsages, Color, ColorTargetState, ColorWrites, CommandEncoder,
    CommandEncoderDescriptor, Device, Extent3d, FilterMode, FragmentState, LoadOp,
    MultisampleState, Operations, Origin3d, PipelineCompilationOptions, PrimitiveState, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    Sampler, SamplerDescriptor, ShaderModule, ShaderModuleDescriptor, ShaderSource, StoreOp,
    TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
    VertexState,
};

const BLUR_SIZE: u32 = 128;
const BLUE_NOISE_SIZE: u32 = 64;
const BLUE_NOISE: &[u8] = include_bytes!("assets/blue_noise.bin");
const CHANNEL_OFFSETS: [f32; 6] = [0.0, 0.0, 6.0, 11.0, 25.0, 51.0];
const R2_X: f32 = 0.754_877_7;
const R2_Y: f32 = 0.569_840_3;
const UNIFORM_BYTES: u64 = 64;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pass {
    Tint,
    Blur,
    Blend,
    Warp,
    Output,
}
struct Target {
    view: TextureView,
    width: u32,
    height: u32,
}
pub struct Renderer {
    device: Device,
    queue: Queue,
    sampler: Sampler,
    tint_pipeline: RenderPipeline,
    blur_pipeline: RenderPipeline,
    blend_pipeline: RenderPipeline,
    warp_pipeline: RenderPipeline,
    output_pipeline: RenderPipeline,

    image: Target,
    blue_noise: Target,
    blur_a: Target,
    blur_b: Target,
    current_album: Target,
    next_album: Target,
    warp: Target,

    uniforms: Vec<Buffer>,
    uniform_index: usize,

    warp_intensity: f32,
    blur_passes: u32,
    transition_duration: f32,
    saturation: f32,
    tint_color: [f32; 3],
    tint_intensity: f32,
    dithering: f32,
    scale: f32,

    has_image: bool,
    transitioning: bool,
    transition_start: Instant,

    start: Instant,
    frame_index: u64,
}

impl Renderer {
    pub fn new(device: Device, queue: Queue, surface_format: TextureFormat) -> Self {
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("kawarp linear clamp sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let vertex = shader(&device, "kawarp vertex", include_str!("assets/vertex.wgsl"));
        let tint = shader(&device, "kawarp tint", include_str!("assets/tint.wgsl"));
        let blur = shader(
            &device,
            "kawarp blur",
            include_str!("assets/kawaseBlur.wgsl"),
        );
        let blend = shader(&device, "kawarp blend", include_str!("assets/blend.wgsl"));
        let warp = shader(
            &device,
            "kawarp warp",
            include_str!("assets/domainWrap.wgsl"),
        );
        let output = shader(&device, "kawarp output", include_str!("assets/output.wgsl"));

        let offscreen = TextureFormat::Rgba16Float;

        let tint_pipeline = make_pipeline(&device, "kawarp tint", &vertex, &tint, "fs", offscreen);
        let blur_pipeline = make_pipeline(&device, "kawarp blur", &vertex, &blur, "fs", offscreen);
        let blend_pipeline =
            make_pipeline(&device, "kawarp blend", &vertex, &blend, "fs", offscreen);
        let warp_pipeline = make_pipeline(&device, "kawarp warp", &vertex, &warp, "fs", offscreen);
        let output_pipeline = make_pipeline(
            &device,
            "kawarp output",
            &vertex,
            &output,
            "fs_main",
            surface_format,
        );

        let image = make_target(
            &device,
            1,
            1,
            TextureFormat::Rgba8Unorm,
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        );

        let blue_noise = make_target(
            &device,
            BLUE_NOISE_SIZE,
            BLUE_NOISE_SIZE,
            TextureFormat::R8Unorm,
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        );
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: blue_noise.view.texture(),
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            BLUE_NOISE,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(BLUE_NOISE_SIZE),
                rows_per_image: Some(BLUE_NOISE_SIZE),
            },
            Extent3d {
                width: BLUE_NOISE_SIZE,
                height: BLUE_NOISE_SIZE,
                depth_or_array_layers: 1,
            },
        );

        let blur_a = make_target(
            &device,
            BLUR_SIZE,
            BLUR_SIZE,
            offscreen,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        );
        let blur_b = make_target(
            &device,
            BLUR_SIZE,
            BLUR_SIZE,
            offscreen,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        );
        let current_album = make_target(
            &device,
            BLUR_SIZE,
            BLUR_SIZE,
            offscreen,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        );
        let next_album = make_target(
            &device,
            BLUR_SIZE,
            BLUR_SIZE,
            offscreen,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        );

        let warp = make_target(
            &device,
            1,
            1,
            offscreen,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        );

        let mut renderer = Self {
            device,
            queue,
            sampler,
            tint_pipeline,
            blur_pipeline,
            blend_pipeline,
            warp_pipeline,
            output_pipeline,
            image,
            blue_noise,
            blur_a,
            blur_b,
            current_album,
            next_album,
            warp,
            uniforms: Vec::new(),
            uniform_index: 0,
            warp_intensity: 1.0,
            blur_passes: 8,
            transition_duration: 1.0,
            saturation: 1.5,
            tint_color: [0.157, 0.157, 0.235],
            tint_intensity: 0.15,
            dithering: 0.008,
            scale: 1.0,
            has_image: false,
            transitioning: false,
            transition_start: Instant::now(),
            start: Instant::now(),
            frame_index: 0,
        };

        renderer.set_gradient();
        renderer
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self.warp.width == width && self.warp.height == height {
            return;
        }
        self.warp = make_target(
            &self.device,
            width,
            height,
            TextureFormat::Rgba16Float,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        );
    }

    pub fn set_artwork(&mut self, width: u32, height: u32, rgba: &[u8]) {
        if width == 0 || height == 0 {
            return;
        }
        let expected = width as usize * height as usize * 4;
        if rgba.len() < expected {
            log::warn!(
                "artwork buffer is {} bytes, expected {} for {width}x{height}",
                rgba.len(),
                expected
            );
            return;
        }

        if self.image.width != width || self.image.height != height {
            self.image = make_target(
                &self.device,
                width,
                height,
                TextureFormat::Rgba8Unorm,
                TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            );
        }

        self.queue.write_texture(
            TexelCopyTextureInfo {
                texture: self.image.view.texture(),
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &rgba[..expected],
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.process_new_image();
    }

    pub fn set_gradient(&mut self) {
        const SIZE: u32 = 256;
        let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
        for y in 0..SIZE {
            for x in 0..SIZE {
                let t = (x as f32 * 0.7 + y as f32 * 0.3) / SIZE as f32;
                let r = 0.12 + 0.18 * t;
                let g = 0.10 + 0.12 * t;
                let b = 0.22 + 0.30 * t;
                rgba.extend_from_slice(&[
                    (r * 255.0) as u8,
                    (g * 255.0) as u8,
                    (b * 255.0) as u8,
                    255,
                ]);
            }
        }
        self.set_artwork(SIZE, SIZE, &rgba);
    }

    fn process_new_image(&mut self) {
        if !self.has_image {
            let next = self.next_album.view.clone();
            let current = self.current_album.view.clone();
            self.blur_source_into(&next);
            self.blur_source_into(&current);
            self.has_image = true;
            self.transitioning = false;
        } else {
            std::mem::swap(&mut self.current_album, &mut self.next_album);
            let next = self.next_album.view.clone();
            self.blur_source_into(&next);
            self.transitioning = true;
            self.transition_start = Instant::now();
        }
    }

    fn blur_source_into(&mut self, target: &TextureView) {
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("kawarp image blur"),
            });

        let image = self.image.view.clone();
        let mut read = self.blur_a.view.clone();
        let mut write = self.blur_b.view.clone();

        let tint_color = self.tint_color;
        let tint_intensity = self.tint_intensity;
        self.encode(
            &mut encoder,
            Pass::Tint,
            &read,
            BLUR_SIZE,
            BLUR_SIZE,
            &[tint_color[0], tint_color[1], tint_color[2], tint_intensity],
            &[&image],
        );

        for i in 0..self.blur_passes {
            let offset = i as f32 + 0.5;
            self.encode(
                &mut encoder,
                Pass::Blur,
                &write,
                BLUR_SIZE,
                BLUR_SIZE,
                &[BLUR_SIZE as f32, BLUR_SIZE as f32, offset],
                &[&read],
            );
            std::mem::swap(&mut read, &mut write);
        }

        self.encode(
            &mut encoder,
            Pass::Blur,
            target,
            BLUR_SIZE,
            BLUR_SIZE,
            &[BLUR_SIZE as f32, BLUR_SIZE as f32, 0.0],
            &[&read],
        );

        self.queue.submit(Some(encoder.finish()));
        self.uniform_index = 0;
    }

    pub fn render(&mut self, target: &TextureView, width: u32, height: u32) {
        if !self.has_image || width == 0 || height == 0 {
            return;
        }
        self.resize(width, height);

        let now = Instant::now();
        let time = now.duration_since(self.start).as_secs_f32();

        let frame = self.frame_index;
        self.frame_index = self.frame_index.wrapping_add(1);
        let (noise_x, noise_y) = noise_offset(frame);

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("kawarp frame"),
            });

        let mut source = self.next_album.view.clone();
        if self.transitioning {
            let elapsed = now.duration_since(self.transition_start).as_secs_f32();
            let progress = if self.transition_duration > 0.0 {
                (elapsed / self.transition_duration).min(1.0)
            } else {
                1.0
            };

            if progress >= 1.0 {
                self.transitioning = false;
            } else {
                let amount = 0.5 - 0.5 * (progress * std::f32::consts::PI).cos();
                let current = self.current_album.view.clone();
                let next = self.next_album.view.clone();
                let blended = self.blur_a.view.clone();
                self.encode(
                    &mut encoder,
                    Pass::Blend,
                    &blended,
                    BLUR_SIZE,
                    BLUR_SIZE,
                    &[amount],
                    &[&current, &next],
                );
                source = blended;
            }
        }

        let warp_target = self.warp.view.clone();
        self.encode(
            &mut encoder,
            Pass::Warp,
            &warp_target,
            width,
            height,
            &[time, self.warp_intensity],
            &[&source],
        );

        let mut output_uniform = [0.0f32; 16];
        output_uniform[0] = self.saturation;
        output_uniform[1] = self.dithering;
        output_uniform[2] = self.scale;
        output_uniform[4] = noise_x;
        output_uniform[5] = noise_y;
        output_uniform[8..14].copy_from_slice(&CHANNEL_OFFSETS);

        let blue_noise = self.blue_noise.view.clone();
        self.encode(
            &mut encoder,
            Pass::Output,
            target,
            width,
            height,
            &output_uniform,
            &[&warp_target, &blue_noise],
        );

        self.queue.submit(Some(encoder.finish()));
        self.uniform_index = 0;
    }

    fn pipeline(&self, pass: Pass) -> &RenderPipeline {
        match pass {
            Pass::Tint => &self.tint_pipeline,
            Pass::Blur => &self.blur_pipeline,
            Pass::Blend => &self.blend_pipeline,
            Pass::Warp => &self.warp_pipeline,
            Pass::Output => &self.output_pipeline,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn encode(
        &mut self,
        encoder: &mut CommandEncoder,
        pass: Pass,
        target: &TextureView,
        width: u32,
        height: u32,
        uniforms: &[f32],
        sources: &[&TextureView],
    ) {
        let uniform_buffer = self.next_uniform();
        self.queue
            .write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(uniforms));

        let pipeline = self.pipeline(pass);
        let layout = pipeline.get_bind_group_layout(0);

        let mut entries = Vec::with_capacity(2 + sources.len());
        entries.push(BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        });
        entries.push(BindGroupEntry {
            binding: 1,
            resource: BindingResource::Sampler(&self.sampler),
        });
        for (index, view) in sources.iter().enumerate() {
            entries.push(BindGroupEntry {
                binding: 2 + index as u32,
                resource: BindingResource::TextureView(view),
            });
        }

        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("kawarp bind group"),
            layout: &layout,
            entries: &entries,
        });

        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("kawarp pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::TRANSPARENT),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        render_pass.set_viewport(0.0, 0.0, width as f32, height as f32, 0.0, 1.0);
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    fn next_uniform(&mut self) -> Buffer {
        if self.uniform_index == self.uniforms.len() {
            let buffer = self.device.create_buffer(&BufferDescriptor {
                label: Some("kawarp uniform"),
                size: UNIFORM_BYTES,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.uniforms.push(buffer);
        }
        let buffer = self.uniforms[self.uniform_index].clone();
        self.uniform_index += 1;
        buffer
    }
}

fn shader(device: &Device, label: &str, source: &'static str) -> ShaderModule {
    device.create_shader_module(ShaderModuleDescriptor {
        label: Some(label),
        source: ShaderSource::Wgsl(source.into()),
    })
}

fn make_pipeline(
    device: &Device,
    label: &str,
    vertex: &ShaderModule,
    fragment: &ShaderModule,
    fragment_entry: &str,
    format: TextureFormat,
) -> RenderPipeline {
    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some(label),
        layout: None,
        vertex: VertexState {
            module: vertex,
            entry_point: Some("vs_target"),
            buffers: &[],
            compilation_options: PipelineCompilationOptions::default(),
        },
        fragment: Some(FragmentState {
            module: fragment,
            entry_point: Some(fragment_entry),
            targets: &[Some(ColorTargetState {
                format,
                blend: Some(BlendState::REPLACE),
                write_mask: ColorWrites::ALL,
            })],
            compilation_options: PipelineCompilationOptions::default(),
        }),
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn make_target(
    device: &Device,
    width: u32,
    height: u32,
    format: TextureFormat,
    usage: TextureUsages,
) -> Target {
    let width = width.max(1);
    let height = height.max(1);

    let texture = device.create_texture(&TextureDescriptor {
        label: Some("kawarp target"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    });

    Target {
        view: texture.create_view(&TextureViewDescriptor::default()),
        width,
        height,
    }
}

fn noise_offset(frame: u64) -> (f32, f32) {
    let frame = frame as f32;
    let x = ((frame * R2_X) % 1.0) * BLUE_NOISE_SIZE as f32;
    let y = ((frame * R2_Y) % 1.0) * BLUE_NOISE_SIZE as f32;
    (x.floor(), y.floor())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_graph_and_renders_offscreen() {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());

        let adapter = match pollster::block_on(
            instance.request_adapter(&wgpu::RequestAdapterOptions::default()),
        ) {
            Ok(adapter) => adapter,
            Err(err) => {
                eprintln!("skipping renderer test, no GPU adapter: {err}");
                return;
            }
        };

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("renderer test"),
            ..Default::default()
        }))
        .expect("failed to create a test device");

        let mut renderer = Renderer::new(
            device.clone(),
            queue.clone(),
            wgpu::TextureFormat::Rgba8Unorm,
        );

        renderer.set_artwork(2, 2, &[255, 0, 0, 255].repeat(4));
        renderer.transition_duration = 0.0;

        let output = make_target(
            &device,
            64,
            48,
            wgpu::TextureFormat::Rgba8Unorm,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        );
        renderer.render(&output.view, 64, 48);

        let row_bytes = 256u32;
        let readback = device.create_buffer(&BufferDescriptor {
            label: Some("readback"),
            size: (row_bytes * 48) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("readback encoder"),
        });
        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: output.view.texture(),
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes),
                    rows_per_image: Some(48),
                },
            },
            Extent3d {
                width: 64,
                height: 48,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(Some(encoder.finish()));

        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("failed to map the readback buffer")
        });

        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("GPU validation failed");

        let data = slice
            .get_mapped_range()
            .expect("failed to access the mapped readback buffer");
        let offset = (24 * row_bytes + 32 * 4) as usize;
        let (r, g, b) = (data[offset], data[offset + 1], data[offset + 2]);
        assert!(r > 50, "chain rendered a dark pixel: rgb({r}, {g}, {b})");
        assert!(
            r as i32 > g as i32 + 20 && r as i32 > b as i32 + 20,
            "red artwork did not stay red: rgb({r}, {g}, {b})"
        );
        drop(data);
        readback.unmap();
    }
}
