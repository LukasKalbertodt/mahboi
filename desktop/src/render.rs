#![allow(unused_imports)] // TODO
use std::{
    iter,
    time::{Duration, Instant},
    sync::{
        Arc,
        atomic::Ordering,
    },
};

use failure::{bail, Error, ResultExt};
use spin_sleep::LoopHelper;
use vulkano::{
    format::Format,
    image::swapchain::SwapchainImage,
    instance::{Instance, PhysicalDevice},
    device::{Device, DeviceExtensions, Queue},
    swapchain::{ColorSpace, PresentMode, Surface, SurfaceTransform, Swapchain},
};
use vulkano_win::VkSurfaceBuild;
use winit::{
    EventsLoop, WindowBuilder, Window,
    dpi::LogicalSize,
};

use mahboi::{
    SCREEN_WIDTH, SCREEN_HEIGHT,
    log::*,
};
use crate::{
    DurationExt, Shared, RenderTiming, WINDOW_TITLE, TARGET_FPS,
    args::{Args, VulkanDevice},
};


/// A Vulkan "context". That word has no real meaning with Vulkan (unlike
/// OpenGL), but we use it for the collection of all "central" Vulkan objects.
pub(crate) struct VulkanContext {
    surface: Arc<Surface<Window>>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    swapchain: Arc<Swapchain<Window>>,
    swapchain_images: Vec<Arc<SwapchainImage<Window>>>,
}


/// Creates a Vulkan context given the values in `args` and `window_size`.
pub(crate) fn create_context(
    args: &Args,
    events_loop: &EventsLoop,
    window_size: &LogicalSize,
) -> Result<VulkanContext, Error> {
    // Build instance
    let instance = {
        let extensions = vulkano_win::required_extensions();
        Instance::new(None, &extensions, None)
            .context("could not create Vulkan instance")?
    };
    debug!("Built Vulkan instance. Loaded extensions: {:#?}", instance.loaded_extensions());


    // Build window with surface
    let surface = WindowBuilder::new()
        .with_dimensions(*window_size)
        .with_resizable(true)
        .with_title(WINDOW_TITLE)
        .build_vk_surface(events_loop, instance.clone())?;
    let window = surface.window();
    debug!("Created window with Vulkan surface");


    // Choosing the physical device
    let physical = {
        let physicals = PhysicalDevice::enumerate(&instance).filter(|_| {
            // TODO: check for capabilities
            true
        }).collect::<Vec<_>>();

        // Print devices if requested
        if args.list_devices {
            println!("Physical Vulkan devices (`--list-devices`):");
            for (i, physical) in physicals.iter().enumerate() {
                println!(
                    "  [{}] {} (type: {:?}, uuid: {})",
                    i,
                    physical.name(),
                    physical.ty(),
                    hex::encode(physical.uuid()),
                );
            }
        }

        let chosen = match args.device {
            Some(VulkanDevice::Index(idx)) => {
                *physicals.get(idx as usize)
                    .ok_or(failure::err_msg("invalid device index given via '--device'"))?
            }
            Some(VulkanDevice::Uuid(uuid)) => {
                *physicals.iter()
                    .find(|d| d.uuid() == &uuid)
                    .ok_or(failure::err_msg("invalid device index given via '--device'"))?
            }
            // Just take the first device
            None => physicals[0],
        };

        debug!("Using physical Vulkan device \"{}\" ({:?})", chosen.name(), chosen.ty());
        chosen
    };


    // Selecting a queue family that supports drawing to our window.
    // TODO: we might want to use an additional transfer queue in parallel. Or
    //       maybe not.
    let queue_family = physical.queue_families()
        .find(|&q| q.supports_graphics() && surface.is_supported(q).unwrap_or(false))
        .ok_or(failure::err_msg("no Vulkan queue family available that supports drawing \
            to the created window"))?;


    // Create Vulkan device and main queue
    let device_ext = DeviceExtensions { khr_swapchain: true, .. DeviceExtensions::none() };
    let (device, mut queues) = Device::new(
        physical,
        physical.supported_features(),
        &device_ext,
        iter::once((queue_family, 0.5)),
    ).context("could not create Vulkan device")?;
    let queue = queues.next().unwrap();
    debug!("Created Vulkan device. Loaded extensions: {:?}", device.loaded_extensions());
    trace!("Enabled device features: {:#?}", device.enabled_features());


    // Create swapchain
    let (swapchain, swapchain_images) = {
        let caps = surface.capabilities(physical)?;
        trace!("Surface capabilities: {:#?}", caps);

        // We basically only use it as color attachment, but it doesn't hurt to
        // just take everything it supports. TODO: or should we just take the
        // minimum we need?
        let usage = caps.supported_usage_flags;

        // We do not care about the alpha mode because our fragment buffer will
        // always output 1.0 as alpha.
        let alpha = caps.supported_composite_alpha.iter()
            .next()
            .ok_or(failure::err_msg("window surface does not support any alpha mode"))?;

        // Choosing the format of swapchain images. TODO: this should probably
        // be smarter.
        if !caps.supported_formats.contains(&(Format::B8G8R8A8Unorm, ColorSpace::SrgbNonLinear)) {
            println!("{:#?}", caps.supported_formats);
            bail!("surface does not support image format `B8G8R8A8Unorm` in sRGB");
        }
        let format = Format::B8G8R8A8Unorm;

        // Get window dimensions
        let dimensions: (u32, u32) = window.get_inner_size()
            .ok_or(failure::err_msg("window unexpectedly closed"))?
            .to_physical(window.get_hidpi_factor())
            .into();
        let initial_dimensions = [dimensions.0, dimensions.1];

        // Decide for present mode
        let present_mode = if let Some(user_choice) = args.present_mode {
            user_choice
        } else {
            if caps.present_modes.mailbox {
                PresentMode::Mailbox
            } else {
                PresentMode::Fifo
            }
        };
        debug!("Using present mode {:?}", present_mode);

        // Finally create swapchain
        Swapchain::new(
            device.clone(),
            surface.clone(),
            caps.min_image_count,
            format,
            initial_dimensions,
            1, // number of layers
            usage,
            &queue,
            SurfaceTransform::Identity,
            alpha,
            present_mode,
            true, // clipped: our shaders do not have side effects, so allow
            None, // old swapchain
        )?
    };
    debug!("Created Vulkan swapchain ({} images)", swapchain.num_images());


    Ok(VulkanContext { surface, device, queue, swapchain, swapchain_images })
}

