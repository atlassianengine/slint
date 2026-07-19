// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Exercises live backdrop blur without adding a Slint item property.
//!
//! The harness renders the accumulated scene into a sampleable texture.
//! Each glass panel captures that texture into a distinct pooled target, applies a full-RGBA
//! separable Gaussian blur, and composites the result before the next panel captures the scene.

use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytemuck::{Pod, Zeroable};
use femtovg::{Canvas, Color, ImageFlags, ImageId, ImageInfo, Paint, Path, PixelFormat};
use wgpu_29 as wgpu;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const NO_PANEL_INDICES: [usize; 0] = [];
const ONE_PANEL_INDICES: [usize; 1] = [2];
const FOUR_PANEL_INDICES: [usize; 4] = [0, 1, 2, 5];
const SIX_PANEL_INDICES: [usize; 6] = [0, 1, 2, 3, 4, 5];
const SIX_PANEL_BLUR_STEPS: [f32; 6] = [2.0, 6.0, 12.0, 18.0, 24.0, 32.0];

const BACKDROP_SHADER: &str = r#"
struct CaptureParams {
    uv_origin: vec2<f32>,
    uv_size: vec2<f32>,
}

struct BlurParams {
    direction: vec2<f32>,
    texel_size: vec2<f32>,
    sigma: f32,
    _padding_a: f32,
    _padding_b: vec2<f32>,
}

struct ColorParams {
    row_0: vec4<f32>,
    row_1: vec4<f32>,
    row_2: vec4<f32>,
    row_3: vec4<f32>,
    offset: vec4<f32>,
}

@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> capture_params: CaptureParams;
@group(0) @binding(3) var<uniform> blur_params: BlurParams;
@group(0) @binding(4) var<uniform> color_params: ColorParams;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let position = positions[vertex_index];
    var output: VertexOutput;
    output.position = vec4<f32>(position, 0.0, 1.0);
    output.uv = vec2<f32>((position.x + 1.0) * 0.5, (1.0 - position.y) * 0.5);
    return output;
}

@fragment
fn capture_fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = capture_params.uv_origin + input.uv * capture_params.uv_size;
    return textureSample(source_texture, source_sampler, uv);
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
        if (sample_index >= sample_count) {
            break;
        }
        let offset = f32(sample_index) * blur_params.direction * blur_params.texel_size;
        color_sum += textureSample(source_texture, source_sampler, input.uv - offset) * coefficient;
        color_sum += textureSample(source_texture, source_sampler, input.uv + offset) * coefficient;
        coefficient_sum += 2.0 * coefficient;
        coefficient *= next_step;
        next_step *= coefficient_step_squared;
    }
    return color_sum / coefficient_sum;
}

@fragment
fn color_fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let premultiplied = textureSample(source_texture, source_sampler, input.uv);
    let straight_rgb = select(vec3<f32>(0.0), premultiplied.rgb / premultiplied.a, premultiplied.a > 0.00001);
    let straight = vec4<f32>(straight_rgb, premultiplied.a);
    let transformed = vec4<f32>(
        dot(color_params.row_0, straight),
        dot(color_params.row_1, straight),
        dot(color_params.row_2, straight),
        dot(color_params.row_3, straight),
    ) + color_params.offset;
    let alpha = clamp(transformed.a, 0.0, 1.0);
    return vec4<f32>(clamp(transformed.rgb, vec3<f32>(0.0), vec3<f32>(1.0)) * alpha, alpha);
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

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq)]
struct ColorParams {
    rows: [[f32; 4]; 4],
    offset: [f32; 4],
}

impl ColorParams {
    fn identity() -> Self {
        Self {
            rows: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            offset: [0.0; 4],
        }
    }

    fn then(self, next: Self) -> Self {
        let mut rows = [[0.0; 4]; 4];
        let mut offset = [0.0; 4];
        for row in 0..4 {
            for column in 0..4 {
                rows[row][column] =
                    (0..4).map(|index| next.rows[row][index] * self.rows[index][column]).sum();
            }
            offset[row] = next.offset[row]
                + (0..4).map(|index| next.rows[row][index] * self.offset[index]).sum::<f32>();
        }
        Self { rows, offset }
    }

    fn brightness(amount: f32) -> Self {
        let mut result = Self::identity();
        for index in 0..3 {
            result.rows[index][index] = amount.max(0.0);
        }
        result
    }

    fn contrast(amount: f32) -> Self {
        let amount = amount.max(0.0);
        let mut result = Self::brightness(amount);
        result.offset[..3].fill(0.5 * (1.0 - amount));
        result
    }

