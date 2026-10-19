//! Bump allocator for the one-shot render. `reset` runs before each render,
//! so nothing is ever freed individually; only the most recent block can be
//! grown or given back in place, which is the common case for the output
//! string and the token vectors.
use core::alloc::{GlobalAlloc, Layout};

const PAGE: usize = 65536;

pub struct Bump;

static mut START: usize = 0;
static mut CUR: usize = 0;
static mut END: usize = 0;
static mut LAST: usize = 0;

pub fn reset() {
    unsafe {
        if START == 0 {
            START = core::arch::wasm32::memory_size(0) * PAGE;
            END = START;
        }
        CUR = START;
        LAST = START;
    }
}

#[inline]
unsafe fn bump(size: usize, align: usize) -> *mut u8 {
    let start = (CUR + align - 1) & !(align - 1);
    let new_cur = start + size;
    if new_cur > END {
        let need = new_cur - END;
        let pages = (need + PAGE - 1) / PAGE;
        let pages = pages.max(16);
        if core::arch::wasm32::memory_grow(0, pages) == usize::MAX {
            return core::ptr::null_mut();
        }
        END += pages * PAGE;
    }
    CUR = new_cur;
    LAST = start;
    start as *mut u8
}

unsafe impl GlobalAlloc for Bump {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if START == 0 {
            reset();
        }
        bump(layout.size(), layout.align())
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr as usize == LAST {
            CUR = LAST;
        }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ptr as usize == LAST {
            CUR = LAST;
            let p = bump(new_size, layout.align());
            // Growing the last block in place: the bytes are still there.
            return p;
        }
        let p = bump(new_size, layout.align());
        if !p.is_null() {
            core::ptr::copy_nonoverlapping(ptr, p, layout.size().min(new_size));
        }
        p
    }
}
