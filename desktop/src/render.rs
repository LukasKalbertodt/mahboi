#![allow(unused_imports)] // TODO
use std::{
    iter,
    time::{Duration, Instant},
    sync::{
        Arc, Condvar,
        atomic::Ordering,
    },
};

use failure::{bail, format_err, Error, ResultExt};
use spin_sleep::LoopHelper;
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, ImmutableBuffer},
    command_buffer::{AutoCommandBufferBuilder, DynamicState},
    descriptor::descriptor_set::PersistentDescriptorSet,
    device::{Device, DeviceExtensions, Queue},
    format::{self, Format},
    framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract, Subpass},
    image::{Dimensions, ImageUsage, StorageImage, SwapchainImage},
    instance::{Instance, PhysicalDevice},
    pipeline::{
        GraphicsPipeline,
        viewport::Viewport,
    },
    sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode},
    swapchain::{
        self, AcquireError, ColorSpace, PresentMode, Surface, SurfaceTransform,
        Swapchain, SwapchainCreationError,
    },
    sync::{FlushError, GpuFuture},
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
#[derive(Clone)]
pub(crate) struct VulkanContext {
    surface: Arc<Surface<Window>>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    swapchain: Arc<Swapchain<Window>>,
    swapchain_images: Vec<Arc<SwapchainImage<Window>>>,
}

impl VulkanContext {
    pub(crate) fn window(&self) -> &Window {
        self.surface.window()
    }
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

        // Choosing the format of swapchain images. There is only a very small
        // number of devices that do not support either of those formats,
        // according to https://vulkan.gpuinfo.org/listsurfaceformats.php
        let acceptable_formats = [
            (Format::B8G8R8A8Unorm, ColorSpace::SrgbNonLinear),
            (Format::R8G8B8A8Unorm, ColorSpace::SrgbNonLinear),
        ];
        let format = acceptable_formats.iter()
            .find(|&pref| caps.supported_formats.contains(pref))
            .ok_or(format_err!(
                "surface does not support any formats acceptable for Mahboi \
                    (acceptable: {:?}, supported: {:?})",
                acceptable_formats,
                caps.supported_formats,
            ))?;
        debug!("Using format {:?}", format);

        // Get window dimensions
        let initial_dimensions = inner_size(&window)?;

        // Decide for present mode
        let present_mode = if let Some(user_choice) = args.present_mode {
            user_choice
        } else {
            if caps.present_modes.mailbox {
                PresentMode::Mailbox
            } else {
                warn!(
                    "Present mode 'mailbox' is not available. Falling back to 'fifo'. \
                        Input lag could be increased due to this."
                );
                PresentMode::Fifo
            }
        };
        debug!("Using present mode {:?}", present_mode);

        // Finally create swapchain
        Swapchain::new(
            device.clone(),
            surface.clone(),
            caps.min_image_count,
            format.0,
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
    VulkanContext { surface, device, queue, mut swapchain, swapchain_images }: VulkanContext,
    shared: &Shared,
) -> Result<(), Error> {
    #[derive(Copy, Clone, Default)]
    struct Vertex {
        position: [f32; 2],
        tex_coords: [f32; 2],
    }
    vulkano::impl_vertex!(Vertex, position, tex_coords);

    // Create vertex buffer with a full screen quad
    let (vertex_buffer, vertex_buffer_init_future) = {

        let data = [
            Vertex { position: [-1.0, -1.0], tex_coords: [0.0, 0.0] },
            Vertex { position: [-1.0,  1.0], tex_coords: [0.0, 1.0] },
            Vertex { position: [ 1.0, -1.0], tex_coords: [1.0, 0.0] },
            Vertex { position: [ 1.0,  1.0], tex_coords: [1.0, 1.0] },
        ];

        ImmutableBuffer::from_iter(
            data.iter().cloned(),
            BufferUsage::vertex_buffer(),
            queue.clone(),
        )?
    };


    // Load shaders
    mod fs {
        vulkano_shaders::shader!{
            ty: "fragment",
            path: "src/shader/simple.frag"
        }
    }

    mod vs {
        vulkano_shaders::shader!{
            ty: "vertex",
            path: "src/shader/simple.vert"
        }
    }

    let vs = vs::Shader::load(device.clone())?;
    let fs = fs::Shader::load(device.clone())?;


    // Create renderpass
    let render_pass = vulkano::single_pass_renderpass!(
        device.clone(),
        attachments: {
            color: {
                load: Clear,
                store: Store,
                format: swapchain.format(),
                samples: 1,
            }
        },
        pass: {
            color: [color],
            depth_stencil: {}
        }
    )?;
    let render_pass = Arc::new(render_pass);

    // Create Pipeline
    let pipeline = GraphicsPipeline::start()
        .vertex_input_single_buffer::<Vertex>()
        .vertex_shader(vs.main_entry_point(), ())
        .triangle_strip()
        .viewports_dynamic_scissors_irrelevant(1)
        .fragment_shader(fs.main_entry_point(), ())
        .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
        .build(device.clone())?;
    let pipeline = Arc::new(pipeline);

    let mut dynamic_state = DynamicState { line_width: None, viewports: None, scissors: None };
    let mut framebuffers = create_framebuffers(
        &swapchain_images,
        render_pass.clone(),
        &mut dynamic_state,
    )?;

    let mut recreate_swapchain = false;

    // Create a buffer that holds the gameboy screen. This buffer will be
    // written by the CPU side. And on the GPU we will transfer data from this
    // buffer into the image created below.
    let screen_buffer = CpuAccessibleBuffer::from_iter(
        device.clone(),
        BufferUsage {
            transfer_source: true,
            .. BufferUsage::none()
        },
        vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4].into_iter(),
    )?;