    fn grayscale(amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let inverse = 1.0 - amount;
        Self {
            rows: [
                [0.2126 * amount + inverse, 0.7152 * amount, 0.0722 * amount, 0.0],
                [0.2126 * amount, 0.7152 * amount + inverse, 0.0722 * amount, 0.0],
                [0.2126 * amount, 0.7152 * amount, 0.0722 * amount + inverse, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            offset: [0.0; 4],
        }
    }

    fn hue_rotate(degrees: f32) -> Self {
        let radians = degrees.to_radians();
        let cosine = radians.cos();
        let sine = radians.sin();
        Self {
            rows: [
                [
                    0.213 + cosine * 0.787 - sine * 0.213,
                    0.715 - cosine * 0.715 - sine * 0.715,
                    0.072 - cosine * 0.072 + sine * 0.928,
                    0.0,
                ],
                [
                    0.213 - cosine * 0.213 + sine * 0.143,
                    0.715 + cosine * 0.285 + sine * 0.140,
                    0.072 - cosine * 0.072 - sine * 0.283,
                    0.0,
                ],
                [
                    0.213 - cosine * 0.213 - sine * 0.787,
                    0.715 - cosine * 0.715 + sine * 0.715,
                    0.072 + cosine * 0.928 + sine * 0.072,
                    0.0,
                ],
                [0.0, 0.0, 0.0, 1.0],
            ],
            offset: [0.0; 4],
        }
    }

    fn invert(amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let scale = 1.0 - 2.0 * amount;
        let mut result = Self::identity();
        for index in 0..3 {
            result.rows[index][index] = scale;
        }
        result.offset[..3].fill(amount);
        result
    }

    fn opacity(amount: f32) -> Self {
        let mut result = Self::identity();
        result.rows[3][3] = amount.clamp(0.0, 1.0);
        result
    }

    fn saturate(amount: f32) -> Self {
        let amount = amount.max(0.0);
        Self {
            rows: [
                [0.213 + 0.787 * amount, 0.715 - 0.715 * amount, 0.072 - 0.072 * amount, 0.0],
                [0.213 - 0.213 * amount, 0.715 + 0.285 * amount, 0.072 - 0.072 * amount, 0.0],
                [0.213 - 0.213 * amount, 0.715 - 0.715 * amount, 0.072 + 0.928 * amount, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            offset: [0.0; 4],
        }
    }

    fn sepia(amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let inverse = 1.0 - amount;
        Self {
            rows: [
                [0.393 * amount + inverse, 0.769 * amount, 0.189 * amount, 0.0],
                [0.349 * amount, 0.686 * amount + inverse, 0.168 * amount, 0.0],
                [0.272 * amount, 0.534 * amount, 0.131 * amount + inverse, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            offset: [0.0; 4],
        }
    }
}

fn panel_color_effect(pool_index: usize) -> ColorParams {
    match pool_index {
        0 => ColorParams::identity()
            .then(ColorParams::brightness(0.65))
            .then(ColorParams::contrast(1.35)),
        1 => ColorParams::grayscale(1.0),
        2 => ColorParams::hue_rotate(120.0),
        3 => ColorParams::invert(1.0).then(ColorParams::opacity(0.78)),
        4 => ColorParams::saturate(2.0).then(ColorParams::sepia(0.65)),
        _ => ColorParams::identity()
            .then(ColorParams::brightness(1.15))
            .then(ColorParams::contrast(1.2))
            .then(ColorParams::grayscale(0.15))
            .then(ColorParams::hue_rotate(-25.0))
            .then(ColorParams::invert(0.08))
            .then(ColorParams::opacity(0.85))
            .then(ColorParams::saturate(1.6))
            .then(ColorParams::sepia(0.25)),
    }
}

#[derive(Clone, Copy)]
struct PanelRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
}

#[derive(Clone, Copy)]
struct CaptureRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Clone)]
struct SpikeConfig {
    width: u32,
    height: u32,
    warmup: Duration,
    sample: Duration,
    target_frame: Duration,
    blur_radius: f32,
    scale_factor: f32,
    color_effects: bool,
    showcase_layout: bool,
    downsample: u32,
    panel_count: usize,
}

impl SpikeConfig {
    fn from_environment() -> Self {
        let target_fps = read_environment("SLINT_BLUR_SPIKE_TARGET_FPS", 60_u64).max(1);
        Self {
            width: read_environment("SLINT_BLUR_SPIKE_WIDTH", 2560_u32).max(1),
            height: read_environment("SLINT_BLUR_SPIKE_HEIGHT", 1440_u32).max(1),
            warmup: Duration::from_secs(read_environment(
                "SLINT_BLUR_SPIKE_WARMUP_SECONDS",
                10_u64,
            )),
            sample: Duration::from_secs(read_environment(
                "SLINT_BLUR_SPIKE_SAMPLE_SECONDS",
                60_u64,
            )),
            target_frame: Duration::from_secs_f64(1.0 / target_fps as f64),
            blur_radius: read_environment("SLINT_BLUR_SPIKE_RADIUS", 18.0_f32).clamp(0.0, 32.0),
            scale_factor: read_environment("SLINT_BLUR_SPIKE_SCALE_FACTOR", 1.0_f32)
                .clamp(0.5, 4.0),
            color_effects: read_environment("SLINT_BLUR_SPIKE_COLOR_EFFECTS", 1_u8) != 0,
            showcase_layout: std::env::var("SLINT_BLUR_SPIKE_LAYOUT")
                .is_ok_and(|layout| layout.eq_ignore_ascii_case("showcase")),
            downsample: read_environment("SLINT_BLUR_SPIKE_DOWNSAMPLE", 2_u32).clamp(1, 4),
            panel_count: match read_environment("SLINT_BLUR_SPIKE_PANEL_COUNT", 4_usize) {
                count @ (0 | 1 | 4 | 6) => count,
                _ => 4,
            },
        }
    }

    fn panel_indices(&self) -> &'static [usize] {
        match self.panel_count {
            0 => &NO_PANEL_INDICES,
            1 => &ONE_PANEL_INDICES,
            6 => &SIX_PANEL_INDICES,
            _ => &FOUR_PANEL_INDICES,
        }
    }
}

fn read_environment<T>(name: &str, default: T) -> T
where
    T: std::str::FromStr,
{
    std::env::var(name).ok().and_then(|value| value.parse().ok()).unwrap_or(default)
}

struct BackdropPipeline {
    capture_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    color_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl BackdropPipeline {
    fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("backdrop blur spike bind group layout"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
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
            label: Some("backdrop blur spike pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("backdrop blur spike shader"),
            source: wgpu::ShaderSource::Wgsl(BACKDROP_SHADER.into()),
        });
        let create_pipeline = |label, fragment_entry_point| {
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
                    entry_point: Some(fragment_entry_point),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: TARGET_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };

        Self {
            capture_pipeline: create_pipeline(
                "backdrop blur spike capture pipeline",
                "capture_fragment",
            ),
            blur_pipeline: create_pipeline("backdrop blur spike blur pipeline", "blur_fragment"),
            color_pipeline: create_pipeline("backdrop blur spike color pipeline", "color_fragment"),
            bind_group_layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("backdrop blur spike clamp sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
        }
    }
}

struct PanelTargets {
    capture: wgpu::Texture,
    horizontal: wgpu::Texture,
    blurred: wgpu::Texture,
    filtered: wgpu::Texture,
    blurred_image: ImageId,
    filtered_image: ImageId,
    capture_params: wgpu::Buffer,
    horizontal_params: wgpu::Buffer,
    vertical_params: wgpu::Buffer,
    color_params: wgpu::Buffer,
    capture_bind_group: wgpu::BindGroup,
    horizontal_bind_group: wgpu::BindGroup,
    vertical_bind_group: wgpu::BindGroup,
    color_bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

impl PanelTargets {
    fn new(
        device: &wgpu::Device,
        canvas: &mut Canvas<femtovg::renderer::WGPURenderer>,
        pipeline: &BackdropPipeline,
        scene_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) -> Result<Self, femtovg::ErrorKind> {
        let create_texture = |label| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TARGET_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };
        let capture = create_texture("backdrop blur spike capture texture");
        let horizontal = create_texture("backdrop blur spike horizontal texture");
        let blurred = create_texture("backdrop blur spike result texture");
        let filtered = create_texture("backdrop blur spike color-filtered texture");
        let capture_params =
            create_uniform_buffer::<CaptureParams>(device, "backdrop blur spike capture uniforms");
        let horizontal_params =
            create_uniform_buffer::<BlurParams>(device, "backdrop blur spike horizontal uniforms");
        let vertical_params =
            create_uniform_buffer::<BlurParams>(device, "backdrop blur spike vertical uniforms");
        let color_params =
            create_uniform_buffer::<ColorParams>(device, "backdrop blur spike color uniforms");
        let unused_capture_params = create_uniform_buffer::<CaptureParams>(
            device,
            "backdrop blur spike unused capture uniforms",
        );
        let unused_blur_params =
            create_uniform_buffer::<BlurParams>(device, "backdrop blur spike unused blur uniforms");
        let unused_color_params = create_uniform_buffer::<ColorParams>(
            device,
            "backdrop blur spike unused color uniforms",
        );
        let capture_bind_group = create_bind_group(
            device,
            pipeline,
            scene_view,
            &capture_params,
            &unused_blur_params,
            &unused_color_params,
            "backdrop blur spike capture bind group",
        );
        let horizontal_bind_group = create_bind_group(
            device,
            pipeline,
            &capture.create_view(&Default::default()),
            &unused_capture_params,
            &horizontal_params,
            &unused_color_params,
            "backdrop blur spike horizontal bind group",
        );
        let vertical_bind_group = create_bind_group(
            device,
            pipeline,
            &horizontal.create_view(&Default::default()),
            &unused_capture_params,
            &vertical_params,
            &unused_color_params,
            "backdrop blur spike vertical bind group",
        );
        let color_bind_group = create_bind_group(
            device,
            pipeline,
            &blurred.create_view(&Default::default()),
            &unused_capture_params,
            &unused_blur_params,
            &color_params,
            "backdrop blur spike color bind group",
        );
        let blurred_image = canvas.create_image_from_native_texture(
            blurred.clone(),
            ImageInfo::new(
                ImageFlags::PREMULTIPLIED,
                width as usize,
                height as usize,
                PixelFormat::Rgba8,
            ),
        )?;
        let filtered_image = canvas.create_image_from_native_texture(
            filtered.clone(),
            ImageInfo::new(
                ImageFlags::PREMULTIPLIED,
                width as usize,
                height as usize,
                PixelFormat::Rgba8,
            ),
        )?;

        Ok(Self {
            capture,
            horizontal,
            blurred,
            filtered,
            blurred_image,
            filtered_image,
            capture_params,
            horizontal_params,
            vertical_params,
            color_params,
            capture_bind_group,
            horizontal_bind_group,
            vertical_bind_group,
            color_bind_group,
            width,
            height,
        })
    }
}

fn create_uniform_buffer<T: Pod>(device: &wgpu::Device, label: &'static str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: std::mem::size_of::<T>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_bind_group(
    device: &wgpu::Device,
    pipeline: &BackdropPipeline,
    source_view: &wgpu::TextureView,
    capture_params: &wgpu::Buffer,
    blur_params: &wgpu::Buffer,
    color_params: &wgpu::Buffer,
    label: &'static str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout: &pipeline.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&pipeline.sampler),
            },
            wgpu::BindGroupEntry { binding: 2, resource: capture_params.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: blur_params.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 4, resource: color_params.as_entire_binding() },
        ],
    })
}

