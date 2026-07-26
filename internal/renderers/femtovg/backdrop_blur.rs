// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use bytemuck::{Pod, Zeroable};
use femtovg::{ImageFlags, ImageId, ImageInfo, Paint, PixelFormat};
use wgpu_30 as wgpu;

use crate::itemrenderer::{BackdropBlurCallback, CanvasRc};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

const SHADER: &str = r#"
struct CaptureParams { uv_origin: vec2<f32>, uv_size: vec2<f32> }
struct BlurParams {
    direction: vec2<f32>, texel_size: vec2<f32>, sigma: f32,
    _padding_a: f32, _padding_b: vec2<f32>,
}
@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> capture_params: CaptureParams;
@group(0) @binding(3) var<uniform> blur_params: BlurParams;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0),
    );
    let position = positions[vertex_index];
    var output: VertexOutput;
    output.position = vec4<f32>(position, 0.0, 1.0);
    output.uv = vec2<f32>((position.x + 1.0) * 0.5, (1.0 - position.y) * 0.5);
    return output;
}

@fragment
fn capture_fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(
        source_texture, source_sampler,
        capture_params.uv_origin + input.uv * capture_params.uv_size,
    );
}

@fragment
fn blur_fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let sigma = max(blur_params.sigma, 0.001);
    let coefficient_step = exp(-0.5 / (sigma * sigma));
    let coefficient_step_squared = coefficient_step * coefficient_step;
    var coefficient = 1.0 / (sqrt(2.0 * 3.141592653589793) * sigma);
    var coefficient_sum = coefficient;
    var color_sum = textureSample(source_texture, source_sampler, input.uv) * coefficient;
    var next_step = coefficient_step;
    coefficient *= next_step;
    next_step *= coefficient_step_squared;
    let sample_count = min(12, i32(ceil(1.5 * sigma)));
    for (var sample_index = 1; sample_index < 12; sample_index += 1) {
        if (sample_index >= sample_count) { break; }
        let offset = f32(sample_index) * blur_params.direction * blur_params.texel_size;
        color_sum += textureSample(source_texture, source_sampler, input.uv - offset) * coefficient;
        color_sum += textureSample(source_texture, source_sampler, input.uv + offset) * coefficient;
        coefficient_sum += 2.0 * coefficient;
        coefficient *= next_step;
        next_step *= coefficient_step_squared;
    }
    return color_sum / coefficient_sum;
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CaptureParams {
    uv_origin: [f32; 2],
    uv_size: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlurParams {
    direction: [f32; 2],
    texel_size: [f32; 2],
    sigma: f32,
    _padding: [f32; 3],
}

struct Pipeline {
    capture: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Pipeline {
    fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Slint backdrop blur bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Slint backdrop blur pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Slint backdrop blur shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let create = |label, entry| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vertex_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            capture: create("Slint backdrop capture pipeline", "capture_fragment"),
            blur: create("Slint backdrop Gaussian pipeline", "blur_fragment"),
            layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Slint backdrop clamp sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
        }
    }
}

struct Targets {
    capture: wgpu::Texture,
    horizontal: wgpu::Texture,
    blurred: wgpu::Texture,
    image: ImageId,
    capture_params: wgpu::Buffer,
    horizontal_params: wgpu::Buffer,
    vertical_params: wgpu::Buffer,
    capture_group: wgpu::BindGroup,
    horizontal_group: wgpu::BindGroup,
    vertical_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

impl Targets {
    fn new(
        device: &wgpu::Device,
        canvas: &mut femtovg::Canvas<femtovg::renderer::WGPURenderer>,
        pipeline: &Pipeline,
        scene_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) -> Result<Self, femtovg::ErrorKind> {
        let texture = |label| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };
        let capture = texture("Slint backdrop capture texture");
        let horizontal = texture("Slint backdrop horizontal texture");
        let blurred = texture("Slint backdrop result texture");
        let capture_params = uniform_buffer::<CaptureParams>(device, "Slint backdrop capture uniforms");
        let horizontal_params = uniform_buffer::<BlurParams>(device, "Slint backdrop horizontal uniforms");
        let vertical_params = uniform_buffer::<BlurParams>(device, "Slint backdrop vertical uniforms");
        let unused_capture = uniform_buffer::<CaptureParams>(device, "Slint backdrop unused capture uniforms");
        let unused_blur = uniform_buffer::<BlurParams>(device, "Slint backdrop unused blur uniforms");
        let capture_group = bind_group(device, pipeline, scene_view, &capture_params, &unused_blur);
        let horizontal_group = bind_group(
            device, pipeline, &capture.create_view(&Default::default()), &unused_capture, &horizontal_params,
        );
        let vertical_group = bind_group(
            device, pipeline, &horizontal.create_view(&Default::default()), &unused_capture, &vertical_params,
        );
        let image = canvas.create_image_from_native_texture(
            blurred.clone(),
            ImageInfo::new(ImageFlags::PREMULTIPLIED, width as usize, height as usize, PixelFormat::Rgba8),
        )?;
        Ok(Self {
            capture, horizontal, blurred, image, capture_params, horizontal_params,
            vertical_params, capture_group, horizontal_group, vertical_group, width, height,
        })
    }
}

