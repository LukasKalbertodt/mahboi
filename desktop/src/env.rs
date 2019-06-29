#![allow(unused_imports)] // TODO

use std::{
    sync::{
        Arc,
        mpsc::{channel, Receiver, Sender},
        atomic::{AtomicU8, Ordering},
    },
    thread,
};

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
    display: Display,
    pixel_buffer: PixelBuffer<(u8, u8, u8)>,
    texture: UnsignedTexture2d,
    vertex_buffer: VertexBuffer<Vertex>,
    indices: NoIndices,
    program: Program,

    keys: AtomicKeys,
    input_actions: Receiver<Action>,

    should_stop: bool,
}

impl NativeWindow {
    /// Opens a window configured by `args`.
    pub(crate) fn open(args: &Args) -> Result<Self, Error> {
        // Create basic glium and glutin structures.
        let keys = AtomicKeys::none();
        let (actions_tx, actions_rx) = channel();
        let context = {
            let (tx, rx) = channel();
            let keys = keys.clone();

            thread::spawn(move || {
                let events_loop = EventsLoop::new();
                let wb = WindowBuilder::new();
                let cb = ContextBuilder::new().with_srgb(false);

                let context = cb.build_windowed(wb, &events_loop);
                tx.send(context).unwrap();

                // Start polling input events
                input_thread(events_loop, keys, actions_tx);
            });

            rx.recv().unwrap()?
        };

        let display = Display::from_gl_window(context)?;
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
            display,
            pixel_buffer,
            texture,
            vertex_buffer,
            indices,
            program,
            keys,
            input_actions: actions_rx,

            should_stop: false,
        })
    }

    /// Returns `true` if the window received signals to stop.
    pub(crate) fn should_stop(&self) -> bool {
        self.should_stop
    }

    /// Updates the window with the internal buffer and handles new events.
    pub(crate) fn update(&mut self) -> Result<(), Error> {
        for action in self.input_actions.try_iter() {
            match action {
                Action::Quit => self.should_stop = true,
            }
        }

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

/// The function that is run in the input thread. It just listens for input
/// events and handles them.
fn input_thread(mut events_loop: EventsLoop, keys: AtomicKeys, input_actions: Sender<Action>) {
    use glium::glutin::{
        ControlFlow, ElementState as State, Event, KeyboardInput,
        VirtualKeyCode as Key, WindowEvent,
    };

    // Mini helper function
    let send_action = |action| {
        input_actions.send(action)
            .expect("failed to send input action: input thread will panic now");
    };

    events_loop.run_forever(move |event| {
        // First, we extract the inner window event as that's what we are
        // interested in.
        let event = match event {
            // That's what we want!
            Event::WindowEvent { event, .. } => event,

            // When the main thread wakes us up, we just stop this thread.
            Event::Awakened => return ControlFlow::Break,

            // We ignore all other events (device events).
            _ => return ControlFlow::Continue,
        };

        // Now handle window events.
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => send_action(Action::Quit),

            // A key input that has a virtual keycode attached
            WindowEvent::KeyboardInput {
                input: KeyboardInput { virtual_keycode: Some(key), state, modifiers, .. },
                ..
            } => {
                match key {
                    // Button keys
                    Key::M if state == State::Pressed => keys.set_key(JoypadKey::Start),
                    Key::M if state == State::Released => keys.unset_key(JoypadKey::Start),
                    Key::N if state == State::Pressed => keys.set_key(JoypadKey::Select),
                    Key::N if state == State::Released => keys.unset_key(JoypadKey::Select),
                    Key::J if state == State::Pressed => keys.set_key(JoypadKey::A),
                    Key::J if state == State::Released => keys.unset_key(JoypadKey::A),
                    Key::K if state == State::Pressed => keys.set_key(JoypadKey::B),
                    Key::K if state == State::Released => keys.unset_key(JoypadKey::B),

                    // Direction keys
                    Key::W if state == State::Pressed => keys.set_key(JoypadKey::Up),
                    Key::W if state == State::Released => keys.unset_key(JoypadKey::Up),
                    Key::A if state == State::Pressed => keys.set_key(JoypadKey::Left),
                    Key::A if state == State::Released => keys.unset_key(JoypadKey::Left),
                    Key::S if state == State::Pressed => keys.set_key(JoypadKey::Down),
                    Key::S if state == State::Released => keys.unset_key(JoypadKey::Down),
                    Key::D if state == State::Pressed => keys.set_key(JoypadKey::Right),
                    Key::D if state == State::Released => keys.unset_key(JoypadKey::Right),

                    // Other non-Gameboy related functions
                    Key::Q if state == State::Pressed && modifiers.ctrl
                        => send_action(Action::Quit),

                    _ => {}
                }
            }
            _ => {}
        }

        ControlFlow::Continue
    });

    debug!("Input thread shutting down");
}

impl Peripherals for NativeWindow {
    fn get_pressed_keys(&self) -> Keys {
        self.keys.as_keys()
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


/// All Gameboy key states stored in one atomic `u8`.
///
/// This is used to update the state from the input thread while reading it on
/// the emulation thread.
#[derive(Debug, Clone)]
struct AtomicKeys(Arc<AtomicU8>);

impl AtomicKeys {
    /// Returns a new instance with no key pressed.
    fn none() -> Self {
        let byte = Keys::none().0;
        Self(Arc::new(AtomicU8::new(byte)))
    }

    /// Returns the non-atomic keys by loading the atomic value.
    fn as_keys(&self) -> Keys {
        let byte = self.0.load(Ordering::SeqCst);
        Keys(byte)
    }

    /// Modify the atomic value to set the `key` as pressed.
    fn set_key(&self, key: JoypadKey) {
        let mut keys = self.as_keys();
        keys.set_key(key);
        self.0.store(keys.0, Ordering::SeqCst);
    }

    /// Modify the atomic value to set the `key` as unpressed.
    fn unset_key(&self, key: JoypadKey) {
        let mut keys = self.as_keys();
        keys.unset_key(key);
        self.0.store(keys.0, Ordering::SeqCst);
    }
}

/// Actions that the input thread can generate.
enum Action {
    /// The application should exit.
    Quit,
}
