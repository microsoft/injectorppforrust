#![cfg(target_os = "windows")]

use core::ffi::c_void;

pub(crate) const MEM_COMMIT: u32 = 0x1000;
pub(crate) const MEM_RESERVE: u32 = 0x2000;
pub(crate) const PAGE_EXECUTE_READWRITE: u32 = 0x40;
pub(crate) const MEM_RELEASE: u32 = 0x8000;

#[repr(C)]
struct SystemInfo {
    w_processor_architecture: u16,
    w_reserved: u16,
    dw_page_size: u32,
    lp_minimum_application_address: *mut c_void,
    lp_maximum_application_address: *mut c_void,
    dw_active_processor_mask: usize,
    dw_number_of_processors: u32,
    dw_processor_type: u32,
    dw_allocation_granularity: u32,
    w_processor_level: u16,
    w_processor_revision: u16,
}

extern "system" {
    pub(crate) fn VirtualProtect(
        lpAddress: *mut c_void,
        dwSize: usize,
        flNewProtect: u32,
        lpflOldProtect: *mut u32,
    ) -> i32;

    pub(crate) fn VirtualAlloc(
        lpAddress: *mut c_void,
        dwSize: usize,
        flAllocationType: u32,
        flProtect: u32,
    ) -> *mut c_void;

    pub(crate) fn VirtualFree(lpAddress: *mut c_void, dwSize: usize, dwFreeType: u32) -> i32;

    pub(crate) fn FlushInstructionCache(
        hProcess: *mut c_void,
        lpBaseAddress: *const c_void,
        dwSize: usize,
    ) -> i32;

    pub(crate) fn GetCurrentProcess() -> *mut c_void;

    fn GetSystemInfo(lpSystemInfo: *mut SystemInfo);
}

pub(crate) unsafe fn get_page_size() -> usize {
    let mut sysinfo = core::mem::zeroed::<SystemInfo>();
    GetSystemInfo(&mut sysinfo);
    sysinfo.dw_page_size as usize
}
