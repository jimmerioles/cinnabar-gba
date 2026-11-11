//! Experimental browser frontend: a canvas renderer for the headless core.
//!
//! Build with `wasm-pack build crates/cinnabar-wasm --target web` and open
//! `web/index.html`. Audio and gamepad support are not wired up yet.

use cinnabar_core::{Cartridge, SCREEN_HEIGHT, SCREEN_WIDTH, System};
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData, window};

#[wasm_bindgen]
pub struct Emulator {
    system: System,
    context: CanvasRenderingContext2d,
    rgba: Vec<u8>,
}

#[wasm_bindgen]
impl Emulator {
    #[wasm_bindgen(constructor)]
    pub fn new(rom: &[u8], canvas_id: &str) -> Result<Emulator, JsValue> {
        let document = window()
            .ok_or_else(|| JsValue::from_str("no browser window"))?
            .document()
            .ok_or_else(|| JsValue::from_str("no document"))?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str("canvas not found"))?
            .dyn_into::<HtmlCanvasElement>()?;
        canvas.set_width(SCREEN_WIDTH as u32);
        canvas.set_height(SCREEN_HEIGHT as u32);
        let context = canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("2d context unavailable"))?
            .dyn_into::<CanvasRenderingContext2d>()?;

        Ok(Self {
            system: System::new(Cartridge::new(rom.to_vec())),
            context,
            rgba: vec![0; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
        })
    }

    pub fn run_frame(&mut self) -> Result<(), JsValue> {
        let output = self.system.run_frame();
        for (index, color) in output
            .framebuffer
            .iter()
            .take(SCREEN_WIDTH * SCREEN_HEIGHT)
            .enumerate()
        {
            let [red, green, blue] = bgr555_to_rgb(*color);
            let target = index * 4;
            self.rgba[target] = red;
            self.rgba[target + 1] = green;
            self.rgba[target + 2] = blue;
            self.rgba[target + 3] = 255;
        }

        let data = js_sys::Uint8ClampedArray::new_with_length(self.rgba.len() as u32);
        data.copy_from(&self.rgba);
        let image = ImageData::new_with_js_u8_clamped_array_and_sh(
            &data,
            SCREEN_WIDTH as u32,
            SCREEN_HEIGHT as u32,
        )?;
        self.context.put_image_data(&image, 0.0, 0.0)
    }

    pub fn set_keys(&mut self, keys: u16) {
        self.system.set_keys(keys);
    }
}

fn bgr555_to_rgb(color: u16) -> [u8; 3] {
    let expand = |value: u16| ((value << 3) | (value >> 2)) as u8;
    [
        expand(color & 0x1F),
        expand((color >> 5) & 0x1F),
        expand((color >> 10) & 0x1F),
    ]
}
