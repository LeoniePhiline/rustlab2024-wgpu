# RustLab 2024 wgpu workshop

Chapters:

1. Basics of wgpu
2. Tracing rays (with compute shaders)
3. Bringing it to the web (with WebAssembly)

Checkout the respective branch for the chapter you are currently working on (ch1, ch2 or ch3).

## Chapter 2

Tasks:

1. Try running the application with `cargo run` to ensure your setup is working.
2. Implement methods in `src/application.rs`
    - `Application::new`
    - `Application::render`
    - Every method has an enumerated list of tasks required to finish the implementation.
    - Your application won't be executable just now, as we must first implement our `Scene` (you will get a validation error until then).
2. Implement these methods in `src/scene.rs`
    - `Scene::new`
    - `Scene::render`
    - Try running your application now, it should display a gradient covering the whole screen.
3. Implement your ray tracing shader in `src/scene.wgsl`: <<<< TODO: CONTINUE HERE
    - Start with the `render` function that is the entrypoint to our shader.
    - Read the implementation of `ray_color`. If anything here is unclear, raise your question to the group.
    - Implement the `hit_sphere` function next for computing the intersection of a ray and a sphere.
    - Try running your application now, it should display a colorful sphere in front of a lightblue gradient background.
4. When resizing the window, you will notice that it just stretches the scene. Implement `Application::resize` and `Scene::resize_texture` to fix this.
5. Our application comes with camera controls, but they are not functional yet. Implement `Scene::update_camera` to transfer any updates to our camera to the GPU.
6. Bonus: As I expect that everyone will require a different timeframe to reach this point, the rest of chapter 2 is considered a bonus exercise that you should attempt if you are quick enough. Try implementing reflections inside your scene. To do this, research anything that may be unclear on your own and continue as follows. Of course, you may also ask the instructor and it is encouraged to work together as a group to share your problems and knowledge.
    - Define multiple spheres (may be hardcoded as an array), give each sphere a unique color
    - Define an additional sphere as your light source.
    - Iterate over all spheres and test if your ray hits them. If multiple hits are registered, use the closest one (smallest `t`).
    - When your ray hits a sphere, compute the reflected vector. The formula for it is: `dir - 2 * dot(dir, N) * N`, where N is the hit points normal on the sphere (it is already used to compute the color in `ray_color` right now).
    - Loop until no more sphere is hit. For every new loop iteration, you use the previous hit point as ray origin and the reflected vector as direction. You cannot use recursion in shaders, so you must implement this iteratively! Limit the maximum iterations of your loop to a small number for now (5 or 10), to ensure you are not stuck in an infinite loop and that your GPU's resources are not exhausted.
    - Every hit adds a bit of color to the result. Start ouf with the color of the first hit. Then for every other hit, mix the previously accumulated color with the new sphere's color (add them together and divide the result by 2). When your ray has no more intersections and reaches the sky (our blue color), mix the sky's color with the result as well.
7. Bonus: If you complete reflection, you can try implementing semi-transparent spheres with refraction using [Snell's law](https://en.wikipedia.org/wiki/Snell%27s_law). For every hit, you get two new vectors: your reflected vector and your refracted vector. This is tricky to implement without recursion so only compute refraction for the first sphere you hit for now. Note that it's possible that your laptop's GPU is not powerful enough to compute this in realtime without further optimizations. If this is the case, come talk to me and to explain how you may render the frame to disk instead of displaying it on your screen.