struct TexturePool {
    panels: Vec<PanelTargets>,
    texture_allocations: u64,
}

impl TexturePool {
    fn new(
        device: &wgpu::Device,
        canvas: &mut Canvas<femtovg::renderer::WGPURenderer>,
        pipeline: &BackdropPipeline,
        scene_view: &wgpu::TextureView,
        panel_sizes: &[(u32, u32)],
    ) -> Result<Self, femtovg::ErrorKind> {
        let mut panels = Vec::with_capacity(panel_sizes.len());
        for &(width, height) in panel_sizes {
            panels.push(PanelTargets::new(device, canvas, pipeline, scene_view, width, height)?);
        }
        Ok(Self { panels, texture_allocations: (panel_sizes.len() * 4) as u64 })
    }

    fn delete_images(&self, canvas: &mut Canvas<femtovg::renderer::WGPURenderer>) {
        for panel in &self.panels {
            canvas.delete_image(panel.blurred_image);
            canvas.delete_image(panel.filtered_image);
        }
    }
}

struct GpuTimer {
    query_set: wgpu::QuerySet,
    resolve_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    timestamp_period: f32,
    query_count: u32,
}

impl GpuTimer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, panel_count: usize) -> Self {
        let query_count = (panel_count * 2) as u32;
        let buffer_size = panel_count as u64 * 16;
        Self {
            query_set: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("backdrop blur spike pass timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: query_count,
            }),
            resolve_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("backdrop blur spike timestamp resolve buffer"),
                size: buffer_size,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("backdrop blur spike timestamp readback buffer"),
                size: buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            timestamp_period: queue.get_timestamp_period(),
            query_count,
        }
    }

    fn finish(&self, device: &wgpu::Device) -> wgpu::CommandBuffer {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("backdrop blur spike timestamp finish encoder"),
        });
        let buffer_size = self.query_count as u64 * 8;
        encoder.resolve_query_set(&self.query_set, 0..self.query_count, &self.resolve_buffer, 0);
        encoder.copy_buffer_to_buffer(
            &self.resolve_buffer,
            0,
            &self.readback_buffer,
            0,
            buffer_size,
        );
        encoder.finish()
    }

    fn read_milliseconds(&self, device: &wgpu::Device) -> Result<f64, Box<dyn Error>> {
        let slice = self.readback_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        device.poll(wgpu::PollType::wait_indefinitely())?;
        receiver.recv()??;
        let mapped = slice.get_mapped_range();
        let timestamps: &[u64] = bytemuck::cast_slice(&mapped);
        let elapsed =
            timestamps.chunks_exact(2).map(|pair| pair[1].saturating_sub(pair[0])).sum::<u64>();
        let milliseconds = elapsed as f64 * self.timestamp_period as f64 / 1_000_000.0;
        drop(mapped);
        self.readback_buffer.unmap();
        Ok(milliseconds)
    }
}

struct Metrics {
    synchronized_frame_ms: Vec<f64>,
    gpu_frame_ms: Vec<f64>,
    missed_frames: usize,
}

impl Metrics {
    fn new() -> Self {
        Self { synchronized_frame_ms: Vec::new(), gpu_frame_ms: Vec::new(), missed_frames: 0 }
    }

