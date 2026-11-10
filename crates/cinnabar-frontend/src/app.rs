//! The interactive winit + softbuffer application.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use cinnabar_core::keypad::keys;
use cinnabar_core::{SCREEN_HEIGHT, SCREEN_WIDTH, System};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

use crate::audio::AudioOutput;
use crate::storage;
use crate::video::{self, WindowSurface};

/// 280,896 cycles per frame at 16.78 MHz is 59.7275 Hz.
const FRAME_PERIOD: Duration = Duration::from_nanos(16_742_706);
/// Frames emulated per presented frame in fast-forward mode.
const FAST_FORWARD_FRAMES: u32 = 4;

pub struct AppConfig {
    pub title: String,
    pub save_path: Option<PathBuf>,
    pub scale: u32,
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
    keys: u16,
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
            keys: 0,
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
            Ok(surface) => surface,
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
        }

        self.system.set_keys(self.keys);
        self.run_frame();
        self.next_frame = Instant::now() + FRAME_PERIOD;
        window.request_redraw();
    }

    fn run_frame(&mut self) {
        let output = self.system.run_frame();
        if let Some(audio) = &mut self.audio {
            audio.push(&output.audio);
        }
    }

    fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        self.next_frame = Instant::now() + FRAME_PERIOD;
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
                KeyCode::F2 => self.fast_forward = !self.fast_forward,
                KeyCode::F5 => self.take_screenshot(),
                KeyCode::F11 => self.toggle_fullscreen(),
                _ => {}
            }
        }

        if let Some(bit) = key_bit(code) {
            if pressed {
                self.keys |= bit;
            } else {
                self.keys &= !bit;
            }
            self.system.set_keys(self.keys);
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
                self.keys = 0;
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

fn key_bit(code: KeyCode) -> Option<u16> {
    Some(match code {
        KeyCode::KeyZ => keys::A,
        KeyCode::KeyX => keys::B,
        KeyCode::Enter => keys::START,
        KeyCode::Backspace => keys::SELECT,
        KeyCode::ArrowUp => keys::UP,
        KeyCode::ArrowDown => keys::DOWN,
        KeyCode::ArrowLeft => keys::LEFT,
        KeyCode::ArrowRight => keys::RIGHT,
        KeyCode::KeyA => keys::L,
        KeyCode::KeyS => keys::R,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_the_documented_keyboard_controls() {
        assert_eq!(key_bit(KeyCode::KeyZ), Some(keys::A));
        assert_eq!(key_bit(KeyCode::KeyX), Some(keys::B));
        assert_eq!(key_bit(KeyCode::Enter), Some(keys::START));
        assert_eq!(key_bit(KeyCode::Backspace), Some(keys::SELECT));
        assert_eq!(key_bit(KeyCode::ArrowUp), Some(keys::UP));
        assert_eq!(key_bit(KeyCode::ArrowDown), Some(keys::DOWN));
        assert_eq!(key_bit(KeyCode::ArrowLeft), Some(keys::LEFT));
        assert_eq!(key_bit(KeyCode::ArrowRight), Some(keys::RIGHT));
        assert_eq!(key_bit(KeyCode::KeyA), Some(keys::L));
        assert_eq!(key_bit(KeyCode::KeyS), Some(keys::R));
        assert_eq!(key_bit(KeyCode::Escape), None);
    }
}
