#![cfg(target_os = "linux")]

extern "C" {
    /// Flushes the CPU instruction cache (provided by glibc on Linux).
    pub(crate) fn __clear_cache(start: *mut u8, end: *mut u8);
}