    fn record(&mut self, frame_time: Duration, gpu_time_ms: Option<f64>, target: Duration) {
        self.synchronized_frame_ms.push(frame_time.as_secs_f64() * 1000.0);
        if let Some(gpu_time_ms) = gpu_time_ms {
            self.gpu_frame_ms.push(gpu_time_ms);
        }
        if frame_time > target {
            self.missed_frames += 1;
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = SpikeConfig::from_environment();
    if read_environment("SLINT_BLUR_SPIKE_INTERACTIVE", 0_u8) != 0 {
        return run_interactive(config);
    }
    run_benchmark(config)
}

fn run_benchmark(config: SpikeConfig) -> Result<(), Box<dyn Error>> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::from_env().unwrap_or_default(),
        flags: wgpu::InstanceFlags::from_build_config().with_env(),
        backend_options: wgpu::BackendOptions::from_env_or_default(),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        display: None,
    });
    let adapter = spin_on::spin_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))?;
    let timestamp_features =
        wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
    let timestamps_available = adapter.features().contains(timestamp_features);
    let timestamps_requested = read_environment("SLINT_BLUR_SPIKE_TIMESTAMPS", 1_u8) != 0;
    let timestamps_enabled = timestamps_available && timestamps_requested;
    let enabled_timestamp_features =
        if timestamps_enabled { timestamp_features } else { wgpu::Features::empty() };
    let (device, queue) = spin_on::spin_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("backdrop blur spike device"),
        required_features: enabled_timestamp_features,
        required_limits: adapter.limits(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        trace: wgpu::Trace::default(),
    }))?;

    let create_frame_texture = |label| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    };
    let scene_texture = create_frame_texture("backdrop blur spike scene texture");
    let output_texture = create_frame_texture("backdrop blur spike output texture");
    let scene_view = scene_texture.create_view(&Default::default());

    let femtovg_renderer = femtovg::renderer::WGPURenderer::new(device.clone(), queue.clone());
    let mut canvas = Canvas::new(femtovg_renderer)?;
    canvas.set_size(config.width, config.height, 1.0);
    let pipeline = BackdropPipeline::new(&device);
    let initial_panels = animated_panels(&config, 0.0);
    let panel_sizes = config
        .panel_indices()
        .iter()
        .enumerate()
        .map(|(pool_index, &source_index)| {
            panel_target_size(
                &config,
                initial_panels[source_index],
                panel_blur_radius(&config, pool_index),
            )
        })
        .collect::<Vec<_>>();
    let mut pool = TexturePool::new(&device, &mut canvas, &pipeline, &scene_view, &panel_sizes)?;
    let gpu_timer = (timestamps_enabled && config.panel_count > 0)
        .then(|| GpuTimer::new(&device, &queue, config.panel_count));

    println!(
        "backdrop-blur-spike adapter={:?} size={}x{} panels={} warmup_s={} sample_s={} radius={} scale_factor={} downsample={} color_effects={} timestamps={}",
        adapter.get_info().name,
        config.width,
        config.height,
        config.panel_count,
        config.warmup.as_secs(),
        config.sample.as_secs(),
        config.blur_radius,
        config.scale_factor,
        config.downsample,
        config.color_effects,
        timestamps_enabled,
    );

    let start = Instant::now();
    let sample_start = start + config.warmup;
    let finish = sample_start + config.sample;
    let allocations_at_sample_start = pool.texture_allocations;
    let mut metrics = Metrics::new();
    let mut command_buffers = Vec::with_capacity(config.panel_count * 2 + 3);
    while Instant::now() < finish {
        let frame_start = Instant::now();
        let animation_time = start.elapsed().as_secs_f32();
        command_buffers.clear();

        draw_animated_background(&mut canvas, &config, animation_time);
        push_canvas_commands(
            &mut canvas,
            &scene_texture,
            config.width,
            config.height,
            &mut command_buffers,
        );

        let panels = animated_panels(&config, animation_time);
        for (pool_index, source_index) in config.panel_indices().iter().copied().enumerate() {
            let panel = panels[source_index];
            let blur_radius = panel_blur_radius(&config, pool_index);
            let capture_rect = padded_capture_rect(panel, &config, blur_radius);
            let targets = &mut pool.panels[pool_index];
            update_panel_uniforms(
                &queue,
                targets,
                capture_rect,
                &config,
                blur_radius,
                panel_color_effect(pool_index),
            );
            command_buffers.push(encode_capture_and_blur(
                &device,
                &pipeline,
                targets,
                gpu_timer.as_ref(),
                pool_index,
                config.color_effects,
            ));
            let image =
                if config.color_effects { targets.filtered_image } else { targets.blurred_image };
            draw_glass_panel(&mut canvas, image, panel, capture_rect);
            push_canvas_commands(
                &mut canvas,
                &scene_texture,
                config.width,
                config.height,
                &mut command_buffers,
            );
        }

        command_buffers.push(encode_output_copy(
            &device,
            &scene_texture,
            &output_texture,
            config.width,
            config.height,
        ));
        if let Some(timer) = &gpu_timer {
            command_buffers.push(timer.finish(&device));
        }
        queue.submit(command_buffers.drain(..));
        let gpu_time_ms = if let Some(timer) = &gpu_timer {
            device.poll(wgpu::PollType::wait_indefinitely())?;
            Some(timer.read_milliseconds(&device)?)
        } else {
            None
        };
        let frame_time = frame_start.elapsed();

        if frame_start >= sample_start {
            metrics.record(frame_time, gpu_time_ms, config.target_frame);
        }
        if frame_time < config.target_frame {
            std::thread::sleep(config.target_frame - frame_time);
        }
    }

    device.poll(wgpu::PollType::wait_indefinitely())?;
    print_metrics(&metrics, allocations_at_sample_start, pool.texture_allocations);
    Ok(())
}

struct InteractiveApp {
    config: SpikeConfig,
    renderer: Option<InteractiveRenderer>,
    device_generation: u32,
    pending_resize: Option<(winit::dpi::PhysicalSize<u32>, Instant)>,
}

impl InteractiveApp {
    fn new(config: SpikeConfig) -> Self {
        Self { config, renderer: None, device_generation: 0, pending_resize: None }
    }

    fn install_renderer(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: Option<Arc<Window>>,
    ) -> Result<(), Box<dyn Error>> {
        let renderer = match window {
            Some(window) => InteractiveRenderer::from_window(window, self.config.clone())?,
            None => InteractiveRenderer::new(event_loop, self.config.clone())?,
        };
        self.device_generation += 1;
        renderer.window.set_title(&format!(
            "Retrospect Backdrop Blur Test — {} panels — device {} — R recreates — Esc closes",
            renderer.config.panel_count, self.device_generation
        ));
        renderer.window.request_redraw();
        self.config = renderer.config.clone();
        self.renderer = Some(renderer);
        self.pending_resize = None;
        Ok(())
    }
}

impl ApplicationHandler for InteractiveApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }

        if let Err(error) = self.install_renderer(event_loop, None) {
            eprintln!("backdrop blur test app failed to start: {error}");
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.renderer.as_ref().is_none_or(|renderer| renderer.window.id() != window_id) {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state.is_pressed()
                    && event.logical_key == Key::Named(NamedKey::Escape) =>
            {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state.is_pressed()
                    && event.physical_key == PhysicalKey::Code(KeyCode::KeyR) =>
            {
                let old_renderer = self.renderer.take().expect("renderer exists for its window");
                let window = old_renderer.window.clone();
                self.config = old_renderer.config.clone();
                drop(old_renderer);
                if let Err(error) = self.install_renderer(event_loop, Some(window)) {
                    eprintln!("backdrop blur test app device recreation failed: {error}");
                    event_loop.exit();
                } else {
                    println!(
                        "backdrop-blur-test-app device_recreated generation={}",
                        self.device_generation
                    );
                }
            }
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                self.pending_resize = Some((size, Instant::now()));
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.config.scale_factor = scale_factor as f32;
                    self.pending_resize = Some((renderer.window.inner_size(), Instant::now()));
                }
            }
            WindowEvent::RedrawRequested => {
                let renderer = self.renderer.as_mut().expect("renderer exists for redraw");
                if self.pending_resize.is_some() {
                    renderer.window.request_redraw();
                    return;
                }
                if let Err(error) = renderer.render() {
                    eprintln!("backdrop blur test app render failed: {error}");
                    event_loop.exit();
                    return;
                }
                renderer.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some((size, changed_at)) = self.pending_resize else { return };
        if changed_at.elapsed() < Duration::from_millis(120) {
            return;
        }
        let Some(renderer) = self.renderer.as_mut() else { return };
        if let Err(error) = renderer.resize(size.width.max(1), size.height.max(1)) {
            eprintln!("backdrop blur test app coalesced resize failed: {error}");
            event_loop.exit();
            return;
        }
        self.config = renderer.config.clone();
        self.pending_resize = None;
        renderer.window.request_redraw();
    }
}

