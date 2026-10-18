#![cfg_attr(target_arch = "wasm32", no_std)]
extern crate alloc;

use alloc::string::String;
use wasm_bindgen::prelude::*;

// std's panic runtime pulls in formatting and backtrace code that costs about 40 KB of wasm.
#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[wasm_bindgen]
pub fn render(src: &str) -> String {
    mdust::render(src)
}
