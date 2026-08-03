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
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64", target_arch = "arm"))]
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
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64", target_arch = "arm"))]
fn allocate_jit_memory_unix(_src: &FuncPtrInternal, code_size: usize) -> *mut u8 {
    #[cfg(target_os = "macos")]
    let flags = libc::MAP_ANON | libc::MAP_PRIVATE | libc::MAP_JIT;

    #[cfg(target_os = "linux")]
    let flags = libc::MAP_ANONYMOUS | libc::MAP_PRIVATE;

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "arm"))]
    {
        #[cfg(target_os = "macos")]
        let max_range: u64 = 0x8000_0000; // ±2GB

        #[cfg(all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        let max_range: u64 = 0x8000000; // ±128MB

        #[cfg(all(target_os = "linux", target_arch = "arm"))]
        let max_range: u64 = 0x1000000; // ±16MB

        let original_addr = _src.as_ptr() as u64;
        let page_size = unsafe { sysconf(_SC_PAGESIZE) as u64 };

        // Search outward from the function address to find the CLOSEST free page.
        // This minimizes the trampoline-to-function distance, which is critical for
        // PC-relative instruction fixups (CBZ/CBNZ have only ±1MB range).
        let mut offset: u64 = 0;
        while offset <= max_range {
            // Try addresses at +offset and -offset from the function
            for &dir in &[1i64, -1i64] {
                let hint = if dir > 0 {
                    original_addr.checked_add(offset)
                } else if offset > 0 {
                    original_addr.checked_sub(offset)
                } else {
                    continue; // Already tried offset=0 with dir=1
                };

                let Some(hint_addr) = hint else { continue };

                let ptr = unsafe {
                    libc::mmap(
                        hint_addr as *mut c_void,
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
            }
            offset += page_size;
        }

        panic!(
            "Failed to allocate JIT memory within ±{max_range} of source on {} arch",
            std::env::consts::ARCH
        );
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64", target_arch = "arm")))]
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
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        #[cfg(target_arch = "aarch64")]
        let max_range: u64 = 0x8000000; // ±128MB

        #[cfg(target_arch = "x86_64")]
        let max_range: u64 = 0x8000_0000; // ±2GB

        let original_addr = _src.as_ptr() as u64;
        let page_size = unsafe { get_page_size() as u64 };

        // Search outward from the function address to find the CLOSEST free page.
        // This avoids allocating far from the function (e.g., in/near stack memory),
        // which could disrupt the stack guard page and cause STATUS_STACK_OVERFLOW.
        let mut offset: u64 = 0;
        while offset <= max_range {
            for &dir in &[1i64, -1i64] {
                let hint = if dir > 0 {
                    original_addr.checked_add(offset)
                } else if offset > 0 {
                    original_addr.checked_sub(offset)
                } else {
                    continue; // Already tried offset=0 with dir=1
                };

                let Some(hint_addr) = hint else { continue };

                let ptr = unsafe {
                    VirtualAlloc(
                        hint_addr as *mut c_void,
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
            }
            offset += page_size;
        }

        panic!(
            "Failed to allocate JIT memory within ±{max_range} bytes of source on {} arch",
            std::env::consts::ARCH
        );
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
#[allow(dead_code)]
pub(crate) struct PatchGuard {
    func_ptr: *mut u8,
    original_bytes: Vec<u8>,
    patch_size: usize,
    jit_memory: *mut u8,

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    jit_size: usize,
}

#[allow(dead_code)]
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

#[cfg(test)]
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
mod tests {
    use super::*;

    /// A dummy function used as the "source" address for JIT allocation tests.
    #[inline(never)]
    fn dummy_target_function() -> i32 {
        std::hint::black_box(42)
    }

    /// Verify that `allocate_jit_memory` returns an address close to the source function,
    /// not at the far end of the ±2GB range where it could collide with the stack.
    ///
    /// The old x86_64 Windows implementation scanned linearly from `func_addr - 2GB`,
    /// which could return memory near the stack guard pages. The fixed implementation
    /// searches outward from the function address, so the result should be much closer.
    #[test]
    fn test_jit_allocation_is_close_to_source() {
        let func_ptr = unsafe {
            FuncPtrInternal::new(std::ptr::NonNull::new(dummy_target_function as *mut ()).unwrap())
        };
        let func_addr = func_ptr.as_ptr() as u64;

        let jit_ptr = allocate_jit_memory(&func_ptr, 256);
        assert!(!jit_ptr.is_null(), "JIT allocation should succeed");

        let jit_addr = jit_ptr as u64;
        let distance = func_addr.abs_diff(jit_addr);

        // The allocation should be relatively close — within 128MB.
        // The old buggy code would often return addresses ~2GB away, near the stack.
        let max_acceptable_distance: u64 = 128 * 1024 * 1024; // 128MB
        assert!(
            distance <= max_acceptable_distance,
            "JIT memory should be allocated close to the function. \
             Function at {func_addr:#x}, JIT at {jit_addr:#x}, distance: {distance} bytes ({} MB). \
             Expected within {max_acceptable_distance} bytes ({} MB).",
            distance / (1024 * 1024),
            max_acceptable_distance / (1024 * 1024),
        );

        // Clean up
        unsafe {
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            {
                libc::munmap(jit_ptr as *mut c_void, 256);
            }
            #[cfg(target_os = "windows")]
            {
                VirtualFree(jit_ptr as *mut c_void, 0, MEM_RELEASE);
            }
        }
    }

    /// Verify that JIT allocation does NOT land in the current thread's stack region.
    /// This directly tests the root cause of the STATUS_STACK_OVERFLOW crash: the old
    /// algorithm could allocate JIT memory in/near the stack, disrupting the guard page.
    #[test]
    fn test_jit_allocation_not_in_stack_region() {
        let func_ptr = unsafe {
            FuncPtrInternal::new(std::ptr::NonNull::new(dummy_target_function as *mut ()).unwrap())
        };

        // Use a stack local's address to approximate the stack location
        let stack_local: u64 = 0;
        let stack_addr = &stack_local as *const u64 as u64;

        let jit_ptr = allocate_jit_memory(&func_ptr, 256);
        assert!(!jit_ptr.is_null(), "JIT allocation should succeed");

        let jit_addr = jit_ptr as u64;
        // Stack on Windows x86_64 is typically 1-8MB. Use a conservative 16MB guard zone.
        let stack_guard_zone: u64 = 16 * 1024 * 1024;
        let distance_to_stack = jit_addr.abs_diff(stack_addr);

        assert!(
            distance_to_stack > stack_guard_zone,
            "JIT memory should NOT be near the stack! \
             JIT at {jit_addr:#x}, stack approx at {stack_addr:#x}, \
             distance: {distance_to_stack} bytes ({} MB). \
             Must be > {stack_guard_zone} bytes ({} MB) from stack.",
            distance_to_stack / (1024 * 1024),
            stack_guard_zone / (1024 * 1024),
        );

        // Clean up
        unsafe {
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            {
                libc::munmap(jit_ptr as *mut c_void, 256);
            }
            #[cfg(target_os = "windows")]
            {
                VirtualFree(jit_ptr as *mut c_void, 0, MEM_RELEASE);
            }
        }
    }
}