struct InteractiveRenderer {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: SpikeConfig,
    canvas: Canvas<femtovg::renderer::WGPURenderer>,
    pipeline: BackdropPipeline,
    pool: TexturePool,
    scene_texture: wgpu::Texture,
    scene_image: ImageId,
    started_at: Instant,
}

impl InteractiveRenderer {
    fn new(event_loop: &ActiveEventLoop, config: SpikeConfig) -> Result<Self, Box<dyn Error>> {
        let attributes = WindowAttributes::default()
            .with_title(format!(
                "Retrospect Backdrop Blur Test — {} live panels — Esc to close",
                config.panel_count
            ))
            .with_inner_size(winit::dpi::PhysicalSize::new(config.width, config.height))
            .with_resizable(true);
        let window = Arc::new(event_loop.create_window(attributes)?);
        Self::from_window(window, config)
    }

    fn from_window(window: Arc<Window>, mut config: SpikeConfig) -> Result<Self, Box<dyn Error>> {
        let size = window.inner_size();
        config.width = size.width.max(1);
        config.height = size.height.max(1);
        if std::env::var_os("SLINT_BLUR_SPIKE_SCALE_FACTOR").is_none() {
            config.scale_factor = window.scale_factor() as f32;
        }
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::from_env().unwrap_or_default(),
            flags: wgpu::InstanceFlags::from_build_config().with_env(),
            backend_options: wgpu::BackendOptions::from_env_or_default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            display: None,
        });
        let surface = instance.create_surface(window.clone())?;
        let adapter = spin_on::spin_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))?;
        let (device, queue) = spin_on::spin_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("backdrop blur test app device"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::default(),
        }))?;
        let mut surface_config = surface
            .get_default_config(&adapter, config.width, config.height)
            .ok_or("the selected WGPU adapter cannot present to this window")?;
        let capabilities = surface.get_capabilities(&adapter);
        surface_config.format = capabilities
            .formats
            .iter()
            .find(|format| {
                matches!(format, wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm)
            })
            .copied()
            .unwrap_or(capabilities.formats[0]);
        surface_config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &surface_config);

        let scene_texture = create_frame_texture(
            &device,
            "backdrop blur test app scene texture",
            config.width,
            config.height,
        );
        let scene_view = scene_texture.create_view(&Default::default());
        let femtovg_renderer = femtovg::renderer::WGPURenderer::new(device.clone(), queue.clone());
        let mut canvas = Canvas::new(femtovg_renderer)?;
        canvas.set_size(config.width, config.height, 1.0);
        let scene_image = canvas.create_image_from_native_texture(
            scene_texture.clone(),
            ImageInfo::new(
                ImageFlags::PREMULTIPLIED,
                config.width as usize,
                config.height as usize,
                PixelFormat::Rgba8,
            ),
        )?;
        let pipeline = BackdropPipeline::new(&device);
        let initial_panels = animated_panels(&config, 0.0);
        let panel_sizes = config
            .panel_indices()
            .iter()
            .enumerate()
            .map(|(pool_index, &source_index)| {
                panel_target_size(
                    &config,
                    initial_panels[source_index],
                    panel_blur_radius(&config, pool_index),
                )
            })
            .collect::<Vec<_>>();
        let pool = TexturePool::new(&device, &mut canvas, &pipeline, &scene_view, &panel_sizes)?;

        println!(
            "backdrop-blur-test-app adapter={:?} size={}x{} panels={} radius={} scale_factor={} downsample={} color_effects={} surface_format={:?}",
            adapter.get_info().name,
            config.width,
            config.height,
            config.panel_count,
            config.blur_radius,
            config.scale_factor,
            config.downsample,
            config.color_effects,
            surface_config.format,
        );

        Ok(Self {
            window,
            surface,
            surface_config,
            device,
            queue,
            config,
            canvas,
            pipeline,
            pool,
            scene_texture,
            scene_image,
            started_at: Instant::now(),
        })
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), Box<dyn Error>> {
        self.device.poll(wgpu::PollType::wait_indefinitely())?;
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.config.width = width;
        self.config.height = height;
        self.canvas.set_size(width, height, 1.0);

        let scene_texture = create_frame_texture(
            &self.device,
            "backdrop blur resized scene texture",
            width,
            height,
        );
        let scene_view = scene_texture.create_view(&Default::default());
        let scene_image = self.canvas.create_image_from_native_texture(
            scene_texture.clone(),
            ImageInfo::new(
                ImageFlags::PREMULTIPLIED,
                width as usize,
                height as usize,
                PixelFormat::Rgba8,
            ),
        )?;
        let panels = animated_panels(&self.config, self.started_at.elapsed().as_secs_f32());
        let panel_sizes = self
            .config
            .panel_indices()
            .iter()
            .enumerate()
            .map(|(pool_index, &source_index)| {
                panel_target_size(
                    &self.config,
                    panels[source_index],
                    panel_blur_radius(&self.config, pool_index),
                )
            })
            .collect::<Vec<_>>();
        let pool = TexturePool::new(
            &self.device,
            &mut self.canvas,
            &self.pipeline,
            &scene_view,
            &panel_sizes,
        )?;

        self.canvas.delete_image(self.scene_image);
        self.pool.delete_images(&mut self.canvas);
        self.scene_texture = scene_texture;
        self.scene_image = scene_image;
        self.pool = pool;
        println!(
            "backdrop-blur-test-app resized={}x{} scale_factor={:.2} texture_allocations={}",
            width, height, self.config.scale_factor, self.pool.texture_allocations
        );
        Ok(())
    }

    fn render(&mut self) -> Result<(), Box<dyn Error>> {
        let surface_frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.surface_config);
                match self.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                    _ => return Ok(()),
                }
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("WGPU rejected the backdrop blur presentation surface".into());
            }
        };
        let animation_time = self.started_at.elapsed().as_secs_f32();
        let mut command_buffers = Vec::with_capacity(self.config.panel_count * 2 + 2);

        draw_animated_background(&mut self.canvas, &self.config, animation_time);
        push_canvas_commands(
            &mut self.canvas,
            &self.scene_texture,
            self.config.width,
            self.config.height,
            &mut command_buffers,
        );
        let panels = animated_panels(&self.config, animation_time);
        for (pool_index, source_index) in self.config.panel_indices().iter().copied().enumerate() {
            let panel = panels[source_index];
            let blur_radius = panel_blur_radius(&self.config, pool_index);
            let capture_rect = padded_capture_rect(panel, &self.config, blur_radius);
            let targets = &mut self.pool.panels[pool_index];
            update_panel_uniforms(
                &self.queue,
                targets,
                capture_rect,
                &self.config,
                blur_radius,
                panel_color_effect(pool_index),
            );
            command_buffers.push(encode_capture_and_blur(
                &self.device,
                &self.pipeline,
                targets,
                None,
                pool_index,
                self.config.color_effects,
            ));
            let image = if self.config.color_effects {
                targets.filtered_image
            } else {
                targets.blurred_image
            };
            draw_glass_panel(&mut self.canvas, image, panel, capture_rect);
            push_canvas_commands(
                &mut self.canvas,
                &self.scene_texture,
                self.config.width,
                self.config.height,
                &mut command_buffers,
            );
        }

        draw_scene_to_surface(
            &mut self.canvas,
            self.scene_image,
            &surface_frame.texture,
            self.surface_config.format,
            self.config.width,
            self.config.height,
            &mut command_buffers,
        );
        self.queue.submit(command_buffers);
        surface_frame.present();
        Ok(())
    }
}

