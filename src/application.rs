// The tasks for this chapter are split into the different methods of Application.
// Go through the methods from top to bottom.
// Once all your methods are fully implemented, start your application and make sure
// it displays two white triangles.
// You can of course already try running your application inbetween to ensure no
// validation errors are raised.
// Afterwards, continue with adjusting your shaders in `application.wgsl`.
//
// Refer to https://docs.rs/wgpu/latest/wgpu/ to learn about a type's constructor,
// methods and attributes.
use std::{borrow::Cow, sync::Arc};

use color_eyre::{
    eyre::{Context, OptionExt},
    Result,
};
use wgpu::{
    Backends, BlendState, ColorWrites, CommandEncoderDescriptor, DeviceDescriptor, Features,
    FragmentState, Instance, InstanceDescriptor, InstanceFlags, MultisampleState,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PowerPreference, PrimitiveState,
    RenderBundleDescriptor, RenderPassDescriptor, RenderPipeline, RequestAdapterOptions,
    ShaderModuleDescriptor, VertexState,
};
use winit::{dpi::PhysicalSize, window::Window};

pub struct Application {
    surface_config: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_pipeline: RenderPipeline,
}

impl Application {
    pub async fn new(window: Arc<Window>, size: PhysicalSize<u32>) -> Result<Self> {
        // 1. We first must create a `wgpu::Instance`.
        // This is the entrypoint to all communication with wgpu.
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::PRIMARY,
            flags: InstanceFlags::advanced_debugging(),
            ..Default::default()
        });

        // 2. Next, we create our surface through the instance we created above.
        // For this, we must pass a window for the surface to target.
        // A surface is what anything we draw will be displayed on.
        let surface = instance.create_surface(window.clone())?;

        // 3. Once we have our surface, we request an adapter that is compatible with
        // this surface from our wgpu instance.
        // We want to request a high performance GPU so in case our device is a laptop
        // with two GPUs, we get the more powerful one.
        // Note that requesting an adapter is an asynchronous operation that must be awaited.
        // If no adapter matches our request options, we receive `None`.
        let adapter = instance
            .request_adapter(
                &(RequestAdapterOptions {
                    power_preference: PowerPreference::HighPerformance,
                    force_fallback_adapter: false,
                    compatible_surface: Some(&surface),
                }),
            )
            .await
            .ok_or_eyre("failed to get adapter")?;

        // 4. While an adapter represents a physical GPU, we also need a logical handle
        // to this GPU that enforces feature and memory limitations and is responsible for
        // executing any GPU commands we feed it.
        // This logical handle is called a "device" and can be requested from the adapter
        // we created above.
        // As we have no special requirements at this moment we just request the default
        // features and limits.
        // Requesting a device from an adapter returns a tuple containing both the device
        // and a queue to which we can submit GPU commands.
        // Note that requesting a device again is an asynchronous operation.
        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("GPU Device"),
                    required_features: Features::all_webgpu_mask(),
                    ..Default::default()
                },
                None,
            )
            .await?;

        // 5. Get the default config for our adapter from the surface, using the size
        // we got as parameter to our constructor. Make sure the size has a width and
        // height of at least 1, otherwise creating the surface may fail.
        // This only returns None if the surface and adapter are incompatible.
        // As we requested the adapter with `compatible_surface`, this is never the case.
        let surface_config = surface
            .get_default_config(&adapter, size.width.min(1), size.height.min(1))
            .expect("surface is compatible");

        // 6. Configure the surface using our logical device and the surface config.
        surface.configure(&device, &surface_config);

        // 7. Load the shader source code from `application.wgsl` and create a shader module
        // on our logical device to which we pass the loaded code as source.
        // As shader source type, we use WGSL.
        // You can optionally pass a label that will be used when reporting errors regarding
        // this particular shader module.
        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shader module"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("./application.wgsl"))),
        });

        // 8. Define the layout for our pipeline by creating a pipeline layout on our device.
        // Our layout is very basic for now, so it is sufficient to use the PipelineLayoutDescriptor's
        // default initializer.
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor::default());

        // 9. Next, create the render pipeline itself on the device.
        // This requires:
        // - layout: Our pipeline layout created above.
        // - vertex: A description of our pipeline's VertexState. This receives our shader module
        //   and optionally the name of the entry_point function inside that shader module
        //   As we only have one vertex shader in our code, this can be set to None for
        //   automatic detection.
        //   We don't need any buffer and no special compilation options.
        // - fragment: A description of our pipeline's FragmentState. This receives our shader module
        //   and optionally the name of the entry_point function inside that shader module
        //   As we only have one fragment shader in our code, this can be set to None for
        //   automatic detection.
        //   Also, we must define the color targets inside our fragment state.
        //   We only have one color target, which is defined by our surface_config's format,
        //   and should use a replacement blend (overwriting colors of the previous render)
        //   as well as write all color components our shaders return.
        //   We don't need any special compilation options.
        // - primitive: A description of our pipeline's PrimitiveState. This defines what
        //   kind of geometric primitive will be used in our render pipeline.
        //   We use the default primitive, a triangle list.
        // All other parameters may use their defaults.
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader_module,
                entry_point: None,
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: None,
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::default(),
                })],
            }),
            multiview: None,
            cache: None,
        });

        // Save these for later use
        Ok(Self {
            surface_config,
            surface,
            device,
            queue,
            render_pipeline,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        log::info!("Resize: {}x{}", width, height);

        // 1. Update our surface_config to the new dimensions.
        // Note that in rare scenarios, we may receive a width or height
        // of zero. Ensure the configured surface has a width and height
        // of at least one, otherwise we will run into validation issues.
        todo!();

        // 2. Reconfigure our surface using the updated surface_config
        todo!();
    }

    pub fn handle_event(
        &mut self,
        window: &winit::window::Window,
        winit_event: &winit::event::WindowEvent,
    ) -> bool {
        false
    }

    pub fn render(&mut self, window: &winit::window::Window) -> Result<(), wgpu::SurfaceError> {
        // Relevant wgpu types for this method:
        // - SurfaceTexture, Texture, TextureView
        // - CommandEncoder, CommandEncoderDescriptor
        // - RenderPass, RenderPassDescriptor
        // - RenderPassColorAttachment, Operations, LoadOp, StoreOp, Color

        // 1. To render something to the screen, we must first request the current
        // texture from our surface.
        let surface_texture = self.surface.get_current_texture()?;

        // 2. A texture itself cannot be used as render target.
        // We must create a view from this texture that then contains the metadata
        // our render pipeline needs to render to it.
        let texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Texture view"),
                format: Some(self.surface_config.format),
                ..Default::default()
            });

        // 3. All commands to be enqueued to our GPU's queue must first be encoded
        // so they are compatible with our logical device.
        // For this, we create a command encoder using our device.
        let command_encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Command encoder"),
            });

        // 4. Defining rendering commands for a GPU happens in form of a render pass.
        // We create a render pass by "beginning" it on the command encoder.
        // To actually get something out of the render pass, we give it a slice of
        // color attachments to render to (in our case, just one).
        // This color attachment receives the view we created for our surface texture earlier.
        // We then tell it what operations (ops) to perform on this view:
        // - On load, clear the surface texture using a black color
        // - On store, overwrite the contents of the surface texture (simply called "Store")
        let render_pass = command_encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Render pass"),
            color_attachments: todo!(),
            depth_stencil_attachment: todo!(),
            timestamp_writes: todo!(),
            occlusion_query_set: todo!(),
        });

        // 5. To let the render pass know of the structure of our pipeline, such as
        // shaders, or geometric primitives, set its pipeline to the render pipeline
        // we created in our constructor.
        todo!();

        // 6. Tell the render pass to draw six vertices (must be passed as a range 0 to 6)
        // for one instance (again, as a range 0 to 1).
        // Instancing will not be covered in this workshop.
        todo!();

        // 7. Before finishing our command encoder, we must drop the
        // render pass so it knows it is complete.
        todo!();

        // 8. Finish the command encoder, returning a command buffer.
        // Then, submit the command buffer to our GPU queue.
        todo!();

        // 9. Present the frame (our SurfaceTexture)
        todo!();

        Ok(())
    }
}
