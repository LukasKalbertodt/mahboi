#![allow(unused_imports)] // TODO

use failure::{Error, ResultExt};
use glium::{
    Display, Program, VertexBuffer, Surface,
    implement_vertex, uniform,
    glutin::{ContextBuilder, EventsLoop, WindowBuilder},
    index::NoIndices,
    program::ProgramCreationInput,
    texture::{
        UnsignedTexture2d, UncompressedUintFormat, MipmapsOption,
        pixel_buffer::PixelBuffer,
    },
};

use mahboi::{
    SCREEN_WIDTH, SCREEN_HEIGHT,
    log::*,
    env::Peripherals,
    primitives::PixelColor,
    machine::input::{Keys, JoypadKey},
};
use crate::{
    args::Args,
};



const WINDOW_TITLE: &str = "Mahboi";

/// Native application window which also handles input and sound.
pub(crate) struct NativeWindow {
    events_loop: EventsLoop,
    display: Display,
    pixel_buffer: PixelBuffer<(u8, u8, u8)>,
    texture: UnsignedTexture2d,
    vertex_buffer: VertexBuffer<Vertex>,
    indices: NoIndices,
    program: Program,
}

impl NativeWindow {
    /// Opens a window configured by `args`.
    pub(crate) fn open(args: &Args) -> Result<Self, Error> {
        // Create basic glium and glutin structures.
        let events_loop = EventsLoop::new();
        let wb = WindowBuilder::new();
        let cb = ContextBuilder::new().with_srgb(false);
        let display = Display::new(wb, cb, &events_loop)?;
        info!("[desktop] Opened window");

        // Create the pixel buffer and initialize all pixels with black.
        let pixel_buffer = PixelBuffer::new_empty(&display, SCREEN_WIDTH * SCREEN_HEIGHT);
        pixel_buffer.write(&vec![(0, 0, 0); SCREEN_WIDTH * SCREEN_HEIGHT]);

        // Create an empty, uninitialized texture
        let texture = UnsignedTexture2d::empty_with_format(
            &display,
            UncompressedUintFormat::U8U8U8,
            MipmapsOption::NoMipmap,
            SCREEN_WIDTH as u32,
            SCREEN_HEIGHT as u32,
        )?;

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


        Ok(Self {
            events_loop,
            display,
            pixel_buffer,
            texture,
            vertex_buffer,
            indices,
            program,
        })
    }

    /// Returns `true` if the window received signals to stop.
    pub(crate) fn should_stop(&self) -> bool {
        false
    }

    /// Updates the window with the internal buffer and handles new events.
    pub(crate) fn update(&mut self) -> Result<(), Error> {
        Ok(())
    }

    pub(crate) fn draw(&self) -> Result<(), Error> {
        // We update the texture data by uploading our pixel buffer.
        self.texture.main_level().raw_upload_from_pixel_buffer(
            self.pixel_buffer.as_slice(),
            0..SCREEN_WIDTH as u32,
            0..SCREEN_HEIGHT as u32,
            0..1,
        );

        // Draw the fullscreenquad to the framebuffer
        let mut target = self.display.draw();
        target.draw(
            &self.vertex_buffer,
            &self.indices,
            &self.program,
            &uniform! { tex: &self.texture },
            &Default::default(),
        )?;
        target.finish()?;

        Ok(())
    }

    pub(crate) fn set_title_postfix(&mut self, postfix: &str) {
        let new_title = format!("{} - {}", WINDOW_TITLE, postfix);
        unimplemented!()
    }

    pub(crate) fn in_turbo_mode(&self) -> bool {
        unimplemented!()
    }

    pub(crate) fn reset_to_pink(&mut self) {
        unimplemented!()
    }
}

impl Peripherals for NativeWindow {
    fn get_pressed_keys(&self) -> Keys {
        Keys::none()
    }

    fn write_lcd_line(&mut self, line_idx: u8, pixels: &[PixelColor; SCREEN_WIDTH]) {
        // We map the pixel buffer and write directly to it.
        let mut mapping = self.pixel_buffer.map_write();
        let offset = line_idx as usize * SCREEN_WIDTH;
        for col in 0..SCREEN_WIDTH {
            let PixelColor { r, g, b } = pixels[col];
            mapping.set(offset + col, (r, g, b));
        }
    }
}

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

implement_vertex!(Vertex, position, tex_coords);