fn run_interactive(config: SpikeConfig) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut InteractiveApp::new(config))?;
    Ok(())
}

fn create_frame_texture(
    device: &wgpu::Device,
    label: &'static str,
    width: u32,
    height: u32,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TARGET_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn draw_scene_to_surface(
    canvas: &mut Canvas<femtovg::renderer::WGPURenderer>,
    scene_image: ImageId,
    surface_texture: &wgpu::Texture,
    surface_format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    command_buffers: &mut Vec<wgpu::CommandBuffer>,
) {
    let mut frame = Path::new();
    frame.rect(0.0, 0.0, width as f32, height as f32);
    canvas.fill_path(
        &frame,
        &Paint::image(scene_image, 0.0, 0.0, width as f32, height as f32, 0.0, 1.0)
            .with_anti_alias(false),
    );
    let output = femtovg::renderer::WGPURenderOutput {
        view: surface_texture.create_view(&Default::default()),
        width,
        height,
        format: surface_format,
    };
    if let Some(command_buffer) = canvas.flush_to_output(output) {
        command_buffers.push(command_buffer);
    }
}

fn panel_blur_radius(config: &SpikeConfig, pool_index: usize) -> f32 {
    let step = SIX_PANEL_BLUR_STEPS[pool_index.min(SIX_PANEL_BLUR_STEPS.len() - 1)];
    (step / 32.0 * config.blur_radius * config.scale_factor).max(0.5)
}

fn panel_target_size(config: &SpikeConfig, panel: PanelRect, blur_radius: f32) -> (u32, u32) {
    let padding = blur_radius.ceil() * 2.0;
    let width = (panel.width + padding).ceil() as u32;
    let height = (panel.height + padding).ceil() as u32;
    (width.div_ceil(config.downsample), height.div_ceil(config.downsample))
}

fn draw_animated_background(
    canvas: &mut Canvas<femtovg::renderer::WGPURenderer>,
    config: &SpikeConfig,
    time: f32,
) {
    canvas.clear_rect(0, 0, config.width, config.height, Color::rgb(12, 15, 24));
    let mut background = Path::new();
    background.rect(0.0, 0.0, config.width as f32, config.height as f32);
    canvas.fill_path(
        &background,
        &Paint::linear_gradient(
            0.0,
            0.0,
            config.width as f32,
            config.height as f32,
            Color::rgb(20, 25, 46),
            Color::rgb(8, 10, 18),
        ),
    );

    for index in 0..28 {
        let phase = time * (0.25 + index as f32 * 0.006) + index as f32 * 0.71;
        let x = (index % 7) as f32 * config.width as f32 / 6.0 + phase.sin() * 90.0;
        let y = (index / 7) as f32 * config.height as f32 / 3.0 + phase.cos() * 75.0;
        let radius = 44.0 + (index % 5) as f32 * 16.0;
        let mut orb = Path::new();
        orb.circle(x, y, radius);
        let color = if index % 2 == 0 {
            Color::rgba(93, 141, 255, 205)
        } else {
            Color::rgba(230, 97, 177, 190)
        };
        canvas.fill_path(&orb, &Paint::color(color));
    }

    for index in 0..16 {
        let x = 80.0 + index as f32 * config.width as f32 / 16.0;
        let y = config.height as f32 * 0.43 + (time * 1.2 + index as f32).sin() * 110.0;
        let mut bar = Path::new();
        bar.rounded_rect(x, y, 96.0, 280.0, 24.0);
        canvas.fill_path(&bar, &Paint::color(Color::rgba(240, 244, 255, 62)));
    }
}

fn animated_panels(config: &SpikeConfig, time: f32) -> [PanelRect; 6] {
    if config.showcase_layout {
        showcase_panels(config, time)
    } else {
        production_panels(config, time)
    }
}

fn showcase_panels(config: &SpikeConfig, time: f32) -> [PanelRect; 6] {
    let width = config.width as f32;
    let height = config.height as f32;
    let drift = |phase: f32| (time * 0.45 + phase).sin() * 10.0;
    [
        PanelRect {
            x: width * 0.07 + drift(0.0),
            y: height * 0.07 + drift(0.8),
            width: width * 0.27,
            height: height * 0.19,
            radius: 30.0,
        },
        PanelRect {
            x: width * 0.66 + drift(0.9),
            y: height * 0.08 + drift(1.7),
            width: width * 0.27,
            height: height * 0.2,
            radius: 30.0,
        },
        PanelRect {
            x: width * 0.37 + drift(1.8),
            y: height * 0.36 + drift(2.6),
            width: width * 0.3,
            height: height * 0.24,
            radius: 34.0,
        },
        PanelRect {
            x: width * 0.08 + drift(2.7),
            y: height * 0.7 + drift(3.5),
            width: width * 0.26,
            height: height * 0.19,
            radius: 30.0,
        },
        PanelRect {
            x: width * 0.64 + drift(3.6),
            y: height * 0.68 + drift(4.4),
            width: width * 0.28,
            height: height * 0.21,
            radius: 32.0,
        },
        PanelRect {
            x: width * 0.025 + drift(4.5),
            y: height * 0.21 + drift(5.3),
            width: width * 0.15,
            height: height * 0.48,
            radius: 36.0,
        },
    ]
}

fn production_panels(config: &SpikeConfig, time: f32) -> [PanelRect; 6] {
    let width = config.width as f32;
    let height = config.height as f32;
    let drift = |phase: f32| (time * 0.45 + phase).sin() * 10.0;
    let horizontal_margin = width * 0.018;
    let gap = width * 0.008;
    let rail_width = (width - horizontal_margin * 2.0 - gap * 4.0) / 5.0;
    let rail_y = height * 0.03;
    let rail_height = height * 0.085;
    let rail = |index: usize, phase: f32| PanelRect {
        x: horizontal_margin + index as f32 * (rail_width + gap) + drift(phase),
        y: rail_y + drift(phase + 0.8),
        width: rail_width,
        height: rail_height,
        radius: 30.0,
    };
    [
        rail(0, 0.0),
        rail(1, 0.9),
        rail(2, 1.8),
        rail(3, 2.7),
        rail(4, 3.6),
        PanelRect {
            x: horizontal_margin + drift(4.5),
            y: height * 0.095 + drift(5.3),
            width: width * 0.17,
            height: height * 0.74,
            radius: 36.0,
        },
    ]
}

fn padded_capture_rect(panel: PanelRect, config: &SpikeConfig, blur_radius: f32) -> CaptureRect {
    let padding = blur_radius.ceil();
    let x = (panel.x - padding).max(0.0);
    let y = (panel.y - padding).max(0.0);
    let right = (panel.x + panel.width + padding).min(config.width as f32);
    let bottom = (panel.y + panel.height + padding).min(config.height as f32);
    CaptureRect { x, y, width: right - x, height: bottom - y }
}

fn update_panel_uniforms(
    queue: &wgpu::Queue,
    targets: &PanelTargets,
    capture: CaptureRect,
    config: &SpikeConfig,
    blur_radius: f32,
    color_effect: ColorParams,
) {
    let capture_params = CaptureParams {
        uv_origin: [capture.x / config.width as f32, capture.y / config.height as f32],
        uv_size: [capture.width / config.width as f32, capture.height / config.height as f32],
    };
    let sigma = (blur_radius / 2.0 / config.downsample as f32).min(8.0);
    let texel_size = [1.0 / targets.width as f32, 1.0 / targets.height as f32];
    let horizontal = BlurParams { direction: [1.0, 0.0], texel_size, sigma, _padding: [0.0; 3] };
    let vertical = BlurParams { direction: [0.0, 1.0], ..horizontal };
    queue.write_buffer(&targets.capture_params, 0, bytemuck::bytes_of(&capture_params));
    queue.write_buffer(&targets.horizontal_params, 0, bytemuck::bytes_of(&horizontal));
    queue.write_buffer(&targets.vertical_params, 0, bytemuck::bytes_of(&vertical));
    queue.write_buffer(&targets.color_params, 0, bytemuck::bytes_of(&color_effect));
}

fn encode_capture_and_blur(
    device: &wgpu::Device,
    pipeline: &BackdropPipeline,
    targets: &PanelTargets,
    gpu_timer: Option<&GpuTimer>,
    panel_index: usize,
    color_effects: bool,
) -> wgpu::CommandBuffer {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("backdrop blur spike capture and blur encoder"),
    });
    if let Some(timer) = gpu_timer {
        encoder.write_timestamp(&timer.query_set, (panel_index * 2) as u32);
    }
    render_fullscreen_pass(
        &mut encoder,
        &targets.capture,
        &pipeline.capture_pipeline,
        &targets.capture_bind_group,
        "backdrop blur spike capture pass",
    );
    render_fullscreen_pass(
        &mut encoder,
        &targets.horizontal,
        &pipeline.blur_pipeline,
        &targets.horizontal_bind_group,
        "backdrop blur spike horizontal pass",
    );
    render_fullscreen_pass(
        &mut encoder,
        &targets.blurred,
        &pipeline.blur_pipeline,
        &targets.vertical_bind_group,
        "backdrop blur spike vertical pass",
    );
    if color_effects {
        render_fullscreen_pass(
            &mut encoder,
            &targets.filtered,
            &pipeline.color_pipeline,
            &targets.color_bind_group,
            "backdrop blur spike color pass",
        );
    }
    if let Some(timer) = gpu_timer {
        encoder.write_timestamp(&timer.query_set, (panel_index * 2 + 1) as u32);
    }
    encoder.finish()
}

