#![cfg(target_os = "macos")]

extern "C" {
    pub(crate) fn sys_dcache_flush(start: *mut u8, len: usize);
    pub(crate) fn sys_icache_invalidate(start: *mut u8, len: usize);
}
