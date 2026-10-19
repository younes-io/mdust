#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

#[cfg(target_arch = "wasm32")]
mod bump;

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: bump::Bump = bump::Bump;

// The JS glue writes UTF-8 into INPUT and reads the result out of OUTPUT.
// Both live across calls so a steady stream of renders does not malloc per
// call; the single-threaded wasm instance is the only reader or writer.
static mut INPUT: Vec<u8> = Vec::new();
static mut OUTPUT: String = String::new();

/// Returns a buffer of at least `cap` bytes for the caller to encode into.
#[no_mangle]
pub extern "C" fn mdust_in(cap: usize) -> *mut u8 {
    let input = unsafe { &mut *core::ptr::addr_of_mut!(INPUT) };
    let output = unsafe { &mut *core::ptr::addr_of_mut!(OUTPUT) };
    // The previous output has been read by now; everything restarts at the
    // arena base, so no render leaves anything behind.
    *output = String::new();
    *input = Vec::new();
    #[cfg(target_arch = "wasm32")]
    bump::reset();
    input.reserve(cap);
    input.as_mut_ptr()
}

/// Renders the first `len` bytes of the input buffer; the result is readable
/// at the returned pointer for `mdust_out_len()` bytes until the next call.
#[no_mangle]
pub extern "C" fn mdust_render(len: usize) -> *const u8 {
    let input = unsafe { &*core::ptr::addr_of!(INPUT) };
    let src = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(input.as_ptr(), len)) };
    let output = unsafe { &mut *core::ptr::addr_of_mut!(OUTPUT) };
    *output = mdust::render(src);
    output.as_ptr()
}

#[no_mangle]
pub extern "C" fn mdust_out_len() -> usize {
    let output = unsafe { &*core::ptr::addr_of!(OUTPUT) };
    output.len()
}
