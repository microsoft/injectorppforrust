#![cfg(target_os = "macos")]

extern "C" {
    /// Prepares memory for execution, typically by invalidating the instruction cache for the
    /// indicated range.
    pub(crate) fn sys_icache_invalidate(start: *mut u8, len: usize);
}