/// Renders the front buffer of `gb_buffer` to the host screen at the host
/// refresh rate.
pub(crate) fn render_thread(
    context: VulkanContext,
    shared: &Shared,
) -> Result<(), Error> {
//     // Create the pixel buffer and initialize all pixels with black.
//     let pixel_buffer = PixelBuffer::new_empty(&display, SCREEN_WIDTH * SCREEN_HEIGHT);
//     pixel_buffer.write(&vec![(0u8, 0, 0); SCREEN_WIDTH * SCREEN_HEIGHT]);

//     // Create an empty, uninitialized texture
//     let texture = UnsignedTexture2d::empty_with_format(
//         &display,
//         UncompressedUintFormat::U8U8U8,
//         MipmapsOption::NoMipmap,
//         SCREEN_WIDTH as u32,
//         SCREEN_HEIGHT as u32,
//     )?;


//     #[derive(Copy, Clone)]
//     struct Vertex {
//         position: [f32; 2],
//         tex_coords: [f32; 2],
//     }

//     implement_vertex!(Vertex, position, tex_coords);

//     // Create the full screen quad
//     let shape = vec![
//         Vertex { position: [-1.0, -1.0], tex_coords: [0.0, 1.0] },
//         Vertex { position: [-1.0,  1.0], tex_coords: [0.0, 0.0] },
//         Vertex { position: [ 1.0, -1.0], tex_coords: [1.0, 1.0] },
//         Vertex { position: [ 1.0,  1.0], tex_coords: [1.0, 0.0] },
//     ];

//     let vertex_buffer = VertexBuffer::new(&display, &shape)?;
//     let indices = NoIndices(glium::index::PrimitiveType::TriangleStrip);


//     // Compile program. We have to do it via `ProgramCreationInput` to set
//     // `outputs_srgb` to `true`. This is an ugly workaround for a bug
//     // somewhere in the window creation stack. The framebuffer is
//     // incorrectly created as sRGB and glium then automatically converts
//     // all values returned by the fragment shader into sRGB. We don't want
//     // a conversion, so we just tell glium we already output sRGB (which we
//     // don't).
//     let program = Program::new(
//         &display,
//         ProgramCreationInput::SourceCode {
//             vertex_shader: include_str!("shader/simple.vert"),
//             tessellation_control_shader: None,
//             tessellation_evaluation_shader: None,
//             geometry_shader: None,
//             fragment_shader: include_str!("shader/simple.frag"),
//             transform_feedback_varyings: None,
//             outputs_srgb: true,
//             uses_point_size: false,
//         }
//     )?;

    let mut loop_helper = LoopHelper::builder()
        .report_interval_s(0.5)
        .build_without_target_rate();

//     // We want to delay drawing the buffer with OpenGL to reduce input lag. It
//     // is difficult to figure out how long we should wait with drawing, though!
//     // Visualizing frame timing:
//     //
//     //  V-Blank                        V-Blank                        V-Blank
//     //     |                              |                              |
//     //      [     sleep    ][draw][margin] [     sleep    ][draw][margin]
//     //
//     // We do this by trying to sync OpenGL to the CPU after issuing the last
//     // draw command. Then we measure the time from the buffer swap command
//     // until we read a pixel from the front buffer. This should be
//     // approximately the time OpenGL waited for V-Blank to happen. In theory,
//     // that's exactly the time we could sleep before drawing. However, drawing
//     // time is not always the same and can vary from frame to frame. Also,
//     // swapping the buffer still takes some time, even if V-Blank is right
//     // around the corner. That's why we insert a 'margin' that we want OpenGL
//     // to block waiting for V-Blank. Otherwise, we would often drop a frame.
//     //
//     // The draw delay starts at 0, but is continiously changed further down.
//     let mut draw_delay = Duration::from_millis(0);

//     // TODO: do not hardcode, but get from system
//     let frame_time = Duration::from_micros(16_667);

    loop {
        loop_helper.loop_start();

        // Check if the application is shutting down.
        if shared.should_quit.load(Ordering::SeqCst) {
            break;
        }

//         // We sleep before doing anything with OpenGL.
//         trace!("sleeping {:.2?} before drawing", draw_delay);
//         spin_sleep::sleep(draw_delay);

//         *shared.render_timing.lock().unwrap() = RenderTiming {
//             next_draw_start: Instant::now() + frame_time,
//             frame_time,
//         };

//         // We map the pixel buffer and write directly to it.
        let frame_birth_time = {
            let frame = shared.gb_frame.lock()
                .expect("failed to lock front buffer");
//             pixel_buffer.write(&*frame.buffer);
            frame.timestamp
        };

//         // We update the texture data by uploading our pixel buffer.
//         texture.main_level().raw_upload_from_pixel_buffer(
//             pixel_buffer.as_slice(),
//             0..SCREEN_WIDTH as u32,
//             0..SCREEN_HEIGHT as u32,
//             0..1,
//         );

//         // We need to find out the current physical window size to know how to
//         // stretch the texture.
//         let dpi_factor = *shared.window_dpi_factor.lock().unwrap();
//         let logical_size = *shared.window_size.lock().unwrap();
//         let physical_size = logical_size.to_physical(dpi_factor);
//         let scale_x = physical_size.width / SCREEN_WIDTH as f64;
//         let scale_y = physical_size.height / SCREEN_HEIGHT as f64;
//         let scale = if scale_x > scale_y { scale_y } else { scale_x };
//         let scale_factor = [(scale_x / scale) as f32, (scale_y / scale) as f32];


//         // Draw the fullscreenquad to the framebuffer
//         let mut target = display.draw();
//         target.clear_color_srgb(0.0, 0.0, 0.0, 0.0);

//         let uniforms = uniform! {
//             scale_factor: scale_factor,
//             tex: &texture,
//         };
//         target.draw(
//             &vertex_buffer,
//             &indices,
//             &program,
//             &uniforms,
//             &Default::default(),
//         )?;

//         // We do our best to sync OpenGL to the CPU here. We issue a fence into
//         // the command stream and then even call `glFinish()`. To really force
//         // the driver to sync here, we could read from the back buffer, I
//         // assume. But so far, it works fine.
//         glium::SyncFence::new(&display).unwrap().wait();
//         display.finish();
//         let after_draw = Instant::now();

//         // We swap buffers to present the finished framebuffer.
//         //
//         // But there is a little problem. We want OpenGL to wait now until
//         // V-Blank (on the host system) has happened, i.e. we want this
//         // function to block. But even with vsync enabled, it often doesn't
//         // (depending on the driver). We could also call `glFinish` which
//         // promises to block until all OpenGL operations are done, but this
//         // function is incorrectly implemented in many drivers, too! Usually,
//         // `glFinish` is a bad idea beause it hurts rendering performance. But
//         // we don't care about this, we mostly care about latency. So we really
//         // want to block here.
//         //
//         // The most reliable way to do that is to read from the front buffer.
//         // That forces OpenGL to wait until that every operation that was
//         // submitted before this read has completed. We only read a single
//         // pixel and do not use that value, but this forces synchronization. We
//         // need to use raw OpenGL here, because glium does not offer the
//         // ability to read a single pixel from the front buffer.
//         target.finish()?;
//         let pixel = unsafe {
//             display.exec_in_context(|| {
//                 // Get the currently bound `READ_BUFFER`
//                 let mut read_buffer_before: gl::types::GLint = 0;
//                 gl::GetIntegerv(gl::READ_BUFFER, &mut read_buffer_before);

//                 // Bind the front buffer and read one pixel from it
//                 gl::ReadBuffer(gl::FRONT);
//                 let mut pixel = [0u8; 4];
//                 let out_ptr = &mut pixel as *mut _ as *mut std::ffi::c_void;
//                 gl::ReadPixels(0, 0, 1, 1, gl::RGBA, gl::UNSIGNED_BYTE, out_ptr);

//                 // Bind the old buffer again (glium requires us to)
//                 gl::ReadBuffer(read_buffer_before as gl::types::GLenum);

//                 // There shouldn't be an error, but let's make sure.
//                 let e = gl::GetError();
//                 if e != 0 {
//                     bail!("unexpected OpenGL error {}", e);
//                 }

//                 Ok(pixel)
//             })?
//         };
        let after_finish = Instant::now();
        let emu_to_display_delay = after_finish - frame_birth_time;
//         trace!(
//             "swapped buffers, delay {:.2?}, pixel at (0, 0) -> {:?}",
//             emu_to_display_delay,
//             pixel,
//         );

//         // Calculate new draw delay.
//         draw_delay = {
//             // How long OpenGL waited for V-Blank.
//             let vblank_wait = after_finish - after_draw;

//             // The theoretical new duration we could sleep.
//             let new_value = draw_delay + vblank_wait;

//             // Subtract the sleep margin from the theoretical value. That is to
//             // avoid frame drops and account for draw time fluctuations.
//             let new_value = new_value.saturating_sub(shared.args.host_block_margin);

//             // Combine new value with the old one, depending on the learning
//             // rate.
//             let learn_rate = shared.args.host_delay_learn_rate as f64;
//             let new_delay = (1.0 - learn_rate) * draw_delay.as_nanos() as f64
//                 + learn_rate * new_value.as_nanos() as f64;
//             Duration::from_nanos(new_delay as u64)
//         };

        // Potentially update the window title to show the current speed.
        if let Some(ogl_fps) = loop_helper.report_rate() {
            let emu_fps = *shared.emulation_rate.lock().unwrap();
            let emu_percent = (emu_fps / TARGET_FPS) * 100.0;
            let title = format!(
                "{} (emulator: {:.1} FPS / {:3}%, OpenGL: {:.1} FPS, delay: {:.1?})",
                WINDOW_TITLE,
                emu_fps,
                emu_percent.round(),
                ogl_fps,
                emu_to_display_delay,
            );
            // display.gl_window().window().set_title(&title);
        }
    }

    Ok(())
}
