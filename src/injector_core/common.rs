use libc::*;
use std::ptr;

#[cfg(target_os = "windows")]
use crate::injector_core::winapi::*;

#[cfg(target_os = "linux")]
use crate::injector_core::linuxapi::*;

/// Allocates a block of executable memory near the provided source address,
/// ensuring that the allocated memory lies within ±128MB of the source.
/// This mirrors the C++ approach.
pub(crate) fn allocate_jit_memory(src: *const u8, code_size: usize) -> *mut u8 {
    #[cfg(target_os = "linux")]
    {
        allocate_jit_memory_linux(src, code_size)
    }

    #[cfg(target_os = "windows")]
    {
        allocate_jit_memory_windows(src, code_size)
    }
}

#[cfg(target_os = "linux")]
fn allocate_jit_memory_linux(src: *const u8, code_size: usize) -> *mut u8 {
    let max_range: u64 = 0x8000000; // 128MB
    let original_addr = src as u64;
    let page_size = unsafe { sysconf(_SC_PAGESIZE) as u64 };
    // Start at original_addr - max_range.
    let mut start_address = original_addr.saturating_sub(max_range);
    loop {
        let ptr = unsafe {
            libc::mmap(
                start_address as *mut c_void,
                code_size,
                PROT_READ | PROT_WRITE | PROT_EXEC,
                libc::MAP_ANONYMOUS | libc::MAP_PRIVATE,
                -1,
                0,
            )
        };
        if ptr != libc::MAP_FAILED {
            let allocated = ptr as u64;
            let diff = if allocated > original_addr {
                allocated - original_addr
            } else {
                original_addr - allocated
            };
            if diff <= max_range {
                return ptr as *mut u8;
            } else {
                unsafe { libc::munmap(ptr, code_size) };
            }
        }
        start_address += page_size;
        if start_address > original_addr + max_range {
            panic!("Failed to allocate jit memory within +/-128MB from original function address");
        }
    }
}

#[cfg(target_os = "windows")]
fn allocate_jit_memory_windows(src: *const u8, code_size: usize) -> *mut u8 {
    let max_range: u64 = 0x8000000; // 128MB
    let original_addr = src as u64;
    let page_size = unsafe { get_page_size() as u64 };
    let mut start_address = original_addr.saturating_sub(max_range);

    loop {
        let ptr = unsafe {
            VirtualAlloc(
                start_address as *mut c_void,
                code_size,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            )
        };
        if !ptr.is_null() {
            let allocated = ptr as u64;
            let diff = if allocated > original_addr {
                allocated - original_addr
            } else {
                original_addr - allocated
            };
            if diff <= max_range {
                return ptr as *mut u8;
            } else {
                // Free the memory because it's outside our desired range
                unsafe {
                    VirtualFree(ptr, 0, MEM_RELEASE);
                }
            }
        }
        start_address += page_size;
        if start_address > original_addr + max_range {
            panic!(
                "Failed to allocate executable memory within ±128MB of original function address"
            );
        }
    }
}

/// Unsafely reads `len` bytes from `ptr` and returns them as a Vec.
///
/// # Safety
///
/// The caller must ensure that `ptr` is valid for reading `len` bytes.
pub(crate) unsafe fn read_bytes(ptr: *const u8, len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    ptr::copy_nonoverlapping(ptr, buf.as_mut_ptr(), len);
    buf
}

/// A guard that stores the original bytes of a patched function and the allocated JIT memory.
/// When dropped, it restores the original function code and frees the JIT memory.
pub(crate) struct PatchGuard {
    func_ptr: *mut u8,
    original_bytes: Vec<u8>,
    patch_size: usize,
    jit_memory: *mut u8,

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    jit_size: usize,
}

impl PatchGuard {
    pub(crate) fn new(
        func_ptr: *mut u8,
        original_bytes: Vec<u8>,
        patch_size: usize,
        jit_memory: *mut u8,
        jit_size: usize,
    ) -> Self {
        Self {
            func_ptr,
            original_bytes,
            patch_size,
            jit_memory,
            jit_size,
        }
    }
}

impl Drop for PatchGuard {
    fn drop(&mut self) {
        unsafe {
            patch_function(self.func_ptr, &self.original_bytes[..self.patch_size]);
            if !self.jit_memory.is_null() {
                #[cfg(target_os = "linux")]
                {
                    libc::munmap(self.jit_memory as *mut c_void, self.jit_size);
                }

                #[cfg(target_os = "windows")]
                {
                    VirtualFree(self.jit_memory as *mut c_void, 0, MEM_RELEASE);
                }
            }

            // Explicitly flush cache and synchronize pipeline after restoring original bytes
            clear_cache(self.func_ptr, self.func_ptr.add(self.patch_size));
        }
    }
}

/// Unsafely patches the code at `func` with the given patch bytes.
///
/// # Safety
///
/// The caller must ensure that `func` points to a valid, patchable code region.
pub(crate) unsafe fn patch_function(func: *mut u8, patch: &[u8]) {
    make_memory_writable_and_executable(func);

    inject_asm_code(patch, func);
}

unsafe fn make_memory_writable_and_executable(func: *mut u8) {
    #[cfg(target_os = "linux")]
    {
        make_memory_writable_and_executable_linux(func);
    }

    #[cfg(target_os = "windows")]
    {
        make_memory_writable_and_executable_windows(func);
    }
}

#[cfg(target_os = "linux")]
unsafe fn make_memory_writable_and_executable_linux(func: *mut u8) {
    let page_size = sysconf(_SC_PAGESIZE) as usize;
    let addr = func as usize;
    let page_start = addr & !(page_size - 1);
    if libc::mprotect(
        page_start as *mut c_void,
        page_size,
        PROT_READ | PROT_WRITE | PROT_EXEC,
    ) != 0
    {
        panic!("mprotect failed");
    }
}

#[cfg(target_os = "windows")]
unsafe fn make_memory_writable_and_executable_windows(func: *const u8) {
    let page_size = get_page_size();
    let addr = func as usize;
    let page_start = addr & !(page_size - 1);

    let mut old_protect: u32 = 0;

    let result = VirtualProtect(
        page_start as *mut c_void,
        page_size,
        PAGE_EXECUTE_READWRITE,
        &mut old_protect,
    );

    if result == 0 {
        panic!("VirtualProtect failed");
    }
}

pub(crate) unsafe fn inject_asm_code(asm_code: &[u8], dest: *mut u8) {
    ptr::copy_nonoverlapping(asm_code.as_ptr(), dest, asm_code.len());
    clear_cache(dest, dest.add(asm_code.len()));
}

unsafe fn clear_cache(start: *mut u8, end: *mut u8) {
    #[cfg(target_os = "linux")]
    {
        __clear_cache(start, end)
    }

    #[cfg(target_os = "windows")]
    {
        let size = end.offset_from(start) as usize;
        let process = GetCurrentProcess();
        let success = FlushInstructionCache(process, start as *const c_void, size);

        if success == 0 {
            panic!("FlushInstructionCache failed");
        }
    }

    // On ARM64, explicitly synchronize the CPU pipeline.
    #[cfg(target_arch = "aarch64")]
    {
        core::arch::asm!("dsb sy", "isb", options(nostack, nomem));
    }
}
