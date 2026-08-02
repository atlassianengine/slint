// Rust guideline compliant 2026-02-21
//! Retrospect's native WGPU canvas underlay.
//!
//! The shader is a presentation layer only. `canvas-core` remains the camera
//! source of truth; this module receives the projected camera and pointer.

use std::{cell::RefCell, rc::Rc, time::Instant};

use slint::wgpu_29::wgpu;
use slint::{ComponentHandle, GraphicsAPI, Image, RenderingState};

use crate::AppWindow;
use canvas_core::CanvasCamera;

const WGSL: &str = r#"
struct Params {
    viewport: vec2<f32>, camera: vec2<f32>, pointer: vec4<f32>,
    zoom: f32, spacing: f32, dot_radius: f32, base_alpha: f32,
    grid_color: vec4<f32>, glow_color: vec4<f32>,
}
@group(0) @binding(0) var<uniform> params: Params;
struct VertexOut { @builtin(position) position: vec4<f32>, }
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    var positions = array<vec2<f32>, 3>(vec2(-1., -1.), vec2(3., -1.), vec2(-1., 3.));
    return VertexOut(vec4(positions[index], 0., 1.));
}
@fragment fn fs_main(@builtin(position) fragment: vec4<f32>) -> @location(0) vec4<f32> {
    let screen = fragment.xy;
    let pointer_active = params.pointer.x >= 0. && params.pointer.y >= 0.;
    let pointer_distance = distance(screen, params.pointer.xy);
    // Pointer velocity is smoothed on the Rust boundary. Keep the spatial
    // displacement modest so high-frequency mouse reports cannot make dots
    // jump between adjacent cells.
    let distortion = select(vec2(0.), params.pointer.zw * exp(-pointer_distance * 0.016) * 0.010, pointer_active);
    let local = abs(fract((screen - distortion - params.camera + vec2(params.spacing * 0.5)) / params.spacing) - vec2(0.5)) * params.spacing;
    let distance_to_dot = length(local);
    let aa = max(fwidth(distance_to_dot), 0.7);
    let dot = 1. - smoothstep(params.dot_radius - aa, params.dot_radius + aa, distance_to_dot);
    let glow = select(0., pow(max(0., 1. - pointer_distance / 260.), 2.55), pointer_active);
    let alpha = dot * min(0.46, params.base_alpha + glow * 0.34);
    let color = mix(params.grid_color.rgb, params.glow_color.rgb, glow);
    return vec4(color, alpha);
}
"#;

#[derive(Clone, Copy)]
struct BackdropState {
    width: u32,
    height: u32,
    camera_x: f32,
    camera_y: f32,
    zoom: f32,
    pointer_x: f32,
    pointer_y: f32,
    velocity_x: f32,
    velocity_y: f32,
    last_pointer_at: Option<Instant>,
    dirty: bool,
}

#[derive(Clone)]
pub struct CanvasBackdrop {
    state: Rc<RefCell<BackdropState>>,
}

impl CanvasBackdrop {
    pub fn sync_camera(&self, camera: CanvasCamera) {
        let mut state = self.state.borrow_mut();
        state.camera_x = camera.x;
        state.camera_y = camera.y;
        state.zoom = camera.zoom;
        state.dirty = true;
    }
}

impl Default for BackdropState {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            camera_x: 0.0,
            camera_y: 0.0,
            zoom: 1.0,
            pointer_x: -1.0,
            pointer_y: -1.0,
            velocity_x: 0.0,
            velocity_y: 0.0,
            last_pointer_at: None,
            dirty: true,
        }
    }
}

struct BackdropRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    grid_color: [f32; 4],
    glow_color: [f32; 4],
    texture: Option<wgpu::Texture>,
}

impl BackdropRenderer {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        grid_color: [f32; 4],
        glow_color: [f32; 4],
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("retrospect canvas backdrop shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(WGSL)),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("retrospect canvas backdrop uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("retrospect canvas backdrop pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("retrospect canvas backdrop pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba8UnormSrgb.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("retrospect canvas backdrop state"),
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            bind_group_layout,
            uniform,
            grid_color,
            glow_color,
            texture: None,
        }
    }

    fn render(&mut self, state: BackdropState) -> Option<wgpu::Texture> {
        if state.width == 0 || state.height == 0 {
            return None;
        }
        let recreate = match &self.texture {
            Some(texture) => {
                texture.size().width != state.width || texture.size().height != state.height
            }
            None => true,
        };
        if recreate {
            self.texture = Some(self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("retrospect procedural canvas backdrop"),
                size: wgpu::Extent3d {
                    width: state.width,
                    height: state.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }));
        }
        let spacing = grid_spacing(state.zoom);
        let params = [
            state.width as f32,
            state.height as f32,
            state.camera_x,
            state.camera_y,
            state.pointer_x,
            state.pointer_y,
            state.velocity_x,
            state.velocity_y,
            state.zoom,
            spacing,
            (spacing * 0.065).clamp(1.25, 1.85),
            (0.13 + spacing * 0.0008).clamp(0.14, 0.22),
            self.grid_color[0],
            self.grid_color[1],
            self.grid_color[2],
            self.grid_color[3],
            self.glow_color[0],
            self.glow_color[1],
            self.glow_color[2],
            self.glow_color[3],
        ];
        self.queue
            .write_buffer(&self.uniform, 0, &floats_as_bytes(&params));
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("retrospect canvas backdrop bind group"),
            layout: &self.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.uniform.as_entire_binding(),
            }],
        });
        let texture = self.texture.as_ref()?.clone();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("retrospect canvas backdrop encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("retrospect canvas backdrop pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture.create_view(&Default::default()),
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
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        Some(texture)
    }
}

