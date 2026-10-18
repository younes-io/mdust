//! Runtime SIMD level for the x86 scanners. The crate is `no_std`, so
//! `is_x86_feature_detected!` is unavailable; cpuid is read once and cached.
use core::arch::x86_64::{__cpuid, __cpuid_count, _xgetbv};
use core::sync::atomic::{AtomicU8, Ordering};

pub const SSE2: u8 = 1;
pub const SSSE3: u8 = 2;
pub const AVX2: u8 = 3;

static LEVEL: AtomicU8 = AtomicU8::new(0);

#[inline(always)]
pub fn level() -> u8 {
    match LEVEL.load(Ordering::Relaxed) {
        0 => detect(),
        l => l,
    }
}

#[cold]
fn detect() -> u8 {
    let f1 = __cpuid(1);
    let ssse3 = f1.ecx & (1 << 9) != 0;
    let osxsave = f1.ecx & (1 << 27) != 0;
    let avx = f1.ecx & (1 << 28) != 0;
    let avx2 = __cpuid_count(7, 0).ebx & (1 << 5) != 0;
    // xgetbv faults unless OSXSAVE is set; bits 1 and 2 say the OS saves the YMM state.
    let ymm = osxsave && unsafe { xcr0() } & 6 == 6;
    let l = if avx && avx2 && ymm {
        AVX2
    } else if ssse3 {
        SSSE3
    } else {
        SSE2
    };
    LEVEL.store(l, Ordering::Relaxed);
    l
}

#[target_feature(enable = "xsave")]
unsafe fn xcr0() -> u64 {
    _xgetbv(0)
}

pub fn force(l: u8) {
    LEVEL.store(l, Ordering::Relaxed);
}
