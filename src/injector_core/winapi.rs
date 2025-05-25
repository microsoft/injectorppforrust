#![cfg(target_os = "windows")]

use core::ffi::c_void;

pub const MEM_COMMIT: u32 = 0x1000;
pub const MEM_RESERVE: u32 = 0x2000;
pub const PAGE_EXECUTE_READWRITE: u32 = 0x40;
pub const MEM_RELEASE: u32 = 0x8000;

#[repr(C)]
pub struct SYSTEM_INFO {
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
    pub fn GetSystemInfo(lpSystemInfo: *mut SYSTEM_INFO);

    pub fn VirtualProtect(
        lpAddress: *mut c_void,
        dwSize: usize,
        flNewProtect: u32,
        lpflOldProtect: *mut u32,
    ) -> i32;

    pub fn VirtualAlloc(
        lpAddress: *mut c_void,
        dwSize: usize,
        flAllocationType: u32,
        flProtect: u32,
    ) -> *mut c_void;

    pub fn VirtualFree(lpAddress: *mut c_void, dwSize: usize, dwFreeType: u32) -> i32;

    pub fn FlushInstructionCache(
        hProcess: *mut c_void,
        lpBaseAddress: *const c_void,
        dwSize: usize,
    ) -> i32;

    pub fn GetCurrentProcess() -> *mut c_void;
}

pub unsafe fn get_page_size() -> usize {
    let mut sysinfo = core::mem::zeroed::<SYSTEM_INFO>();
    GetSystemInfo(&mut sysinfo);
    sysinfo.dw_page_size as usize
}