fn render_fullscreen_pass(
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::Texture,
    pipeline: &wgpu::RenderPipeline,
    bind_group: &wgpu::BindGroup,
    label: &'static str,
) {
    let target_view = target.create_view(&Default::default());
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &target_view,
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
    pass.set_bind_group(0, bind_group, &[]);
    pass.draw(0..3, 0..1);
}

fn draw_glass_panel(
    canvas: &mut Canvas<femtovg::renderer::WGPURenderer>,
    blurred_image: ImageId,
    panel: PanelRect,
    capture: CaptureRect,
) {
    let mut shape = Path::new();
    shape.rounded_rect(panel.x, panel.y, panel.width, panel.height, panel.radius);
    let blurred_paint =
        Paint::image(blurred_image, capture.x, capture.y, capture.width, capture.height, 0.0, 1.0)
            .with_anti_alias(false);
    canvas.fill_path(&shape, &blurred_paint);
    canvas.fill_path(&shape, &Paint::color(Color::rgba(30, 35, 47, 112)));
    canvas.stroke_path(&shape, &Paint::color(Color::rgba(231, 237, 255, 94)).with_line_width(2.0));
}

fn push_canvas_commands(
    canvas: &mut Canvas<femtovg::renderer::WGPURenderer>,
    scene_texture: &wgpu::Texture,
    width: u32,
    height: u32,
    command_buffers: &mut Vec<wgpu::CommandBuffer>,
) {
    let output = femtovg::renderer::WGPURenderOutput {
        view: scene_texture.create_view(&Default::default()),
        width,
        height,
        format: TARGET_FORMAT,
    };
    if let Some(command_buffer) = canvas.flush_to_output(output) {
        command_buffers.push(command_buffer);
    }
}

fn encode_output_copy(
    device: &wgpu::Device,
    scene_texture: &wgpu::Texture,
    output_texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> wgpu::CommandBuffer {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("backdrop blur spike output copy encoder"),
    });
    encoder.copy_texture_to_texture(
        scene_texture.as_image_copy(),
        output_texture.as_image_copy(),
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    encoder.finish()
}

fn percentile(values: &mut [f64], percentile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f64::total_cmp);
    let index = ((values.len() - 1) as f64 * percentile).round() as usize;
    values[index]
}