    // Create an image that is used as texture on the fullscreen quad. It will
    // be filled with the buffer above.
    let tex = StorageImage::with_usage(
        device.clone(),
        Dimensions::Dim2d { width: SCREEN_WIDTH as u32, height: SCREEN_HEIGHT as u32 },
        format::R8G8B8A8Uint, // TODO: check if supported?
        ImageUsage {
            transfer_destination: true,
            sampled: true,
            .. ImageUsage::none()
        },
        Some(queue.family()),
    )?;

    // Sampler to sample the texture in the shader
    let sampler = Sampler::new(
        device.clone(),
        Filter::Nearest,
        Filter::Nearest,
        MipmapMode::Nearest,
        SamplerAddressMode::Repeat,
        SamplerAddressMode::Repeat,
        SamplerAddressMode::Repeat,
        0.0, // mip_lod_bias
        1.0, // max_anisotropy
        0.0, // min_lod
        0.0, // max_lod
    )?;

    let descriptor_set = PersistentDescriptorSet::start(pipeline.clone(), 0)
        .add_sampled_image(tex.clone(), sampler.clone())?
        .build()?;
    let descriptor_set = Arc::new(descriptor_set);

    // Before we can start rendering, we have to wait until the vertex buffer
    // was completely initialized.
    drop(vertex_buffer_init_future);

    let mut loop_helper = LoopHelper::builder()
        .report_interval_s(0.5)
        .build_without_target_rate();

    let present_mode = swapchain.present_mode();
    let immediate_present = present_mode == PresentMode::Immediate
        || present_mode == PresentMode::Mailbox;

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

        if recreate_swapchain {
            let dimensions = inner_size(surface.window())?;

            let (new_swapchain, new_images) = match swapchain.recreate_with_dimension(dimensions) {
                Ok(r) => r,
                Err(SwapchainCreationError::UnsupportedDimensions) => {
                    // TODO: handle this in a better way
                    warn!("Could not create swapchain with dimension {:?}", dimensions);
                    continue;
                }
                Err(err) => panic!("{:?}", err)
            };

            swapchain = new_swapchain;
            framebuffers = create_framebuffers(
                &new_images,
                render_pass.clone(),
                &mut dynamic_state,
            )?;

            recreate_swapchain = false;
        }

//         // We sleep before doing anything with OpenGL.
//         trace!("sleeping {:.2?} before drawing", draw_delay);
//         spin_sleep::sleep(draw_delay);

//         *shared.render_timing.lock().unwrap() = RenderTiming {
//             next_draw_start: Instant::now() + frame_time,
//             frame_time,
//         };


