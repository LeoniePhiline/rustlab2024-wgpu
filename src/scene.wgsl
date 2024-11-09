// If you have semantic or syntactical errors in your shader, the application will crash on launch.
// Scroll past the panic's stack trace to see the actual errors.
// 
// These resources may help you with your tasks:
// - https://google.github.io/tour-of-wgsl/ (don't mind the "WebGPU is not supported in this browser")
// - https://webgpufundamentals.org/webgpu/lessons/webgpu-wgsl-function-reference.html

// We only have on bind group for our compute pipeline, so we use group 0.
// The scene's texture is the first entry of our bind group, at index 0.
// It is a storage texture with write-access.
// The texture format must match the format we defined in `Scene::new`.
@group(0) @binding(0)
var frame: texture_storage_2d<rgba8unorm, write>;

// This must match the order and entries of our `CameraUniform` in `scene.rs`.
struct Camera {
    origin: vec4<f32>,
    view_direction: vec4<f32>,
    up: vec4<f32>,
}
// The scene's camera is the second entry of our bind group, at index 1.
// It is a uniform buffer as we only need to read and have no large size requirements.
// An explanation of the difference between uniform and storage buffers can be found in
// `scene.rs`.
@group(0) @binding(1)
var<uniform> camera: Camera;

// How do we pick an ideal workgroup size? It can depend on how the data you work on is structured,
// but in general the ideal workgroup size is mostly dictated by the GPU architecture we run on.
//
// NVIDIA: https://developer.nvidia.com/blog/advanced-api-performance-intrinsics/
// => "Use GroupSize and WorkGroup as a multiplier of warp size (32 * N), 64 is usually a sweet spot"
//
// AMD: https://gpuopen.com/learn/rdna-performance-guide/
// => "GCN runs shader threads in groups of 64 known as wave64."
// => "RDNA runs shader threads in groups of 32 known as wave32."
//
// A workgroup size of 8 * 8 = 64 should be the most compatible.
@compute @workgroup_size(8, 8)
fn render(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = textureDimensions(frame);
    // if we're outside the frame, there's nothing to do
    if (gid.x >= size.x || gid.y >= size.y) {
        return;
    }

    let width = f32(size.x);
    let height = f32(size.y);
    let x = f32(gid.x);
    let y = f32(gid.y);


    let aspect_ratio = width / height;

    // Camera properties
    let origin = camera.origin.xyz;
    let view_direction = camera.view_direction.xyz;
    let up = camera.up.xyz;
    // Feel free to play around with this!
    // To learn more about the focal length, check out:
    // https://en.wikipedia.org/wiki/Focal_length#In_photography
    let focal_length = 1.0;

    // Our camera's viewport has its own coordinate system, which we define as a range
    // from -1.0 to 1.0 in Y direction.
    // The viewport range in X direction is then derived from the aspect ratio of our frame
    // so we can ensure the unit sizes in X direction match those in Y direction.
    let viewport_height = 2.0;
    let viewport_width = aspect_ratio * viewport_height;

    // Scene direction from left to right on our viewport
    let horizontal = normalize(cross(up, -view_direction));
    // Scene direction from bottom to top on our viewport
    let vertical = normalize(cross(view_direction, horizontal));

    // 1. Translate the screen pixel coordinates (x, y) to viewport coordinates (u, v).
    // Remember that (0, 0) is the center of our viewport.
    // To translate the coordinates, you first must compute the relative position of the
    // pixel on the screen (by dividing it by the frame's width / height).
    // Then, you multiply it with the viewport's width / height to get the relative position
    // on the viewport.
    // Because the viewport's coordinates are not relative to one of the edges but to its center,
    // you must also subtract half of the viewport's width / height.
    // let u = ...
    // let v = ...

    // 2. Now we can finally compute the direction of our ray. All rays begin at the camera's
    // origin and then go through the desired pixel's position on the viewport.
    // The focal length is the distance between our camera's origin and the viewport.
    // Our view direction always points at the center (0, 0) of the viewport.
    // Multipliying the focal length with the view direction gives us the viewport's center point
    // in coordinates relative to the camera's origin.
    // We can then offset this relative coordinate to point to the desired pixel on the viewport
    // instead.
    // To do this, add the viewport coordinate u multiplied by the viewport's right pointing
    // horizontal direction (to translate the offset into scene coordinates), and the viewport
    // coordinate v multiplied by the viewport's upwards pointing vertical direction.
    // let dir = ...

    // 3. Instead of calculating this static gradient as output color, call ray_color with the
    // camera's origin and the computed ray direction.
    let color = vec4<f32>(x / width, y / height, 1.0, 1.0);

    textureStore(frame, vec2<i32>(i32(gid.x), i32(gid.y)), color);
}

fn ray_color(origin: vec3<f32>, dir: vec3<f32>) -> vec4<f32> {
    let unit_dir = normalize(dir);
    let sphere_center = vec3<f32>(0.0, 0.0, -1.0);
    let sphere_radius = 0.5;

    var t = hit_sphere(sphere_center, sphere_radius, origin, unit_dir);
    // A negative t means there was no solution, i.e., the ray does not intersect the sphere.
    // A positive t is the distance from our camera to the hit point.

    // Hit
    if (t > 0.0) {
        let hit_point = origin + unit_dir * t;
        let sphere_coords = hit_point - sphere_center;

        // N is the normal vector of the hit point on our sphere.
        // Visualizing it as a color allows us to identify the different areas
        // of the sphere.
        let N = normalize(sphere_coords);
        return 0.5 * vec4<f32>(N.x + 1, N.y + 1, N.z + 1, 1.0);
    }

    // No hit
    // We use the unit vector of our ray to create some variance in the background color.
    let a = 0.5 * (unit_dir.y + 1.0);
    return ((1.0 - a) * vec4<f32>(1.0, 1.0, 1.0, 1.0) + a * vec4<f32>(0.5, 0.7, 1.0, 1.0));
}


fn hit_sphere(center: vec3<f32>, radius: f32, origin: vec3<f32>, dir: vec3<f32>) -> f32 {
    // 4. Determine if and where our ray (origin, dir) intersects with a sphere (center, radius).
    // Refer to:
    // - https://www.cs.uaf.edu/2012/spring/cs481/section/0/lecture/01_26_ray_intersections.html
    // - https://webgpufundamentals.org/webgpu/lessons/webgpu-wgsl-function-reference.html
    // Note that we are only interested in one solution:
    // the point where the ray enters the sphere (this is the one with the negative square root).
    // An unsolvable square root (negative discriminant) means we have no intersection.
    // In this case, return -1.0.

    return -1.0;
}
