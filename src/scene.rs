use cgmath::{Vector2, Vector3};
use wgpu::{
    util::DeviceExt, BindGroupEntry, BindGroupLayoutEntry, BindingResource, BindingType,
    BufferBindingType, BufferUsages, ShaderStages, StorageTextureAccess, TextureViewDimension,
};

use crate::{
    arcball::{ArcballCamera, CameraOperation},
    texture::Texture,
};

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    // Because of WebGPU's memory model, we must align our vectors to
    // four components eventhough they only have three:
    // https://www.w3.org/TR/WGSL/#alignment-and-size
    origin: [f32; 4],
    view_direction: [f32; 4],
    up: [f32; 4],
}

impl From<&ArcballCamera<f32>> for CameraUniform {
    fn from(camera: &ArcballCamera<f32>) -> Self {
        let eye_pos = camera.eye_pos();
        let eye_dir = camera.eye_dir();
        let up_dir = camera.up_dir();
        // We have to pass data for our shaders as raw continuous bytes,
        // which we achieve by converting our vectors into slices and letting
        // bytemuck handle the serialization.
        Self {
            origin: [eye_pos.x, eye_pos.y, eye_pos.z, 0.0],
            view_direction: [eye_dir.x, eye_dir.y, eye_dir.z, 0.0],
            up: [up_dir.x, up_dir.y, up_dir.z, 0.0],
        }
    }
}

pub struct Scene {
    camera_buffer: wgpu::Buffer,
    camera: ArcballCamera<f32>,
    pub texture: Texture,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
    pub prev_pointer_pos: Option<(f32, f32)>,
}