pub fn install(app: &AppWindow) -> Result<CanvasBackdrop, slint::SetRenderingNotifierError> {
    let state = Rc::new(RefCell::new(BackdropState::default()));
    bind_ui_events(app, state.clone());
    let renderer = Rc::new(RefCell::new(None));
    let palette = app.global::<crate::WorkspacePalette>();
    let grid_color = color_components(palette.get_canvas_grid_dot());
    let glow_color = color_components(palette.get_canvas_grid_glow());
    let app_weak = app.as_weak();
    let render_state = state.clone();
    app.window()
        .set_rendering_notifier(move |phase, graphics_api| match phase {
            RenderingState::RenderingSetup => {
                if let GraphicsAPI::WGPU29 { device, queue, .. } = graphics_api {
                    *renderer.borrow_mut() =
                        Some(BackdropRenderer::new(device, queue, grid_color, glow_color));
                }
            }
            RenderingState::BeforeRendering => {
                let mut state = render_state.borrow_mut();
                if !state.dirty {
                    return;
                }
                state.dirty = false;
                let snapshot = *state;
                drop(state);
                let Some(texture) = renderer
                    .borrow_mut()
                    .as_mut()
                    .and_then(|renderer| renderer.render(snapshot))
                else {
                    return;
                };
                if let Some(app) = app_weak.upgrade() {
                    if let Ok(image) = Image::try_from(texture) {
                        app.set_canvas_backdrop(image);
                    }
                }
            }
            RenderingState::RenderingTeardown => *renderer.borrow_mut() = None,
            _ => {}
        })?;
    Ok(CanvasBackdrop { state })
}

fn bind_ui_events(app: &AppWindow, state: Rc<RefCell<BackdropState>>) {
    let bounds_state = state.clone();
    let bounds_app = app.as_weak();
    app.on_canvas_bounds_changed(move |width, height| {
        let mut state = bounds_state.borrow_mut();
        state.width = width.max(0.0) as u32;
        state.height = height.max(0.0) as u32;
        state.dirty = true;
        if let Some(app) = bounds_app.upgrade() {
            app.window().request_redraw();
        }
    });
    let pointer_app = app.as_weak();
    app.on_canvas_pointer_moved(move |x, y| {
        let now = Instant::now();
        let mut state = state.borrow_mut();
        if x >= 0.0 && y >= 0.0 {
            if let Some(previous) = state.last_pointer_at {
                let elapsed = now.duration_since(previous).as_secs_f32();
                if (0.004..=0.120).contains(&elapsed) {
                    let velocity_x = ((x - state.pointer_x) / elapsed).clamp(-900.0, 900.0);
                    let velocity_y = ((y - state.pointer_y) / elapsed).clamp(-900.0, 900.0);
                    // An exponential moving average removes mouse-report
                    // jitter while preserving the directional wake.
                    state.velocity_x = state.velocity_x * 0.78 + velocity_x * 0.22;
                    state.velocity_y = state.velocity_y * 0.78 + velocity_y * 0.22;
                }
            }
            state.pointer_x = x;
            state.pointer_y = y;
            state.last_pointer_at = Some(now);
        } else {
            state.pointer_x = -1.0;
            state.pointer_y = -1.0;
            state.velocity_x = 0.0;
            state.velocity_y = 0.0;
            state.last_pointer_at = None;
        }
        state.dirty = true;
        if let Some(app) = pointer_app.upgrade() {
            app.window().request_redraw();
        }
    });
}

fn color_components(color: slint::Color) -> [f32; 4] {
    [
        f32::from(color.red()) / 255.0,
        f32::from(color.green()) / 255.0,
        f32::from(color.blue()) / 255.0,
        f32::from(color.alpha()) / 255.0,
    ]
}

fn grid_spacing(zoom: f32) -> f32 {
    const STEPS: [f32; 16] = [
        4.0, 6.0, 8.0, 12.0, 16.0, 20.0, 24.0, 28.0, 32.0, 40.0, 48.0, 56.0, 64.0, 80.0, 96.0,
        128.0,
    ];
    let zoom = zoom.max(0.1);
    let ideal = 24.0 / zoom;
    STEPS
        .into_iter()
        .min_by(|a, b| (*a - ideal).abs().total_cmp(&(*b - ideal).abs()))
        .unwrap_or(24.0)
        * zoom
}

fn floats_as_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect()
}
