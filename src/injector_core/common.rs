use libc::*;
use std::ptr;
use std::ptr::NonNull;

#[cfg(target_os = "windows")]
use crate::injector_core::winapi::*;

#[cfg(target_os = "linux")]
use crate::injector_core::linuxapi::*;

#[cfg(target_os = "macos")]
use crate::injector_core::macosapi::*;

/// A safe wrapper around a raw function pointer.
///
/// `FuncPtrInternal` encapsulates a non-null function pointer and provides safe
/// creation and access methods. It's used throughout injectorpp
/// to represent both original functions to be mocked and their replacement
/// implementations.
///
/// # Safety
///
/// The caller must ensure that the pointer is valid and points to a function.
pub(crate) struct FuncPtrInternal(NonNull<()>);

impl FuncPtrInternal {
    /// Creates a new `FuncPtrInternal` from a raw pointer.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer is valid and points to a function.
    pub(crate) unsafe fn new(non_null_ptr: NonNull<()>) -> Self {
        FuncPtrInternal(non_null_ptr)
    }

    /// Returns the raw pointer to the function.
    pub(crate) fn as_ptr(&self) -> *const () {
        self.0.as_ptr()
    }
}

/// Allocates a block of executable memory near the provided source address,
/// ensuring that the allocated memory lies within ±128MB of the source.
/// This mirrors the C++ approach.
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub(crate) fn allocate_jit_memory(src: &FuncPtrInternal, code_size: usize) -> *mut u8 {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        allocate_jit_memory_unix(src, code_size)
    }

    #[cfg(target_os = "windows")]
    {
        allocate_jit_memory_windows(src, code_size)
    }
}

