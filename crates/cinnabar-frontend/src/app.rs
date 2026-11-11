//! The interactive winit + softbuffer application.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use cinnabar_core::{Cartridge, SCREEN_HEIGHT, SCREEN_WIDTH, System};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

use crate::audio::AudioOutput;
use crate::gamepad::Gamepad;
use crate::keymap::KeyMap;
use crate::storage;
use crate::video::{self, Shader, WindowSurface};

/// 280,896 cycles per frame at 16.78 MHz is 59.7275 Hz.
const FRAME_PERIOD: Duration = Duration::from_nanos(16_742_706);
/// Frames emulated per presented frame in fast-forward mode.
const FAST_FORWARD_FRAMES: u32 = 4;

pub struct AppConfig {
    pub title: String,
    pub save_path: Option<PathBuf>,
    pub scale: u32,
    pub shader: Shader,
    pub volume: f32,
    pub keymap: KeyMap,
}

/// Build the event loop and run until the user quits.
pub fn run(system: System, config: AppConfig) -> Result<()> {
    let event_loop = EventLoop::new().context("failed to create the window event loop")?;
    let mut app = App::new(system, config);
    event_loop
        .run_app(&mut app)
        .context("the window event loop failed")?;
    if let Some(error) = app.fatal.take() {
        return Err(error);
    }
    Ok(())
}

struct App {
    system: System,
    config: AppConfig,
    window: Option<Arc<Window>>,
    surface: Option<WindowSurface>,
    audio: Option<AudioOutput>,
    audio_attempted: bool,
    keyboard_keys: u16,
    gamepad_keys: u16,
    gamepad: Option<Gamepad>,
    paused: bool,
    fast_forward: bool,
    fullscreen: bool,
    next_frame: Instant,
    fatal: Option<anyhow::Error>,
    present_error_reported: bool,
}

impl App {
    fn new(system: System, config: AppConfig) -> Self {
        Self {
            system,
            config,
            window: None,
            surface: None,
            audio: None,
            audio_attempted: false,
            keyboard_keys: 0,
            gamepad_keys: 0,
            gamepad: Gamepad::new()
                .map_err(|error| {
                    eprintln!("cinnabar: gamepad unavailable: {error:#}");
                })
                .ok(),
            paused: false,
            fast_forward: false,
            fullscreen: false,
            next_frame: Instant::now() + FRAME_PERIOD,
            fatal: None,
            present_error_reported: false,
        }
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) {
        let scale = self.config.scale.max(1);
        let attributes = Window::default_attributes()
            .with_title(self.config.title.clone())
            .with_inner_size(LogicalSize::new(
                (SCREEN_WIDTH as u32).saturating_mul(scale),
                (SCREEN_HEIGHT as u32).saturating_mul(scale),
            ))
            .with_min_inner_size(LogicalSize::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.fatal = Some(
                    anyhow::Error::new(error).context("failed to create the application window"),
                );
                event_loop.exit();
                return;
            }
        };

        let surface = match WindowSurface::new(window.clone()) {
            Ok(mut surface) => {
                surface.set_shader(self.config.shader);
                surface
            }
            Err(error) => {
                self.fatal = Some(error.context("failed to initialize window rendering"));
                event_loop.exit();
                return;
            }
        };

        self.window = Some(window.clone());
        self.surface = Some(surface);

        if !self.audio_attempted {
            self.audio_attempted = true;
            self.audio = AudioOutput::start(self.system.bus.apu.sample_rate());
            if let Some(audio) = &mut self.audio {
                audio.set_volume(self.config.volume);
            }
        }

