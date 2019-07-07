use std::{
    time::{Duration, Instant},
};

use failure::{bail, Error};
use glium::{
    Display, Program, VertexBuffer, Surface,
    implement_vertex, uniform,
    glutin::{WindowedContext, NotCurrent},
    index::NoIndices,
    program::ProgramCreationInput,
    texture::{
        UnsignedTexture2d, UncompressedUintFormat, MipmapsOption,
        pixel_buffer::PixelBuffer,
    },
};
use spin_sleep::LoopHelper;

use mahboi::{
    SCREEN_WIDTH, SCREEN_HEIGHT,
    log::*,
};
use crate::{DurationExt, Shared, RenderTiming, WINDOW_TITLE, TARGET_FPS};



/// Renders the front buffer of `gb_buffer` to the host screen at the host
/// refresh rate.
pub(crate) fn render_thread(
    context: WindowedContext<NotCurrent>,
    shared: Shared,
) -> Result<(), Error> {
    let display = Display::from_gl_window(context)?;

    // We need to load some raw OpenGL functions that we are gonna use later.
    // Of course, glium already loaded everything, but it does not let us
    // access those, so we need to use `gl`.
    unsafe {
        display.exec_in_context(|| {
            let mut loader = |symbol| display.gl_window().get_proc_address(symbol) as *const _;
            gl::GetError::load_with(&mut loader);
            gl::GetIntegerv::load_with(&mut loader);
            gl::ReadBuffer::load_with(&mut loader);
            gl::ReadPixels::load_with(&mut loader);
        });
    }

    // Create the pixel buffer and initialize all pixels with black.
    let pixel_buffer = PixelBuffer::new_empty(&display, SCREEN_WIDTH * SCREEN_HEIGHT);
    pixel_buffer.write(&vec![(0u8, 0, 0); SCREEN_WIDTH * SCREEN_HEIGHT]);

    // Create an empty, uninitialized texture
    let texture = UnsignedTexture2d::empty_with_format(
        &display,
        UncompressedUintFormat::U8U8U8,
        MipmapsOption::NoMipmap,
        SCREEN_WIDTH as u32,
        SCREEN_HEIGHT as u32,
    )?;


    #[derive(Copy, Clone)]
    struct Vertex {
        position: [f32; 2],
        tex_coords: [f32; 2],
    }

    implement_vertex!(Vertex, position, tex_coords);

    // Create the full screen quad
    let shape = vec![
        Vertex { position: [-1.0, -1.0], tex_coords: [0.0, 1.0] },
        Vertex { position: [-1.0,  1.0], tex_coords: [0.0, 0.0] },
        Vertex { position: [ 1.0, -1.0], tex_coords: [1.0, 1.0] },
        Vertex { position: [ 1.0,  1.0], tex_coords: [1.0, 0.0] },
    ];

    let vertex_buffer = VertexBuffer::new(&display, &shape)?;
    let indices = NoIndices(glium::index::PrimitiveType::TriangleStrip);


    // Compile program. We have to do it via `ProgramCreationInput` to set
    // `outputs_srgb` to `true`. This is an ugly workaround for a bug
    // somewhere in the window creation stack. The framebuffer is
    // incorrectly created as sRGB and glium then automatically converts
    // all values returned by the fragment shader into sRGB. We don't want
    // a conversion, so we just tell glium we already output sRGB (which we
    // don't).
    let program = Program::new(
        &display,
        ProgramCreationInput::SourceCode {
            vertex_shader: include_str!("shader/simple.vert"),
            tessellation_control_shader: None,
            tessellation_evaluation_shader: None,
            geometry_shader: None,
            fragment_shader: include_str!("shader/simple.frag"),
            transform_feedback_varyings: None,
            outputs_srgb: true,
            uses_point_size: false,
        }
    )?;

    let mut loop_helper = LoopHelper::builder()
        .report_interval_s(0.5)
        .build_without_target_rate();

    // We want to delay drawing the buffer with OpenGL to reduce input lag. It
    // is difficult to figure out how long we should wait with drawing, though!
    // Visualizing frame timing:
    //
    //  V-Blank                        V-Blank                        V-Blank
    //     |                              |                              |
    //      [     sleep    ][draw][margin] [     sleep    ][draw][margin]
    //
    // We do this by trying to sync OpenGL to the CPU after issuing the last
    // draw command. Then we measure the time from the buffer swap command
    // until we read a pixel from the front buffer. This should be
    // approximately the time OpenGL waited for V-Blank to happen. In theory,
    // that's exactly the time we could sleep before drawing. However, drawing
    // time is not always the same and can vary from frame to frame. Also,
    // swapping the buffer still takes some time, even if V-Blank is right
    // around the corner. That's why we insert a 'margin' that we want OpenGL
    // to block waiting for V-Blank. Otherwise, we would often drop a frame.
    //
    // The draw delay starts at 0, but is continiously changed further down.
    let mut draw_delay = Duration::from_millis(0);

    // TODO: do not hardcode, but get from system
    let frame_time = Duration::from_micros(16_667);

    loop {
        loop_helper.loop_start();

        // We sleep before doing anything with OpenGL.
        trace!("sleeping {:.2?} before drawing", draw_delay);
        spin_sleep::sleep(draw_delay);

        *shared.state.render_timing.lock().unwrap() = RenderTiming {
            next_draw_start: Instant::now() + frame_time,
            frame_time,
        };

        // We map the pixel buffer and write directly to it.
        let frame_birth_time = {
            let frame = shared.state.gb_frame.lock()
                .expect("failed to lock front buffer");
            pixel_buffer.write(&*frame.buffer);
            frame.timestamp
        };

        // We update the texture data by uploading our pixel buffer.
        texture.main_level().raw_upload_from_pixel_buffer(
            pixel_buffer.as_slice(),
            0..SCREEN_WIDTH as u32,
            0..SCREEN_HEIGHT as u32,
            0..1,
        );

        // We need to find out the current physical window size to know how to
        // stretch the texture.
        let dpi_factor = *shared.state.window_dpi_factor.lock().unwrap();
        let logical_size = *shared.state.window_size.lock().unwrap();
        let physical_size = logical_size.to_physical(dpi_factor);
        let scale_x = physical_size.width / SCREEN_WIDTH as f64;
        let scale_y = physical_size.height / SCREEN_HEIGHT as f64;
        let scale = if scale_x > scale_y { scale_y } else { scale_x };
        let scale_factor = [(scale_x / scale) as f32, (scale_y / scale) as f32];


        // Draw the fullscreenquad to the framebuffer
        let mut target = display.draw();
        target.clear_color_srgb(0.0, 0.0, 0.0, 0.0);

        let uniforms = uniform! {
            scale_factor: scale_factor,
            tex: &texture,
        };
        target.draw(
            &vertex_buffer,
            &indices,
            &program,
            &uniforms,
            &Default::default(),
        )?;

        // We do our best to sync OpenGL to the CPU here. We issue a fence into
        // the command stream and then even call `glFinish()`. To really force
        // the driver to sync here, we could read from the back buffer, I
        // assume. But so far, it works fine.
        glium::SyncFence::new(&display).unwrap().wait();
        display.finish();
        let after_draw = Instant::now();

        // We swap buffers to present the finished framebuffer.
        //
        // But there is a little problem. We want OpenGL to wait now until
        // V-Blank (on the host system) has happened, i.e. we want this
        // function to block. But even with vsync enabled, it often doesn't
        // (depending on the driver). We could also call `glFinish` which
        // promises to block until all OpenGL operations are done, but this
        // function is incorrectly implemented in many drivers, too! Usually,
        // `glFinish` is a bad idea beause it hurts rendering performance. But
        // we don't care about this, we mostly care about latency. So we really
        // want to block here.
        //
        // The most reliable way to do that is to read from the front buffer.
        // That forces OpenGL to wait until that every operation that was
        // submitted before this read has completed. We only read a single
        // pixel and do not use that value, but this forces synchronization. We
        // need to use raw OpenGL here, because glium does not offer the
        // ability to read a single pixel from the front buffer.
        target.finish()?;
        let pixel = unsafe {
            display.exec_in_context(|| {
                // Get the currently bound `READ_BUFFER`
                let mut read_buffer_before: gl::types::GLint = 0;
                gl::GetIntegerv(gl::READ_BUFFER, &mut read_buffer_before);

                // Bind the front buffer and read one pixel from it
                gl::ReadBuffer(gl::FRONT);
                let mut pixel = [0u8; 4];
                let out_ptr = &mut pixel as *mut _ as *mut std::ffi::c_void;
                gl::ReadPixels(0, 0, 1, 1, gl::RGBA, gl::UNSIGNED_BYTE, out_ptr);

                // Bind the old buffer again (glium requires us to)
                gl::ReadBuffer(read_buffer_before as gl::types::GLenum);

                // There shouldn't be an error, but let's make sure.
                let e = gl::GetError();
                if e != 0 {
                    bail!("unexpected OpenGL error {}", e);
                }

                Ok(pixel)
            })?
        };
        let after_finish = Instant::now();
        let emu_to_display_delay = after_finish - frame_birth_time;
        trace!(
            "swapped buffers, delay {:.2?}, pixel at (0, 0) -> {:?}",
            emu_to_display_delay,
            pixel,
        );

        // Calculate new draw delay.
        draw_delay = {
            // How long OpenGL waited for V-Blank.
            let vblank_wait = after_finish - after_draw;

            // The theoretical new duration we could sleep.
            let new_value = draw_delay + vblank_wait;

            // Subtract the sleep margin from the theoretical value. That is to
            // avoid frame drops and account for draw time fluctuations.
            let new_value = new_value.saturating_sub(shared.state.args.host_block_margin);

            // Combine new value with the old one, depending on the learning
            // rate.
            let learn_rate = shared.state.args.host_delay_learn_rate as f64;
            let new_delay = (1.0 - learn_rate) * draw_delay.as_nanos() as f64
                + learn_rate * new_value.as_nanos() as f64;
            Duration::from_nanos(new_delay as u64)
        };

        // Potentially update the window title to show the current speed.
        if let Some(ogl_fps) = loop_helper.report_rate() {
            let emu_fps = *shared.state.emulation_rate.lock().unwrap();
            let emu_percent = (emu_fps / TARGET_FPS) * 100.0;
            let title = format!(
                "{} (emulator: {:.1} FPS / {:3}%, OpenGL: {:.1} FPS, delay: {:.1?})",
                WINDOW_TITLE,
                emu_fps,
                emu_percent.round(),
                ogl_fps,
                emu_to_display_delay,
            );
            display.gl_window().window().set_title(&title);
        }
    }
}
