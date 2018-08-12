#![feature(use_extern_macros)]

extern crate wasm_bindgen;
use wasm_bindgen::prelude::*;


#[wasm_bindgen]
pub fn get_color(x: u8, y: u8) -> Color {
    Color {
        r: x.saturating_mul(2),
        g: y.saturating_mul(2),
        b: (x.wrapping_sub(y) % 50) * 5,
    }
}

#[wasm_bindgen]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}