// See https://github.com/microsoft/injectorppforrust/issues/84
// See https://github.com/microsoft/injectorppforrust/issues/88
/// Allocate JIT memory on Unix platforms.
///
/// On MacOS, both aarch64 and x86_64 architectures have a ±2GB memory range.
/// On Linux, both aarch64 and x86_64 architectures have a ±128MB memory range.
/// Other architectures have no enforced address range constraint.
///
/// # Panics
/// Panics if memory allocation fails or if no memory is found within the valid address range on
/// `aarch64` or `x86_64`.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
fn allocate_jit_memory_unix(_src: &FuncPtrInternal, code_size: usize) -> *mut u8 {
    #[cfg(target_os = "macos")]
    let flags = libc::MAP_ANON | libc::MAP_PRIVATE | libc::MAP_JIT;

    #[cfg(target_os = "linux")]
    let flags = libc::MAP_ANONYMOUS | libc::MAP_PRIVATE;

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        #[cfg(target_os = "macos")]
        let max_range: u64 = 0x8000_0000; // ±2GB

        #[cfg(target_os = "linux")]
        let max_range: u64 = 0x8000000; // ±128MB

        let original_addr = _src.as_ptr() as u64;
        let page_size = unsafe { sysconf(_SC_PAGESIZE) as u64 };
        let mut start_address = original_addr.saturating_sub(max_range);

        while start_address <= original_addr + max_range {
            let ptr = unsafe {
                libc::mmap(
                    start_address as *mut c_void,
                    code_size,
                    PROT_READ | PROT_WRITE | PROT_EXEC,
                    flags,
                    -1,
                    0,
                )
            };
            if ptr != libc::MAP_FAILED {
                let allocated = ptr as u64;
                let diff = allocated.abs_diff(original_addr);
                if diff <= max_range {
                    return ptr as *mut u8;
                } else {
                    unsafe { libc::munmap(ptr, code_size) };
                }
            }
            start_address += page_size;
        }

        panic!(
            "Failed to allocate JIT memory within ±{max_range} of source on {} arch",
            std::env::consts::ARCH
        );
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                code_size,
                PROT_READ | PROT_WRITE | PROT_EXEC,
                flags,
                -1,
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            panic!(
                "Failed to allocate executable memory on {} arch",
                std::env::consts::ARCH
            );
        }

        ptr as *mut u8
    }
}
// See https://github.com/microsoft/injectorppforrust/issues/84
/// Allocate executable JIT memory on Windows platforms.
///
/// For AArch64, memory must be within ±128MB due to instruction encoding limits (e.g., B/BL).
/// For x86_64, memory must be within ±2GB for `jmp rel32` instructions.
#[cfg(target_os = "windows")]
fn allocate_jit_memory_windows(_src: &FuncPtrInternal, code_size: usize) -> *mut u8 {
    #[cfg(target_arch = "aarch64")]
    {
        let max_range: u64 = 0x8000000; // ±128MB
        let original_addr = _src.as_ptr() as u64;
        let page_size = unsafe { get_page_size() as u64 };
        let mut start_address = original_addr.saturating_sub(max_range);

        while start_address <= original_addr + max_range {
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
                let diff = allocated.abs_diff(original_addr);
                if diff <= max_range {
                    return ptr as *mut u8;
                } else {
                    unsafe {
                        VirtualFree(ptr, 0, MEM_RELEASE);
                    }
                }
            }
            start_address += page_size;
        }

        panic!("Failed to allocate executable memory within ±128MB of original function address on AArch64 Windows");
    }

    #[cfg(target_arch = "x86_64")]
    {
        let max_range: usize = 0x8000_0000; // ±2GB
        let original_addr = _src.as_ptr() as usize;
        let page_size = unsafe { get_page_size() };
        let mut addr = original_addr.saturating_sub(max_range);

        while addr <= original_addr + max_range {
            let ptr = unsafe {
                VirtualAlloc(
                    addr as *mut c_void,
                    code_size,
                    MEM_COMMIT | MEM_RESERVE,
                    PAGE_EXECUTE_READWRITE,
                )
            };

            if !ptr.is_null() {
                let allocated = ptr as usize;
                if allocated.abs_diff(original_addr) <= max_range {
                    return ptr as *mut u8;
                } else {
                    unsafe {
                        VirtualFree(ptr, 0, MEM_RELEASE);
                    }
                }
            }

            addr += page_size;
        }

        panic!("Failed to allocate executable memory within ±2GB of original function address on x86_64 Windows");
    }

    #[cfg(all(not(target_arch = "x86_64"), not(target_arch = "aarch64")))]
    {
        let ptr = unsafe {
            VirtualAlloc(
                std::ptr::null_mut(), // let OS choose suitable address
                code_size,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            )
        };

        if ptr.is_null() {
            panic!("Failed to allocate executable memory on Windows (unsupported architecture)");
        }

        ptr as *mut u8
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
                #[cfg(any(target_os = "linux", target_os = "macos"))]
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
#[cfg(not(target_os = "macos"))]
pub(crate) unsafe fn patch_function(func: *mut u8, patch: &[u8]) {
    make_memory_writable_and_executable(func);

    inject_asm_code(patch, func);
}

#[cfg(target_os = "macos")]
pub(crate) unsafe fn patch_function(func: *mut u8, patch: &[u8]) {
    use mach2::traps::mach_task_self;
    use mach2::vm::{mach_vm_protect, mach_vm_remap};
    use mach2::vm_inherit::VM_INHERIT_NONE;
    use mach2::vm_prot::VM_PROT_COPY;
    use mach2::vm_statistics::{VM_FLAGS_ANYWHERE, VM_FLAGS_OVERWRITE, VM_FLAGS_RETURN_DATA_ADDR};

    let mut addr = func as mach_vm_address_t;
    let mut remap: mach_vm_address_t = std::mem::zeroed();
    let mut cur: vm_prot_t = std::mem::zeroed();
    let mut max: vm_prot_t = std::mem::zeroed();
    mach_vm_remap(
        mach_task_self(),
        &mut remap,
        patch.len() as u64,
        0,
        VM_FLAGS_ANYWHERE | VM_FLAGS_RETURN_DATA_ADDR,
        mach_task_self(),
        addr,
        0,
        &mut cur,
        &mut max,
        VM_INHERIT_NONE,
    );

    mach_vm_protect(
        mach_task_self(),
        remap,
        0x8,
        0,
        VM_PROT_READ | VM_PROT_WRITE | VM_PROT_COPY,
    );

    inject_asm_code(patch, remap as *mut u8);

    sys_dcache_flush(func, patch.len());

    mach_vm_protect(
        mach_task_self(),
        remap,
        0x8,
        0,
        VM_PROT_READ | VM_PROT_EXECUTE,
    );

    sys_icache_invalidate(func, patch.len());

    mach_vm_remap(
        mach_task_self(),
        &mut addr,
        patch.len() as u64,
        0,
        VM_FLAGS_OVERWRITE | VM_FLAGS_RETURN_DATA_ADDR,
        mach_task_self(),
        remap,
        0,
        &mut cur,
        &mut max,
        VM_INHERIT_NONE,
    );
}

// MacOS forces memory to be writable or executable but not both. So we don't need an
// implementation for it.
#[cfg(not(target_os = "macos"))]
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
    #[cfg(target_os = "macos")]
    pthread_jit_write_protect_np(0);

    ptr::copy_nonoverlapping(asm_code.as_ptr(), dest, asm_code.len());

    #[cfg(target_os = "macos")]
    pthread_jit_write_protect_np(1);

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

    #[cfg(target_os = "macos")]
    {
        // The cache is invalidated in patch_function.
        let _ = start;
        let _ = end;
    }

    // On ARM64, explicitly synchronize the CPU pipeline.
    #[cfg(target_arch = "aarch64")]
    {
        core::arch::asm!("dsb sy", "isb", options(nostack, nomem));
    }
}