pub(crate) struct BackdropBlur {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: Pipeline,
    canvas: CanvasRc<femtovg::renderer::WGPURenderer>,
    scene: RefCell<wgpu::Texture>,
    width: Cell<u32>,
    height: Cell<u32>,
    targets: RefCell<Vec<Targets>>,
    next_target: Cell<usize>,
    enabled: Cell<bool>,
    transform_diagnostic_emitted: Cell<bool>,
    allocation_diagnostic_emitted: Cell<bool>,
}

impl BackdropBlur {
    pub(crate) fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        canvas: CanvasRc<femtovg::renderer::WGPURenderer>,
        width: u32,
        height: u32,
    ) -> Rc<Self> {
        Rc::new(Self {
            pipeline: Pipeline::new(&device),
            scene: RefCell::new(scene_texture(&device, width, height)),
            device,
            queue,
            canvas,
            width: Cell::new(width),
            height: Cell::new(height),
            targets: Default::default(),
            next_target: Cell::new(0),
            enabled: Cell::new(true),
            transform_diagnostic_emitted: Cell::new(false),
            allocation_diagnostic_emitted: Cell::new(false),
        })
    }

    pub(crate) fn begin_frame(&self, enabled: bool) {
        self.enabled.set(enabled);
        self.next_target.set(0);
    }

    pub(crate) fn render_output(&self) -> femtovg::renderer::WGPURenderOutput {
        femtovg::renderer::WGPURenderOutput {
            view: self.scene.borrow().create_view(&Default::default()),
            width: self.width.get(),
            height: self.height.get(),
            format: FORMAT,
        }
    }

    pub(crate) fn resize(&self, width: u32, height: u32) {
        if self.width.get() == width && self.height.get() == height { return; }
        self.width.set(width);
        self.height.set(height);
        *self.scene.borrow_mut() = scene_texture(&self.device, width, height);
        let scene_view = self.scene.borrow().create_view(&Default::default());
        for target in self.targets.borrow_mut().iter_mut() {
            target.capture_group = bind_group(
                &self.device,
                &self.pipeline,
                &scene_view,
                &target.capture_params,
                &target.horizontal_params,
            );
        }
    }

    pub(crate) fn copy_to_surface(&self, surface: &wgpu::Texture) {
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Slint backdrop presentation encoder"),
        });
        encoder.copy_texture_to_texture(
            self.scene.borrow().as_image_copy(),
            surface.as_image_copy(),
            wgpu::Extent3d {
                width: self.width.get(), height: self.height.get(), depth_or_array_layers: 1,
            },
        );
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    pub(crate) fn callback(self: &Rc<Self>) -> BackdropBlurCallback<femtovg::renderer::WGPURenderer> {
        let state = self.clone();
        Rc::new(move |canvas, path, radius| state.draw(canvas, path, radius.get()))
    }

    fn draw(
        &self,
        canvas: &CanvasRc<femtovg::renderer::WGPURenderer>,
        path: &femtovg::Path,
        radius: f32,
    ) {
        if !self.enabled.get() || radius <= 0. { return; }
        let mut canvas_ref = canvas.borrow_mut();
        let transform = canvas_ref.transform();
        let [a, b, c, d, tx, ty] = transform.0;
        if b.abs() > f32::EPSILON || c.abs() > f32::EPSILON || a <= 0. || d <= 0. {
            if !self.transform_diagnostic_emitted.replace(true) {
                eprintln!("Slint FemtoVG-WGPU backdrop blur skipped for a rotated or reflected item");
            }
            return;
        }
        let bbox = canvas_ref.path_bbox(path);
        let padding = radius.ceil() * 2.;
        let x = (bbox.minx - padding).floor().max(0.);
        let y = (bbox.miny - padding).floor().max(0.);
        let right = (bbox.maxx + padding).ceil().min(self.width.get() as f32);
        let bottom = (bbox.maxy + padding).ceil().min(self.height.get() as f32);
        if right <= x || bottom <= y { return; }
        let capture_width = right - x;
        let capture_height = bottom - y;
        let downsample = if radius > 12. { 2 } else { 1 };
        let target_width = ((capture_width / downsample as f32).ceil() as u32).max(1);
        let target_height = ((capture_height / downsample as f32).ceil() as u32).max(1);

        if let Some(commands) = canvas_ref.flush_to_output(self.render_output()) {
            self.queue.submit(std::iter::once(commands));
        }
        drop(canvas_ref);

        let target_index = self.next_target.get();
        self.next_target.set(target_index + 1);
        if !self.ensure_target(target_index, target_width, target_height) { return; }
        let targets = self.targets.borrow();
        let targets = &targets[target_index];
        let capture = CaptureParams {
            uv_origin: [x / self.width.get() as f32, y / self.height.get() as f32],
            uv_size: [capture_width / self.width.get() as f32, capture_height / self.height.get() as f32],
        };
        let sigma = (radius / 2. / downsample as f32).min(8.);
        let texel_size = [1. / targets.width as f32, 1. / targets.height as f32];
        let horizontal = BlurParams { direction: [1., 0.], texel_size, sigma, _padding: [0.; 3] };
        let vertical = BlurParams { direction: [0., 1.], ..horizontal };
        self.queue.write_buffer(&targets.capture_params, 0, bytemuck::bytes_of(&capture));
        self.queue.write_buffer(&targets.horizontal_params, 0, bytemuck::bytes_of(&horizontal));
        self.queue.write_buffer(&targets.vertical_params, 0, bytemuck::bytes_of(&vertical));
        self.queue.submit(std::iter::once(encode_blur(&self.device, &self.pipeline, targets)));

        let local_x = (x - tx) / a;
        let local_y = (y - ty) / d;
        let local_width = capture_width / a;
        let local_height = capture_height / d;
        let paint = Paint::image(
            targets.image, local_x, local_y, local_width, local_height, 0., 1.,
        ).with_anti_alias(false);
        canvas.borrow_mut().fill_path(path, &paint);
    }

    fn ensure_target(&self, index: usize, width: u32, height: u32) -> bool {
        let mut targets = self.targets.borrow_mut();
        if let Some(existing) = targets.get(index) {
            if existing.width == width && existing.height == height { return true; }
            self.canvas.borrow_mut().delete_image(existing.image);
            targets.truncate(index);
        }
        let scene_view = self.scene.borrow().create_view(&Default::default());
        match Targets::new(
            &self.device, &mut self.canvas.borrow_mut(), &self.pipeline, &scene_view, width, height,
        ) {
            Ok(target) => {
                targets.push(target);
                true
            }
            Err(error) => {
                if !self.allocation_diagnostic_emitted.replace(true) {
                    eprintln!("Slint FemtoVG-WGPU backdrop blur allocation failed: {error}");
                }
                false
            }
        }
    }

    fn delete_target_images(&self) {
        let mut canvas = self.canvas.borrow_mut();
        for target in self.targets.borrow().iter() { canvas.delete_image(target.image); }
    }
}