        // We map the Vulkan buffer and write directly to it.
        let frame_birth_time = {
            let mut frame = shared.gb_frame.lock()
                .expect("failed to lock front buffer");

            // If the swapchain swap is immediate, we will limit this thread by
            // waiting until a new frame was rendered by the emulator thread.
            if immediate_present {
                while !shared.should_quit.load(Ordering::SeqCst) && frame.num_finished == 0 {
                    frame = shared.frame_finished_event.wait(frame)
                        .expect("frame mutex got poisioned");
                }
            }

            // Write GB screen to Vulkan buffer
            let mut write = screen_buffer.write()?;
            for (chunk, pixels) in write.chunks_mut(4).zip(&frame.buffer) {
                chunk[0] = pixels.0;
                chunk[1] = pixels.1;
                chunk[2] = pixels.2;
            }

            // Check for droppped frames
            if frame.num_finished > 1 {
                shared.dropped_frames.fetch_add(frame.num_finished - 1, Ordering::SeqCst);
            }
            frame.num_finished = 0;

            frame.timestamp
        };



        let (image_idx, acquire_future) = {
            let aquire_res = swapchain::acquire_next_image(swapchain.clone(), None);
            match aquire_res {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    // TODO: is `continue` correct here regarding timing?
                    recreate_swapchain = true;
                    continue;
                },
                Err(e) => Err(e)?,
            }
        };

        // We need to find out the current physical window size to know how to
        // stretch the texture.
        let dpi_factor = *shared.window_dpi_factor.lock().unwrap();
        let logical_size = *shared.window_size.lock().unwrap();
        let physical_size = logical_size.to_physical(dpi_factor);
        let scale_x = physical_size.width / SCREEN_WIDTH as f64;
        let scale_y = physical_size.height / SCREEN_HEIGHT as f64;
        let scale = if scale_x > scale_y { scale_y } else { scale_x };

        let push_constants = vs::ty::PushConstants {
            scale_factor: [(scale_x / scale) as f32, (scale_y / scale) as f32],
        };

        // Build command buffer
        let clear_values = vec!([0.0, 0.0, 0.0, 1.0].into());
        let command_buffer
            = AutoCommandBufferBuilder::primary_one_time_submit(device.clone(), queue.family())?
            .copy_buffer_to_image(screen_buffer.clone(), tex.clone())?
            .begin_render_pass(framebuffers[image_idx].clone(), false, clear_values)?
            .draw(
                pipeline.clone(),
                &dynamic_state,
                vertex_buffer.clone(),
                descriptor_set.clone(),
                push_constants,
            )?
            .end_render_pass()?
            .build()?;

        let future = acquire_future
            .then_execute(queue.clone(), command_buffer)?
            .then_swapchain_present(queue.clone(), swapchain.clone(), image_idx)
            .then_signal_fence_and_flush();


        match future {
            Ok(future) => {
                // Block until complete
                // TODO: call `cleanup_finished?`
                drop(future);
            }
            Err(FlushError::OutOfDate) => {
                recreate_swapchain = true;
            }
            Err(e) => Err(e)?,
        }

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
                "{} (emu: {:.1} FPS / {:3}%, Vulkan: {:.1} FPS, delay: {:.1?}, dropped: {})",
                WINDOW_TITLE,
                emu_fps,
                emu_percent.round(),
                ogl_fps,
                emu_to_display_delay,
                shared.dropped_frames.load(Ordering::SeqCst),
            );
            *shared.window_title.lock().unwrap() = title;
            shared.event_thread.wakeup()?;
        }
    }

    Ok(())
}

fn create_framebuffers(
    swapchain_images: &[Arc<SwapchainImage<Window>>],
    render_pass: Arc<dyn RenderPassAbstract + Send + Sync>,
    dynamic_state: &mut DynamicState,
) -> Result<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>, Error> {
    let dimensions = swapchain_images[0].dimensions();

    let viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: [dimensions[0] as f32, dimensions[1] as f32],
        depth_range: 0.0 .. 1.0,
    };
    dynamic_state.viewports = Some(vec!(viewport));

    swapchain_images.iter().map(|image| {
        let fb = Framebuffer::start(render_pass.clone())
            .add(image.clone())?
            .build()?;

        Ok(Arc::new(fb) as Arc<dyn FramebufferAbstract + Send + Sync>)
    }).collect()
}

fn inner_size(window: &Window) -> Result<[u32; 2], Error> {
    let dimensions: (u32, u32) = window.get_inner_size()
        .ok_or(failure::err_msg("window unexpectedly closed"))?
        .to_physical(window.get_hidpi_factor())
        .into();

    Ok([dimensions.0, dimensions.1])
}