impl Scene {
    pub fn new(device: &wgpu::Device, center: Vector3<f32>, width: u32, height: u32) -> Self {
        // Creating a shader module for a compute shader works exactly like our vertex and fragment
        // shader module, as there is nothing specific to the shader type here.
        // Theoratically, we could even put all three shader types into one file, but for separation
        // of concerns, we split the shader responsible for displaying the scene from the shader
        // for rendering the scene.
        let shader_src = include_str!("scene.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        // Creating the texture is abstracted into the `texture.rs` module.
        // We create a texture covering the whole surface.
        // The color format is mostly a question of compatibility here, as the final color
        // format that gets presented to the screen is determined by our surface config.
        // We use RGBA with 8 bits per component as its the easiest to work with.
        // Also, we specify the texture to be a storage texture so we can write to it
        // from within our compute shader.
        let texture = Texture::new(
            device,
            (width, height),
            Some("scene texture"),
            wgpu::TextureFormat::Rgba8Unorm,
            true,
        );

        // The arcball camera mechanism has been defined by Ken Shoemake in 1992, you can find his paper here:
        // https://www.talisman.org/~erlkonig/misc/shoemake92-arcball.pdf
        //
        // It works similar to a snow dome or a globe that we observe from the outside.
        // We can rotate the ball to change what we see, but we can't change the direction in which we look.
        // Mathematically our implementation assumes the scene is stationary and instead changes the position
        // and direction of the camera, but this difference is simply a definition of the inertial system.
        // Feel free to look into `arcball.rs` to see the implementation.
        let mut camera = ArcballCamera::new(center, 1.0, [width as f32, height as f32]);
        camera.zoom(-1.0, 1.0);
        // As described in the implementation of `CameraUniform`, we must pass data to our shaders as byte buffers.
        // For this, we make use of bytemuck. Our `CameraUniform` struct derives the required bytemuck traits
        // and can then be turned into a byte slice through `bytemuck::cast_slice`.
        let camera_uniform = CameraUniform::from(&camera);

        // 1. Create a camera buffer on our device. As we already know the initial contents of this buffer,
        // you can use `Device::create_buffer_init` to pass the camera uniform data.
        // Remember to use bytemuck to turn our camera uniform into a byte slice.
        // As usage type, we require two: a uniform, to actually expose the buffer to our shader,
        // and a copy destination, so we can write to the buffer from within Rust to update the camera.
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                // 2. Similar to our render bind group layout in `Application::new`, we first bind the texture of our scene.
                // The texture must be visible to our compute shader stage.
                // Instead of type texture, we use type storage texture so we can write to it.
                // As access type, we specify write only as we currently do not need to read from the previous frame.
                // The format of our texture can be access through `texture.format` and it is two-dimensional.
                // We again specify no count as this is not an array of textures.
                // The binding index must match the index of `@binding(..)` in our shader.
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: texture.format,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
                // 3. This time, we also include a second item in our bind group: the camera uniform.
                // Again, this must be visible to the compute shader stage.
                // The type of our buffer is uniform, not storage.
                // Our buffer has no dynamic offset and we specify no minimum size for now.
                //
                // Uniform buffers are generally faster, but read-only and limitted in size.
                // Storage buffers can optionally be marked as writable.
                // The size limitation of a uniform buffer is at least 64 kilobytes, while a storage
                // buffer has a limit of at least 128 megabytes.
                // You can find out more about the differences on:
                // https://webgpufundamentals.org/webgpu/lessons/webgpu-storage-buffers.html
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("bind_group_layout"),
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[
                // 4. Bind our texture and camera uniform by specifying them as group entries here.
                // For our texture, we again wrap the view in an `wgpu::BindingResource` enum.
                // WebGPU buffers can be converted into a resource through their `as_entire_binding`
                // method.
                // Make sure to bind them to their correct index!
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&texture.view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: camera_buffer.as_entire_binding(),
                },
            ],
            label: Some("bind_group"),
        });

        // Creating a compute pipeline is much simpler than creating a render pipeline, most dynamic parts
        // of it are determined by our own code inside our compute shader.
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: None, // optional if there is only one @compute function
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            camera_buffer,
            camera,
            texture,
            bind_group_layout,
            bind_group,
            pipeline,
            prev_pointer_pos: None,
        }
    }

    pub fn render(&mut self, encoder: &mut wgpu::CommandEncoder) {
        // A compute pass works very similar to render pass, except that it takes a
        // compute pipeline instead of a render pipeline.
        // As with our render pass, we first assign the pipeline and the bind group to our pass
        // before letting it do the actual work.
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("cpass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &self.bind_group, &[]);

        // The resolution of our scene, which we use to determine the necessary
        // amount of workgroups to dispatch.
        let (width, height) = self.texture.dimensions;

        // A compute pass does not "draw". Instead, it dispatches workgroups to the GPU
        // to perform the work described by its compute pipeline (including shader exectution).
        // A workgroup is an independent unit that executes the code of our compute shader.
        // This is _somewhat_ similar to threads, in that separate workgroups can perform
        // different work while still running in parallel to other workgroups.
        // Unlike a thread, even inside a workgroup there are many execution running in parallel.
        // The limitation to this is that while the data the units inside a workgroup are working
        // on can differ, the instructions they execute do not.
        // For example, a loop with a differing amount of iterations or if-else-constructs with
        // differing branches to execute inside the same workgroup result in some of the workgroup's
        // unit to be blocked and not perform any work until the units that do have to perform this
        // work are finished.
        // WebGPU ensures that all workgroups of our compute pass are finished before continuing with
        // the render pass in `application.rs`.
        //
        // 5. Dispatch enough workgroups on our compute pass to cover the whole texture.
        // Our workgroup size is 8 * 8 (see why in `scene.wgsl`), we need at least as many workgroups in
        // X direction to cover our width and in Y direction to cover our height.
        // This could be accomplished by doing a floating point division by 8 and then
        // ceilung the result to ensure we have enough.
        // With integer division, a trick can be used instead: add (divisor - 1) to the dividend.
        // This way we ensure we are _at least_ 1 short of the next number dividable by 8.
        // As integer division is always floored, this trick gives us the desired ceiling.
        // In Z direction, we only want one workgroup.

        cpass.dispatch_workgroups((width + 7) / 8, (height + 7) / 8, 1);
    }

    pub fn resize_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    ) {
        // 6. As mentioned in `Application::resize`, we have to recreate a texture to resize it.
        // Recreate (and reassign) our `self.texture` here using the same parameters as in
        // `Scene::new`, but with the new width and height.
        // self.texture = ...

        // 7. Recreate `self.bind_group` just like in `Scene::new` so it uses the new texture.
        // self.bind_group = ...

        // Updating the size of the scene also affects our camera perspective.
        self.camera.update_screen(width as f32, height as f32);
        self.update_camera(queue);
    }

    pub fn update_camera(&mut self, queue: &wgpu::Queue) {
        let uniform = CameraUniform::from(&self.camera);

        // 8. As the buffer size stays the same when updating the camera, we don't have to create
        // a new buffer. Instead, we write the new data to the existing `self.camera_buffer`.
        // Writing to a buffer is a scheduled operation and only starts once the queue is submitted
        // to the GPU, so we have to enqueue a `write_buffer` on the queue.
        // As offset, use zero and make sure to bring the camera uniform into the required format using
        // bytemuck.
        //
        // Don't submit the queue yet, it will be submitted together with our compute and render pass.
        // queue.write...
    }

    pub fn reset_camera(&mut self, queue: &wgpu::Queue) {
        let center = self.camera.center;
        let (width, height) = self.texture.dimensions;
        self.camera = ArcballCamera::new(center, 1.0, [width as f32, height as f32]);
        self.camera.zoom(-1.0, 1.0);
        self.update_camera(queue);
    }

    pub fn on_zoom(&mut self, queue: &wgpu::Queue, delta: f32) {
        #[cfg(not(target_arch = "wasm32"))]
        self.camera.zoom(delta, 1.0 / 60.0);
        #[cfg(target_arch = "wasm32")]
        self.camera.zoom(delta / 10.0, 1.0 / 60.0);
        self.update_camera(queue);
    }

    pub fn on_pointer_moved(
        &mut self,
        queue: &wgpu::Queue,
        camera_op: CameraOperation,
        pos: (f32, f32),
    ) {
        if self.prev_pointer_pos.is_none() {
            self.prev_pointer_pos = Some(pos);
            return;
        }
        let prev = self.prev_pointer_pos.unwrap();
        match camera_op {
            CameraOperation::Rotate => {
                self.camera
                    .rotate(Vector2::new(prev.0, prev.1), Vector2::new(pos.0, pos.1));
                self.update_camera(queue);
            }
            CameraOperation::Pan => {
                self.camera
                    .pan(Vector2::new(pos.0 - prev.0, pos.1 - prev.1));
                self.update_camera(queue);
            }
            CameraOperation::None => {}
        }
        self.prev_pointer_pos = Some(pos);
    }
}