impl Drop for BackdropBlur {
    fn drop(&mut self) { self.delete_target_images(); }
}

fn scene_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Slint sampleable scene texture"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}


fn uniform_buffer<T>(device: &wgpu::Device, label: &'static str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: std::mem::size_of::<T>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn bind_group(
    device: &wgpu::Device,
    pipeline: &Pipeline,
    source: &wgpu::TextureView,
    capture: &wgpu::Buffer,
    blur: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Slint backdrop blur bind group"),
        layout: &pipeline.layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(source) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&pipeline.sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: capture.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: blur.as_entire_binding() },
        ],
    })
}

fn encode_blur(
    device: &wgpu::Device,
    pipeline: &Pipeline,
    target: &Targets,
) -> wgpu::CommandBuffer {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Slint backdrop blur encoder"),
    });
    fullscreen_pass(&mut encoder, &target.capture, &pipeline.capture, &target.capture_group);
    fullscreen_pass(&mut encoder, &target.horizontal, &pipeline.blur, &target.horizontal_group);
    fullscreen_pass(&mut encoder, &target.blurred, &pipeline.blur, &target.vertical_group);
    encoder.finish()
}

fn fullscreen_pass(
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    pipeline: &wgpu::RenderPipeline,
    group: &wgpu::BindGroup,
) {
    let view = texture.create_view(&Default::default());
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Slint backdrop blur pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.draw(0..3, 0..1);
}
