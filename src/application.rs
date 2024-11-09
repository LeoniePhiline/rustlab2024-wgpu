// The tasks for this chapter are split into the different methods of Application.
// Refer to https://docs.rs/wgpu/latest/wgpu/ to learn about a type's constructor,
// methods and attributes.
use std::sync::Arc;

use anyhow::{Context, Result};
use cgmath::{Vector3, Zero};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry,
    RenderPipeline, ShaderStages, TextureViewDimension,
};
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::{arcball::CameraOperation, scene::Scene};

pub struct Application {
    surface_config: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    scene: Scene,
    render_bind_group_layout: BindGroupLayout,
    render_bind_group: BindGroup,
    render_pipeline: RenderPipeline,
    mouse_down: bool,
}

impl Application {
    pub async fn new(window: Arc<Window>, size: PhysicalSize<u32>) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .context("no compatible adapter found")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::default(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await?;

        let surface_config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            // `get_default_config` only returns None if the surface and adapter are incompatible.
            // As we requested the adapter with `compatible_surface`, this is never the case.
            .unwrap();
        surface.configure(&device, &surface_config);

        // We encapsulate the actual ray tracing renderer into a separate module.
        // This application module now is only responsible for displaying frames rendered by the
        // ray tracer to the screen.
        let scene = Scene::new(&device, Vector3::zero(), size.width, size.height);

        // 1. To be able to display the ray tracer's rendered texture to the screen, we must make
        // our render pipeline know about it first.
        // This can be accomplished with bind groups.
        // Before we can create a bind group, we must describe its layout.
        //
        // Task: Create a bind group layout directly on our `device`.
        // - The bind group layout should have exactly one entry, binding to index 0.
        // - As we must read from the texture inside our fragment shader to be able to present it,
        // the bind group layout entry must have the fragment shader stage as visibility.
        // - The type of our entry is a texture.
        // - As a fragment shader returns the color with floating point components, we specify
        // a sample type of float.
        // - It doesn't have to be filterable as we map one pixel from the scene's texture to exactly
        // one pixel on the screen.
        // - Also, we don't want multisampling as our texture only has one layer.
        // - As we don't bind an array of textures but just a single texture, our count is `None`.
        let render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Bind group layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            });

        // 2. After creating the layout for our bind group, we can create the bind group itself.
        // This is again done using our `device`.
        // Specify the layout we created above.
        // As entries, we pass a slice with only one bind group entry, again using binding index 0.
        // Just like in the previous chapter, texture's are accessed through views.
        // Our abstraction in `texture.rs` already created the view for us, which we can access as
        // `scene.texture.view`.
        // To pass a texture view as resource of a bind group entry, it must be wrapped in the
        // `wgpu::BindingResource` enum.
        let render_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Bind group"),
            layout: &render_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&scene.texture.view),
            }],
        });

        let shader_src = include_str!("application.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("application shader"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        // 3. We must also include the layout of our bind group in our render pipeline layout
        // so the render pipeline knows what data to expect.
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render_pipeline_layout"),
                bind_group_layouts: &[&render_bind_group_layout],
                push_constant_ranges: &[],
                ..Default::default()
            });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render_pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: None, // optional if there is only one @vertex function
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: None, // optional if there is only one @fragment function
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Ok(Self {
            surface_config,
            surface,
            device,
            queue,
            scene,
            render_bind_group_layout,
            render_bind_group,
            render_pipeline,
            mouse_down: false,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        log::info!("Resize: {}x{}", width, height);
        self.surface_config.width = width.max(1);
        self.surface_config.height = height.max(1);
        self.surface.configure(&self.device, &self.surface_config);

        // When our window is resized, we now must not only resize the surface we display
        // our scene on but also the texture our ray tracer renders too.
        // The resize itself is abstracted into the scene module, which you will touch in a moment.
        self.scene
            .resize_texture(&self.device, &self.queue, width.max(1), height.max(1));

        // 4. A texture can not actually be resized, instead a new texture with the desired size
        // is created inside `Scene::resize_texture`.
        // This also means that the texture view we passed to our bind group before is not valid
        // anymore, it's still pointing to the old texture.
        // Recreate the bind group here (overwriting the current one in `self.render_bind_group`),
        // using the same arguments as in `Application::new` (`self.render_bind_group_layout` as
        // layout, `self.scene.texture.view` as texture view).
        self.render_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render_bind_group"),
            layout: &self.render_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&self.scene.texture.view),
            }],
        });
    }

    pub fn handle_event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        // We use mouse and keyboard inputs to control our camera.
        // Feel free to experiment with this if you have time!
        match event {
            // On left mouse button down, track mouse movement to control the camera.
            // On left mouse button up, stop controlling the camera.
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.scene.prev_pointer_pos = None;
                self.mouse_down = match state {
                    ElementState::Pressed => true,
                    ElementState::Released => false,
                };
                true
            }
            // Mouse movement rotates our arcball.
            WindowEvent::CursorMoved { position, .. } if self.mouse_down => {
                self.scene.on_pointer_moved(
                    &self.queue,
                    CameraOperation::Rotate,
                    (position.x as f32, position.y as f32),
                );
                true
            }
            // The mouse wheel controls the camera zoom.
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_horz, vert) => *vert,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };
                self.scene.on_zoom(&self.queue, delta);
                true
            }
            // When the space bar is pressed, reset the camera.
            WindowEvent::KeyboardInput { event, .. }
                if event.physical_key == PhysicalKey::Code(KeyCode::Space) =>
            {
                self.scene.reset_camera(&self.queue);
                true
            }
            _ => false,
        }
    }

    pub fn render(&mut self, window: &Window) -> Result<(), wgpu::SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = &frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });

        // A wgpu debug group helps us identify in which part of our application
        // errors occured, and can also be used to identify parts of our rendering
        // process in a graphical debugger such as RenderDoc or Xcode.
        // See https://github.com/gfx-rs/wgpu/wiki/Debugging-wgpu-Applications
        // for more information.
        encoder.push_debug_group("render scene");
        self.scene.render(&mut encoder);
        encoder.pop_debug_group();

        encoder.push_debug_group("display");
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("rpass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        rpass.set_pipeline(&self.render_pipeline);

        // 5. Our render pipline already knows about the required bindings thanks to
        // the bind group layout we included in the render pipeline layout.
        // But as you may have noticed, we haven't actually used the render bind group
        // itself anywhere yet.
        // As a bind group contains data used for rendering, and a render pass
        // requires all rendering specific data, we must set the render passes
        // bind group to our render bind group first before performing the draw call.
        // Set the bind group to index 0, as that is the index we specified in our bind
        // group layout, without any offsets (empty slice).
        rpass.set_bind_group(0, &self.render_bind_group, &[]);

        rpass.draw(0..6, 0..1);
        drop(rpass);
        encoder.pop_debug_group();

        self.queue.submit(Some(encoder.finish()));
        frame.present();

        Ok(())
    }
}