        self.system.set_keys(self.combined_keys());
        self.run_frame();
        self.next_frame = Instant::now() + FRAME_PERIOD;
        self.update_window_title();
        window.request_redraw();
    }

    fn combined_keys(&self) -> u16 {
        self.keyboard_keys | self.gamepad_keys
    }

    fn run_frame(&mut self) {
        let output = self.system.run_frame();
        if let Some(audio) = &mut self.audio {
            audio.push(&output.audio);
        }
    }

    fn update_window_title(&self) {
        let mut title = self.config.title.clone();
        if self.paused {
            title.push_str(" — paused");
        }
        if self.fast_forward {
            title.push_str(" — fast-forward");
        }
        if self.config.shader != Shader::default() {
            title.push_str(" — ");
            title.push_str(&self.config.shader.label());
        }
        if let Some(window) = &self.window {
            window.set_title(&title);
        }
    }

    fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        self.next_frame = Instant::now() + FRAME_PERIOD;
        self.update_window_title();
    }

    fn load_rom(&mut self, path: PathBuf) {
        if let Err(error) = self.try_load_rom(&path) {
            eprintln!("cinnabar: failed to load {}: {error:#}", path.display());
        }
    }

    fn try_load_rom(&mut self, path: &Path) -> Result<()> {
        self.persist_save();
        let rom = storage::read_rom(path)?;
        let cart = Cartridge::new(rom);
        let title = crate::cli::window_title(&cart, Some(path));
        let save_path = storage::resolve_save_path(None, Some(path));
        let mut system = System::new(cart);
        if let Err(error) = storage::load_save(&mut system, save_path.as_deref()) {
            eprintln!("cinnabar: warning: {error:#}");
        }

        self.system = system;
        self.config.title = title;
        self.config.save_path = save_path;
        self.keyboard_keys = 0;
        self.gamepad_keys = 0;
        self.system.set_keys(0);
        self.paused = false;
        self.fast_forward = false;
        self.next_frame = Instant::now() + FRAME_PERIOD;
        self.update_window_title();
        self.run_frame();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        println!("cinnabar: loaded {}", path.display());
        Ok(())
    }

    fn take_screenshot(&mut self) {
        let path = video::screenshot_path();
        match video::save_screenshot(&path, self.system.framebuffer()) {
            Ok(()) => println!("cinnabar: wrote screenshot {}", path.display()),
            Err(error) => eprintln!("cinnabar: screenshot failed: {error:#}"),
        }
    }

    fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        if let Some(window) = &self.window {
            window.set_fullscreen(if self.fullscreen {
                Some(Fullscreen::Borderless(None))
            } else {
                None
            });
        }
    }

    fn persist_save(&mut self) {
        if let Err(error) =
            storage::persist_save(&mut self.system, self.config.save_path.as_deref())
        {
            eprintln!("cinnabar: failed to write save: {error:#}");
        }
    }

    fn exit(&mut self, event_loop: &ActiveEventLoop) {
        self.persist_save();
        event_loop.exit();
    }

    fn handle_key(&mut self, event: &KeyEvent) -> bool {
        let pressed = event.state == ElementState::Pressed;
        let PhysicalKey::Code(code) = event.physical_key else {
            return false;
        };

        if pressed && !event.repeat {
            match code {
                KeyCode::Escape => return true,
                KeyCode::F1 => self.toggle_pause(),
                KeyCode::F2 => {
                    self.fast_forward = !self.fast_forward;
                    self.update_window_title();
                }
                KeyCode::F5 => self.take_screenshot(),
                KeyCode::F11 => self.toggle_fullscreen(),
                _ => {}
            }
        }

        if let Some(bit) = self.config.keymap.bit_for(code) {
            if pressed {
                self.keyboard_keys |= bit;
            } else {
                self.keyboard_keys &= !bit;
            }
            self.system.set_keys(self.combined_keys());
        }

        false
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            self.create_window(event_loop);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let is_ours = self
            .window
            .as_ref()
            .is_some_and(|window| window.id() == window_id);
        if !is_ours {
            return;
        }

        match event {
            WindowEvent::CloseRequested => self.exit(event_loop),
            WindowEvent::Destroyed => event_loop.exit(),
            WindowEvent::DroppedFile(path) => self.load_rom(path),
            WindowEvent::Resized(size) => {
                if let Some(surface) = &mut self.surface {
                    if let Err(error) = surface.resize(size.width, size.height) {
                        eprintln!("cinnabar: window resize failed: {error:#}");
                    }
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if self.handle_key(&event) {
                    self.exit(event_loop);
                }
            }
            WindowEvent::Focused(false) => {
                self.keyboard_keys = 0;
                self.gamepad_keys = 0;
                self.system.set_keys(0);
            }
            WindowEvent::RedrawRequested => {
                let result = if let Some(surface) = &mut self.surface {
                    surface.present(self.system.framebuffer())
                } else {
                    Ok(())
                };
                if let Err(error) = result {
                    if !self.present_error_reported {
                        eprintln!("cinnabar: present failed: {error:#}");
                        self.present_error_reported = true;
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.fatal.is_some() {
            event_loop.exit();
            return;
        }

        let Some(window) = self.window.clone() else {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        };

        if let Some(gamepad) = &mut self.gamepad {
            let gamepad_keys = gamepad.poll();
            if gamepad_keys != self.gamepad_keys {
                self.gamepad_keys = gamepad_keys;
                self.system.set_keys(self.combined_keys());
            }
        }

        if self.paused {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }

        let now = Instant::now();
        if now < self.next_frame {
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
            return;
        }

        let frames = if self.fast_forward {
            FAST_FORWARD_FRAMES
        } else {
            1
        };
        for _ in 0..frames {
            self.run_frame();
        }

        // Advance on a fixed cadence, but do not accumulate backlog after a
        // slow frame or a suspend/resume cycle.
        let finished = Instant::now();
        self.next_frame += FRAME_PERIOD;
        if self.next_frame <= finished {
            self.next_frame = finished + FRAME_PERIOD;
        }

        window.request_redraw();
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.persist_save();
    }
}