fn print_metrics(metrics: &Metrics, allocations_before: u64, allocations_after: u64) {
    let frame_count = metrics.synchronized_frame_ms.len();
    let missed_percent = if frame_count == 0 {
        0.0
    } else {
        metrics.missed_frames as f64 / frame_count as f64 * 100.0
    };
    let mut synchronized = metrics.synchronized_frame_ms.clone();
    let mut gpu = metrics.gpu_frame_ms.clone();
    println!(
        "backdrop-blur-spike-result frames={} missed={} missed_percent={:.3} sync_p50_ms={:.3} sync_p95_ms={:.3} sync_p99_ms={:.3} gpu_p95_ms={} texture_allocations_before={} texture_allocations_after={} stable_texture_allocations={}",
        frame_count,
        metrics.missed_frames,
        missed_percent,
        percentile(&mut synchronized.clone(), 0.50),
        percentile(&mut synchronized, 0.95),
        percentile(&mut metrics.synchronized_frame_ms.clone(), 0.99),
        if gpu.is_empty() {
            "unavailable".to_owned()
        } else {
            format!("{:.3}", percentile(&mut gpu, 0.95))
        },
        allocations_before,
        allocations_after,
        allocations_before == allocations_after,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_uniforms_match_wgsl_alignment() {
        assert_eq!(std::mem::size_of::<CaptureParams>(), 16);
        assert_eq!(std::mem::size_of::<BlurParams>(), 32);
        assert_eq!(std::mem::size_of::<ColorParams>(), 80);
    }

    #[test]
    fn color_filters_compose_in_declared_order() {
        let effect = ColorParams::brightness(0.5).then(ColorParams::contrast(2.0));
        assert_eq!(effect.rows[0][0], 1.0);
        assert_eq!(effect.rows[1][1], 1.0);
        assert_eq!(effect.rows[2][2], 1.0);
        assert_eq!(effect.offset[..3], [-0.5; 3]);

        let inverted = ColorParams::invert(1.0);
        assert_eq!(inverted.rows[0][0], -1.0);
        assert_eq!(inverted.offset[..3], [1.0; 3]);
    }

    #[test]
    fn capture_padding_stays_inside_the_scene() {
        let config = SpikeConfig {
            width: 2560,
            height: 1440,
            warmup: Duration::ZERO,
            sample: Duration::ZERO,
            target_frame: Duration::from_secs_f64(1.0 / 60.0),
            blur_radius: 18.0,
            scale_factor: 1.0,
            color_effects: true,
            showcase_layout: false,
            downsample: 2,
            panel_count: 4,
        };
        let capture = padded_capture_rect(
            PanelRect { x: 0.0, y: 0.0, width: 500.0, height: 300.0, radius: 32.0 },
            &config,
            18.0,
        );
        assert_eq!(capture.x, 0.0);
        assert_eq!(capture.y, 0.0);
        assert!(capture.width <= config.width as f32);
        assert!(capture.height <= config.height as f32);
    }

    #[test]
    fn benchmark_profiles_cover_four_and_six_panels() {
        let mut config = SpikeConfig {
            width: 2560,
            height: 1440,
            warmup: Duration::ZERO,
            sample: Duration::ZERO,
            target_frame: Duration::from_secs_f64(1.0 / 60.0),
            blur_radius: 18.0,
            scale_factor: 1.0,
            color_effects: true,
            showcase_layout: false,
            downsample: 2,
            panel_count: 4,
        };
        config.panel_count = 0;
        assert_eq!(config.panel_indices(), &NO_PANEL_INDICES);
        config.panel_count = 1;
        assert_eq!(config.panel_indices(), &ONE_PANEL_INDICES);
        config.panel_count = 4;
        assert_eq!(config.panel_indices(), &FOUR_PANEL_INDICES);
        config.panel_count = 6;
        assert_eq!(config.panel_indices(), &SIX_PANEL_INDICES);
    }

    #[test]
    fn pooled_targets_preserve_each_panel_aspect_ratio() {
        let config = SpikeConfig {
            width: 2560,
            height: 1440,
            warmup: Duration::ZERO,
            sample: Duration::ZERO,
            target_frame: Duration::from_secs_f64(1.0 / 60.0),
            blur_radius: 18.0,
            scale_factor: 1.0,
            color_effects: true,
            showcase_layout: false,
            downsample: 2,
            panel_count: 4,
        };
        let panels = animated_panels(&config, 0.0);
        let rail = panel_target_size(&config, panels[0], 2.0);
        let navigator = panel_target_size(&config, panels[5], 32.0);

        assert_ne!(rail, navigator);
        assert!(rail.0 > rail.1);
        assert!(navigator.1 > navigator.0);
    }

    #[test]
    fn six_panel_profile_uses_the_full_blur_ladder() {
        let config = SpikeConfig {
            width: 2560,
            height: 1440,
            warmup: Duration::ZERO,
            sample: Duration::ZERO,
            target_frame: Duration::from_secs_f64(1.0 / 60.0),
            blur_radius: 32.0,
            scale_factor: 1.0,
            color_effects: true,
            showcase_layout: false,
            downsample: 2,
            panel_count: 6,
        };
        let radii = (0..6).map(|index| panel_blur_radius(&config, index)).collect::<Vec<_>>();
        assert_eq!(radii, SIX_PANEL_BLUR_STEPS);
    }

    #[test]
    fn logical_blur_scales_to_physical_pixels() {
        let mut config = SpikeConfig {
            width: 1600,
            height: 900,
            warmup: Duration::ZERO,
            sample: Duration::ZERO,
            target_frame: Duration::from_secs_f64(1.0 / 60.0),
            blur_radius: 32.0,
            scale_factor: 1.0,
            color_effects: true,
            showcase_layout: false,
            downsample: 2,
            panel_count: 6,
        };
        assert_eq!(panel_blur_radius(&config, 5), 32.0);
        config.scale_factor = 1.5;
        assert_eq!(panel_blur_radius(&config, 5), 48.0);
        config.scale_factor = 2.0;
        assert_eq!(panel_blur_radius(&config, 5), 64.0);
    }

    #[test]
    fn final_panel_overlaps_and_samples_an_earlier_panel() {
        let config = SpikeConfig {
            width: 1600,
            height: 900,
            warmup: Duration::ZERO,
            sample: Duration::ZERO,
            target_frame: Duration::from_secs_f64(1.0 / 60.0),
            blur_radius: 32.0,
            scale_factor: 1.0,
            color_effects: true,
            showcase_layout: false,
            downsample: 2,
            panel_count: 6,
        };
        let panels = animated_panels(&config, 0.0);
        let earlier = panels[0];
        let final_panel = panels[5];
        let overlaps = earlier.x < final_panel.x + final_panel.width
            && earlier.x + earlier.width > final_panel.x
            && earlier.y < final_panel.y + final_panel.height
            && earlier.y + earlier.height > final_panel.y;
        assert!(overlaps);
        assert_eq!(config.panel_indices().last(), Some(&5));
    }
}
