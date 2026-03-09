#![cfg(target_arch = "x86_64")]

use std::cell::Cell;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::Mutex;

use crate::injector_core::common::*;

#[cfg(target_os = "linux")]
use crate::injector_core::linuxapi::__clear_cache;

#[cfg(target_os = "windows")]
use crate::injector_core::winapi::*;

thread_local! {
    static THREAD_REPLACEMENTS: UnsafeCell<HashMap<usize, usize>> = UnsafeCell::new(HashMap::new());
    // Reentrancy guard: prevents infinite recursion when a patched function
    // (like memset) is called internally during our HashMap operations.
    static IN_TLS_OP: Cell<bool> = const { Cell::new(false) };
}

/// Read from thread-local replacements map with reentrancy protection.
/// Returns `default` if called reentrantly (e.g. a patched function like memset
/// is called during a HashMap operation).
fn tls_get(key: &usize, default: usize) -> usize {
    IN_TLS_OP
        .try_with(|flag| {
            if flag.get() {
                return default;
            }
            flag.set(true);
            let result = THREAD_REPLACEMENTS
                .try_with(|map| unsafe { (*map.get()).get(key).copied().unwrap_or(default) })
                .unwrap_or(default);
            flag.set(false);
            result
        })
        .unwrap_or(default)
}

/// Insert into thread-local replacements map with reentrancy protection.
fn tls_insert(key: usize, value: usize) {
    let _ = IN_TLS_OP.try_with(|flag| {
        if flag.get() {
            return;
        }
        flag.set(true);
        let _ = THREAD_REPLACEMENTS.try_with(|map| unsafe {
            (*map.get()).insert(key, value);
        });
        flag.set(false);
    });
}

/// Remove from thread-local replacements map with reentrancy protection.
fn tls_remove(key: &usize) {
    let _ = IN_TLS_OP.try_with(|flag| {
        if flag.get() {
            return;
        }
        flag.set(true);
        let _ = THREAD_REPLACEMENTS.try_with(|map| unsafe {
            (*map.get()).remove(key);
        });
        flag.set(false);
    });
}

struct MethodEntry {
    trampoline: *mut u8,
    trampoline_size: usize,
    dispatcher_jit: *mut u8,
    dispatcher_jit_size: usize,
    original_bytes: Vec<u8>,
    func_ptr: *mut u8,
    patch_size: usize,
    ref_count: usize,
}

// Safety: MethodEntry contains raw pointers that are only accessed while holding the REGISTRY lock
unsafe impl Send for MethodEntry {}

static REGISTRY: std::sync::LazyLock<Mutex<HashMap<usize, MethodEntry>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// A registration handle for a thread-local function replacement.
/// When dropped, it unregisters the replacement and potentially restores the original function.
pub(crate) struct ThreadRegistration {
    method_key: usize,
    extra_jit: Option<(*mut u8, usize)>,
}

// Safety: ThreadRegistration is intentionally !Send because it's tied to the creating thread's
// thread-local storage. The raw pointers prevent auto-Send, which is what we want.

impl Drop for ThreadRegistration {
    fn drop(&mut self) {
        // Remove this thread's replacement from thread-local storage
        tls_remove(&self.method_key);

        // Free extra JIT block (e.g., return-boolean code) if any
        if let Some((ptr, _size)) = self.extra_jit {
            unsafe {
                #[cfg(any(target_os = "linux", target_os = "macos"))]
                {
                    libc::munmap(ptr as *mut libc::c_void, _size);
                }
                #[cfg(target_os = "windows")]
                {
                    VirtualFree(ptr as *mut libc::c_void, 0, MEM_RELEASE);
                }
            }
        }

        // Decrement ref_count in global registry
        let mut registry = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = registry.get_mut(&self.method_key) {
            entry.ref_count = entry.ref_count.saturating_sub(1);
            if entry.ref_count == 0 {
                // Last reference removed — restore original function and clean up
                unsafe {
                    patch_function(entry.func_ptr, &entry.original_bytes[..entry.patch_size]);
                    clear_cache_ptr(entry.func_ptr, entry.patch_size);

                    free_jit_block(entry.trampoline, entry.trampoline_size);
                    free_jit_block(entry.dispatcher_jit, entry.dispatcher_jit_size);
                }

                registry.remove(&self.method_key);
            }
        }
    }
}

/// Called by the JIT dispatcher to get the target function pointer for the current thread.
///
/// If the current thread has a registered replacement for `method_key`, returns that.
/// Otherwise, returns `default_target` (the trampoline to the original function).
///
/// # Safety
/// This function is called from JIT-generated code. It must not panic across the FFI boundary.
pub(crate) extern "C" fn get_thread_target(method_key: usize, default_target: usize) -> usize {
    match std::panic::catch_unwind(AssertUnwindSafe(|| {
        tls_get(&method_key, default_target)
    })) {
        Ok(target) => target,
        Err(_) => default_target,
    }
}

/// Register a thread-local replacement for a function.
///
/// If this is the first replacement for this function, installs the dispatcher infrastructure
/// (dispatcher JIT + trampoline) and patches the original function.
///
/// Returns a `ThreadRegistration` that cleans up on drop.
pub(crate) fn register_replacement(
    func_ptr: &FuncPtrInternal,
    replacement_addr: usize,
    extra_jit: Option<(*mut u8, usize)>,
) -> ThreadRegistration {
    // Resolve import thunks (jmp [rip+disp]) to the actual function address.
    // This is critical on Windows where extern functions go through an IAT thunk.
    let raw_addr = func_ptr.as_ptr() as *mut u8;
    let func_addr = unsafe { resolve_function_address(raw_addr) };
    let method_key = func_addr as usize;

    {
        let mut registry = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());

        let entry = registry
            .entry(method_key)
            .or_insert_with(|| install_dispatcher(func_addr, method_key));

        entry.ref_count += 1;
    }

    // Set thread-local replacement
    tls_insert(method_key, replacement_addr);

    ThreadRegistration {
        method_key,
        extra_jit,
    }
}

/// Resolve import thunks to the actual function address.
///
/// On Windows, extern "C" functions often go through an import address table (IAT) thunk:
/// `jmp [rip+disp32]` (FF 25 xx xx xx xx). This reads the target from the IAT and returns
/// the real function address. For non-thunk functions, returns the address unchanged.
///
/// This is important because the trampoline copies the original function bytes, and
/// RIP-relative instructions would resolve to wrong addresses in the trampoline.
unsafe fn resolve_function_address(func_addr: *mut u8) -> *mut u8 {
    let code = std::slice::from_raw_parts(func_addr, 6);
    if code[0] == 0xFF && code[1] == 0x25 {
        // jmp [rip+disp32]: target address is at *(rip_after_insn + disp32)
        let disp = i32::from_le_bytes([code[2], code[3], code[4], code[5]]);
        let rip_after_insn = func_addr.add(6);
        let iat_entry = rip_after_insn.offset(disp as isize) as *const *mut u8;
        let real_addr = std::ptr::read(iat_entry);
        // Recursively resolve in case of chained thunks
        return resolve_function_address(real_addr);
    }
    func_addr
}

/// Install the dispatcher infrastructure for a function:
/// 1. Create a trampoline (original bytes + jump back)
/// 2. Generate the dispatcher JIT code
/// 3. Patch the original function to jump to the dispatcher
fn install_dispatcher(func_addr: *mut u8, method_key: usize) -> MethodEntry {
    // Step 1: Create trampoline from original function bytes
    let (trampoline, trampoline_size, copy_size) = create_trampoline(func_addr, method_key);

    let trampoline_addr = trampoline as usize;

    // Step 2: Generate dispatcher JIT code
    let (dispatcher, dispatcher_size) =
        generate_dispatcher_jit(method_key, trampoline_addr, func_addr);

    // Step 3: Generate branch from original function to dispatcher
    let dispatcher_addr = dispatcher as usize;
    let func_addr_usize = func_addr as usize;
    let branch_code = generate_branch_to_dispatcher(func_addr_usize, dispatcher_addr);
    let patch_size = branch_code.len();

    // Read original bytes (at least patch_size, but we already read copy_size for trampoline)
    let save_size = patch_size.max(copy_size);
    let original_bytes = unsafe { read_bytes(func_addr, save_size) };

    // Step 4: Patch the original function
    unsafe {
        patch_function(func_addr, &branch_code);
    }

    MethodEntry {
        trampoline,
        trampoline_size,
        dispatcher_jit: dispatcher,
        dispatcher_jit_size: dispatcher_size,
        original_bytes,
        func_ptr: func_addr,
        patch_size,
        ref_count: 0,
    }
}

// ============================================================================
// x86_64 Dispatcher JIT Code Generation
// ============================================================================

/// Generate a branch instruction from `from` to `to`.
fn generate_branch_to_dispatcher(from: usize, to: usize) -> Vec<u8> {
    let offset = to as isize - (from as isize + 5);
    if offset >= i32::MIN as isize && offset <= i32::MAX as isize {
        // rel32 jump (5 bytes)
        let mut code = Vec::with_capacity(5);
        code.push(0xE9); // JMP rel32
        code.extend_from_slice(&(offset as i32).to_le_bytes());
        code
    } else {
        // movabs rax, imm64; jmp rax (12 bytes)
        let mut code = Vec::with_capacity(12);
        code.extend_from_slice(&[0x48, 0xB8]); // MOV RAX, imm64
        code.extend_from_slice(&(to as u64).to_le_bytes());
        code.extend_from_slice(&[0xFF, 0xE0]); // JMP RAX
        code
    }
}

/// Generate the dispatcher JIT code for x86_64.
///
/// The dispatcher:
/// 1. Saves all argument registers (integer + xmm)
/// 2. Calls `get_thread_target(method_key, trampoline_addr)` to get the target
/// 3. Restores all argument registers
/// 4. Jumps to the returned target
fn generate_dispatcher_jit(
    method_key: usize,
    trampoline_addr: usize,
    near_addr: *mut u8,
) -> (*mut u8, usize) {
    let fn_addr = get_thread_target as *const () as usize;

    #[cfg(target_os = "windows")]
    let code = generate_dispatcher_windows(method_key, trampoline_addr, fn_addr);

    #[cfg(not(target_os = "windows"))]
    let code = generate_dispatcher_sysv(method_key, trampoline_addr, fn_addr);

    let near_src = unsafe { FuncPtrInternal::new(std::ptr::NonNull::new(near_addr as *mut ()).unwrap()) };
    let jit_size = code.len();
    let jit_mem = allocate_jit_memory(&near_src, jit_size);

    unsafe {
        inject_asm_code(&code, jit_mem);
    }

    (jit_mem, jit_size)
}

/// Windows x64 calling convention dispatcher.
/// Integer args: rcx, rdx, r8, r9. Float args: xmm0-xmm3.
#[cfg(target_os = "windows")]
fn generate_dispatcher_windows(
    method_key: usize,
    trampoline_addr: usize,
    fn_addr: usize,
) -> Vec<u8> {
    let mut code: Vec<u8> = Vec::with_capacity(128);

    // Save integer argument registers
    code.extend_from_slice(&[0x41, 0x51]); // push r9
    code.extend_from_slice(&[0x41, 0x50]); // push r8
    code.push(0x52); // push rdx
    code.push(0x51); // push rcx

    // Allocate space: 64 (xmm0-3) + 32 (shadow) + 8 (alignment) = 104 = 0x68
    code.extend_from_slice(&[0x48, 0x83, 0xEC, 0x68]); // sub rsp, 0x68

    // Save xmm argument registers (above shadow space)
    code.extend_from_slice(&[0x0F, 0x29, 0x44, 0x24, 0x20]); // movaps [rsp+0x20], xmm0
    code.extend_from_slice(&[0x0F, 0x29, 0x4C, 0x24, 0x30]); // movaps [rsp+0x30], xmm1
    code.extend_from_slice(&[0x0F, 0x29, 0x54, 0x24, 0x40]); // movaps [rsp+0x40], xmm2
    code.extend_from_slice(&[0x0F, 0x29, 0x5C, 0x24, 0x50]); // movaps [rsp+0x50], xmm3

    // mov rcx, method_key (first arg)
    code.extend_from_slice(&[0x48, 0xB9]);
    code.extend_from_slice(&(method_key as u64).to_le_bytes());

    // mov rdx, trampoline_addr (second arg = default target)
    code.extend_from_slice(&[0x48, 0xBA]);
    code.extend_from_slice(&(trampoline_addr as u64).to_le_bytes());

    // mov rax, fn_addr
    code.extend_from_slice(&[0x48, 0xB8]);
    code.extend_from_slice(&(fn_addr as u64).to_le_bytes());

    // call rax
    code.extend_from_slice(&[0xFF, 0xD0]);

    // mov r10, rax (save result)
    code.extend_from_slice(&[0x49, 0x89, 0xC2]);

    // Restore xmm argument registers
    code.extend_from_slice(&[0x0F, 0x28, 0x44, 0x24, 0x20]); // movaps xmm0, [rsp+0x20]
    code.extend_from_slice(&[0x0F, 0x28, 0x4C, 0x24, 0x30]); // movaps xmm1, [rsp+0x30]
    code.extend_from_slice(&[0x0F, 0x28, 0x54, 0x24, 0x40]); // movaps xmm2, [rsp+0x40]
    code.extend_from_slice(&[0x0F, 0x28, 0x5C, 0x24, 0x50]); // movaps xmm3, [rsp+0x50]

    // Deallocate
    code.extend_from_slice(&[0x48, 0x83, 0xC4, 0x68]); // add rsp, 0x68

    // Restore integer argument registers
    code.push(0x59); // pop rcx
    code.push(0x5A); // pop rdx
    code.extend_from_slice(&[0x41, 0x58]); // pop r8
    code.extend_from_slice(&[0x41, 0x59]); // pop r9

    // Jump to target
    code.extend_from_slice(&[0x41, 0xFF, 0xE2]); // jmp r10

    code
}

/// System V AMD64 ABI dispatcher (Linux, macOS).
/// Integer args: rdi, rsi, rdx, rcx, r8, r9. Float args: xmm0-xmm7.
#[cfg(not(target_os = "windows"))]
fn generate_dispatcher_sysv(
    method_key: usize,
    trampoline_addr: usize,
    fn_addr: usize,
) -> Vec<u8> {
    let mut code: Vec<u8> = Vec::with_capacity(200);

    // Save integer argument registers (6 registers)
    code.extend_from_slice(&[0x41, 0x51]); // push r9
    code.extend_from_slice(&[0x41, 0x50]); // push r8
    code.push(0x51); // push rcx
    code.push(0x52); // push rdx
    code.push(0x56); // push rsi
    code.push(0x57); // push rdi

    // Allocate space: 128 (xmm0-7) + 8 (alignment) = 136 = 0x88
    // 0x88 > 0x7F so needs imm32 encoding
    code.extend_from_slice(&[0x48, 0x81, 0xEC, 0x88, 0x00, 0x00, 0x00]); // sub rsp, 0x88

    // Save xmm0-7
    code.extend_from_slice(&[0x0F, 0x29, 0x04, 0x24]); // movaps [rsp], xmm0
    code.extend_from_slice(&[0x0F, 0x29, 0x4C, 0x24, 0x10]); // movaps [rsp+0x10], xmm1
    code.extend_from_slice(&[0x0F, 0x29, 0x54, 0x24, 0x20]); // movaps [rsp+0x20], xmm2
    code.extend_from_slice(&[0x0F, 0x29, 0x5C, 0x24, 0x30]); // movaps [rsp+0x30], xmm3
    code.extend_from_slice(&[0x0F, 0x29, 0x64, 0x24, 0x40]); // movaps [rsp+0x40], xmm4
    code.extend_from_slice(&[0x0F, 0x29, 0x6C, 0x24, 0x50]); // movaps [rsp+0x50], xmm5
    code.extend_from_slice(&[0x0F, 0x29, 0x74, 0x24, 0x60]); // movaps [rsp+0x60], xmm6
    code.extend_from_slice(&[0x0F, 0x29, 0x7C, 0x24, 0x70]); // movaps [rsp+0x70], xmm7

    // mov rdi, method_key (first arg)
    code.extend_from_slice(&[0x48, 0xBF]);
    code.extend_from_slice(&(method_key as u64).to_le_bytes());

    // mov rsi, trampoline_addr (second arg = default target)
    code.extend_from_slice(&[0x48, 0xBE]);
    code.extend_from_slice(&(trampoline_addr as u64).to_le_bytes());

    // mov rax, fn_addr
    code.extend_from_slice(&[0x48, 0xB8]);
    code.extend_from_slice(&(fn_addr as u64).to_le_bytes());

    // call rax
    code.extend_from_slice(&[0xFF, 0xD0]);

    // mov r10, rax (save result)
    code.extend_from_slice(&[0x49, 0x89, 0xC2]);

    // Restore xmm0-7
    code.extend_from_slice(&[0x0F, 0x28, 0x04, 0x24]); // movaps xmm0, [rsp]
    code.extend_from_slice(&[0x0F, 0x28, 0x4C, 0x24, 0x10]); // movaps xmm1, [rsp+0x10]
    code.extend_from_slice(&[0x0F, 0x28, 0x54, 0x24, 0x20]); // movaps xmm2, [rsp+0x20]
    code.extend_from_slice(&[0x0F, 0x28, 0x5C, 0x24, 0x30]); // movaps xmm3, [rsp+0x30]
    code.extend_from_slice(&[0x0F, 0x28, 0x64, 0x24, 0x40]); // movaps xmm4, [rsp+0x40]
    code.extend_from_slice(&[0x0F, 0x28, 0x6C, 0x24, 0x50]); // movaps xmm5, [rsp+0x50]
    code.extend_from_slice(&[0x0F, 0x28, 0x74, 0x24, 0x60]); // movaps xmm6, [rsp+0x60]
    code.extend_from_slice(&[0x0F, 0x28, 0x7C, 0x24, 0x70]); // movaps xmm7, [rsp+0x70]

    // Deallocate
    code.extend_from_slice(&[0x48, 0x81, 0xC4, 0x88, 0x00, 0x00, 0x00]); // add rsp, 0x88

    // Restore integer argument registers
    code.push(0x5F); // pop rdi
    code.push(0x5E); // pop rsi
    code.push(0x5A); // pop rdx
    code.push(0x59); // pop rcx
    code.extend_from_slice(&[0x41, 0x58]); // pop r8
    code.extend_from_slice(&[0x41, 0x59]); // pop r9

    // Jump to target
    code.extend_from_slice(&[0x41, 0xFF, 0xE2]); // jmp r10

    code
}

// ============================================================================
// Trampoline: copies original function bytes + appends jump back
// ============================================================================

/// Create a trampoline for the original function.
///
/// The trampoline contains the original function's first N bytes (instruction-aligned,
/// with RIP-relative displacements adjusted) followed by a jump back to (original_func + N).
///
/// Returns (trampoline_ptr, trampoline_alloc_size, bytes_copied_from_original).
fn create_trampoline(func_addr: *mut u8, _method_key: usize) -> (*mut u8, usize, usize) {
    // We need to copy enough bytes to cover the patch that will be applied.
    // The patch is a jmp to the dispatcher (5 bytes for rel32, 12 for movabs+jmp).
    // Since dispatcher JIT is allocated nearby, the patch is typically 5 bytes.
    // Use 12 as the minimum to be safe.
    let min_copy = 12;

    // Read enough bytes from the original function to decode instructions
    let read_size = min_copy + 16; // extra space for the last instruction
    let original_code = unsafe { read_bytes(func_addr, read_size) };

    // Find instruction-aligned boundary >= min_copy
    let copy_size = find_instruction_boundary(&original_code, min_copy);

    // The jump-back uses jmp [rip+0] + 8-byte address = 14 bytes
    let jump_back_size = 14;
    let trampoline_total = copy_size + jump_back_size;

    // Allocate executable memory for the trampoline (near original for ±2GB reach)
    let near_src =
        unsafe { FuncPtrInternal::new(std::ptr::NonNull::new(func_addr as *mut ()).unwrap()) };
    let trampoline = allocate_jit_memory(&near_src, trampoline_total);

    // Copy original instruction bytes
    unsafe {
        std::ptr::copy_nonoverlapping(func_addr, trampoline, copy_size);
    }

    // Fix up RIP-relative displacements in the copied instructions.
    // When instructions use [rip+disp32], the displacement is relative to the
    // instruction's position. After copying to the trampoline at a different address,
    // we must adjust disp32 so it still points to the same absolute target.
    let delta = func_addr as isize - trampoline as isize;
    fixup_rip_relative_instructions(trampoline, &original_code, copy_size, delta);

    // Append jump back to original + copy_size
    // Using: jmp [rip+0] (FF 25 00 00 00 00) + 8-byte target address
    let jump_back_addr = (func_addr as usize + copy_size) as u64;
    let jump_back_offset = copy_size;

    unsafe {
        let jmp_ptr = trampoline.add(jump_back_offset);
        // FF 25 00 00 00 00 = jmp [rip+0]
        *jmp_ptr = 0xFF;
        *jmp_ptr.add(1) = 0x25;
        *jmp_ptr.add(2) = 0x00;
        *jmp_ptr.add(3) = 0x00;
        *jmp_ptr.add(4) = 0x00;
        *jmp_ptr.add(5) = 0x00;
        // 8-byte absolute target address
        std::ptr::copy_nonoverlapping(
            jump_back_addr.to_le_bytes().as_ptr(),
            jmp_ptr.add(6),
            8,
        );

        // Flush instruction cache for the trampoline
        clear_cache_ptr(trampoline, trampoline_total);
    }

    (trampoline, trampoline_total, copy_size)
}

/// Adjust RIP-relative displacements in trampoline instructions so they
/// point to the same absolute targets as the original instructions.
///
/// `delta` = original_addr - trampoline_addr (add to disp32 to correct it).
///
/// If the adjusted displacement overflows i32, the instruction is NOP-ed out.
/// This happens when coverage instrumentation inserts `lock inc [rip+disp32]`
/// and the coverage counter is too far from the trampoline for a 32-bit
/// displacement. NOP-ing the counter increment is safe — it only affects
/// profiling accuracy, not functional behavior.
fn fixup_rip_relative_instructions(
    trampoline: *mut u8,
    original_code: &[u8],
    copy_size: usize,
    delta: isize,
) {
    let mut offset = 0;
    while offset < copy_size {
        let insn = &original_code[offset..];
        let insn_len = x86_64_insn_len(insn);
        if insn_len == 0 {
            break;
        }

        // Check for ModR/M-based RIP-relative addressing (mod=00, rm=101)
        if let Some(disp_offset) = find_rip_relative_disp_offset(insn, insn_len) {
            unsafe {
                let disp_ptr = trampoline.add(offset + disp_offset) as *mut i32;
                let old_disp = disp_ptr.read_unaligned();
                let new_disp = old_disp as i64 + delta as i64;
                if new_disp >= i32::MIN as i64 && new_disp <= i32::MAX as i64 {
                    disp_ptr.write_unaligned(new_disp as i32);
                } else {
                    // Overflow: NOP out the entire instruction in the trampoline
                    for i in 0..insn_len {
                        *trampoline.add(offset + i) = 0x90; // NOP
                    }
                }
            }
        }

        // Check for relative call/jmp (E8/E9 rel32)
        let opcode_pos = skip_prefixes(insn);
        if opcode_pos < insn.len() {
            let opcode = insn[opcode_pos];
            if opcode == 0xE8 || opcode == 0xE9 {
                let rel_offset = opcode_pos + 1;
                unsafe {
                    let rel_ptr = trampoline.add(offset + rel_offset) as *mut i32;
                    let old_rel = rel_ptr.read_unaligned();
                    let new_rel = old_rel as i64 + delta as i64;
                    if new_rel >= i32::MIN as i64 && new_rel <= i32::MAX as i64 {
                        rel_ptr.write_unaligned(new_rel as i32);
                    } else {
                        // Overflow: NOP out the entire instruction
                        for i in 0..insn_len {
                            *trampoline.add(offset + i) = 0x90;
                        }
                    }
                }
            }
            // Check for 0F 8x (Jcc rel32) two-byte opcode
            if opcode == 0x0F && opcode_pos + 1 < insn.len() {
                let op2 = insn[opcode_pos + 1];
                if (0x80..=0x8F).contains(&op2) {
                    let rel_offset = opcode_pos + 2;
                    unsafe {
                        let rel_ptr = trampoline.add(offset + rel_offset) as *mut i32;
                        let old_rel = rel_ptr.read_unaligned();
                        let new_rel = old_rel as i64 + delta as i64;
                        if new_rel >= i32::MIN as i64 && new_rel <= i32::MAX as i64 {
                            rel_ptr.write_unaligned(new_rel as i32);
                        } else {
                            for i in 0..insn_len {
                                *trampoline.add(offset + i) = 0x90;
                            }
                        }
                    }
                }
            }
        }

        offset += insn_len;
    }
}

/// Find the byte offset of the disp32 field in a RIP-relative instruction.
/// Returns None if the instruction doesn't use RIP-relative addressing.
fn find_rip_relative_disp_offset(insn: &[u8], _insn_len: usize) -> Option<usize> {
    let mut pos = skip_prefixes(insn);
    if pos >= insn.len() {
        return None;
    }

    let opcode = insn[pos];
    pos += 1;

    // Two-byte opcode
    if opcode == 0x0F {
        if pos >= insn.len() {
            return None;
        }
        let op2 = insn[pos];
        pos += 1;
        // Jcc rel32 (0F 80-8F) don't use ModR/M RIP-relative, skip
        if (0x80..=0x8F).contains(&op2) {
            return None;
        }
        // Most other 0F xx opcodes have a ModR/M byte — fall through to check
    } else {
        // Single-byte opcodes: check if they have a ModR/M byte
        match opcode {
            // Opcodes that do NOT have ModR/M — skip
            0x50..=0x5F | 0x90 | 0xC3 | 0xCC | 0xCB | 0xC9 | 0xF4 | 0xF5 | 0xF8 | 0xF9
            | 0xFC | 0xFD | 0x99 | 0x9E | 0x9F => return None,
            0x6A | 0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C | 0xCD | 0xEB
            | 0xA8 => return None,
            0x70..=0x7F => return None, // Jcc rel8
            0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D | 0x68 | 0xA9 => {
                return None
            }
            0xE8 | 0xE9 | 0xE3 => return None, // call/jmp rel32, JRCXZ
            0xA0..=0xA3 => return None,         // MOV AL/AX moffs
            0xB0..=0xBF => return None,         // MOV reg, imm
            0xC2 => return None,                // RET imm16
            _ => {
                // Assume has ModR/M — fall through
            }
        }
    }

    // pos now points to the ModR/M byte
    if pos >= insn.len() {
        return None;
    }

    let modrm = insn[pos];
    let mod_field = (modrm >> 6) & 3;
    let rm_field = modrm & 7;

    if mod_field == 0b00 && rm_field == 0b101 {
        // RIP-relative: disp32 starts right after the ModR/M byte
        Some(pos + 1)
    } else {
        None
    }
}

/// Skip legacy prefixes and REX prefix, return the position of the opcode byte.
fn skip_prefixes(code: &[u8]) -> usize {
    let mut pos = 0;
    // Skip legacy prefixes
    while pos < code.len() {
        match code[pos] {
            0x66 | 0x67 | 0xF0 | 0xF2 | 0xF3 | 0x26 | 0x2E | 0x36 | 0x3E | 0x64 | 0x65 => {
                pos += 1
            }
            _ => break,
        }
    }
    // Skip REX prefix
    if pos < code.len() && (code[pos] & 0xF0) == 0x40 {
        pos += 1;
    }
    pos
}

// ============================================================================
// Minimal x86_64 instruction length decoder
// ============================================================================

/// Find the first instruction boundary at or after `min_bytes`.
fn find_instruction_boundary(code: &[u8], min_bytes: usize) -> usize {
    let mut offset = 0;
    while offset < min_bytes && offset < code.len() {
        let len = x86_64_insn_len(&code[offset..]);
        if len == 0 {
            // Can't decode — fall back to min_bytes
            return min_bytes;
        }
        offset += len;
    }
    offset
}

/// Returns the byte-length of the x86_64 instruction starting at `code[0]`.
/// Returns 0 if the instruction cannot be decoded.
fn x86_64_insn_len(code: &[u8]) -> usize {
    if code.is_empty() {
        return 0;
    }

    let mut pos = 0;

    // Skip legacy prefixes
    while pos < code.len() {
        match code[pos] {
            0x66 | 0x67 | 0xF0 | 0xF2 | 0xF3 | 0x26 | 0x2E | 0x36 | 0x3E | 0x64 | 0x65 => {
                pos += 1
            }
            _ => break,
        }
    }

    if pos >= code.len() {
        return 0;
    }

    // Check for REX prefix (0x40-0x4F)
    let has_rex_w = if (code[pos] & 0xF0) == 0x40 {
        let rex = code[pos];
        pos += 1;
        (rex & 0x08) != 0
    } else {
        false
    };

    if pos >= code.len() {
        return 0;
    }

    let opcode = code[pos];
    pos += 1;

    match opcode {
        // Single byte, no operands
        0x50..=0x5F | 0x90 | 0xC3 | 0xCC | 0x99 | 0x9E | 0x9F | 0xCB | 0xF4 | 0xF5 | 0xF8
        | 0xF9 | 0xFC | 0xFD => pos,

        // imm8 operand
        0x6A | 0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C | 0xCD | 0xEB | 0xA8 => {
            pos + 1
        }
        0x70..=0x7F => pos + 1, // Jcc rel8

        // imm32 operand
        0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D | 0x68 | 0xA9 => pos + 4,
        0xE8 | 0xE9 => pos + 4, // call/jmp rel32

        // Short jump
        0xE3 => pos + 1, // JRCXZ rel8

        // MOV AL/AX/EAX/RAX, moffs
        0xA0 | 0xA1 => pos + if has_rex_w { 8 } else { 4 },
        0xA2 | 0xA3 => pos + if has_rex_w { 8 } else { 4 },

        // MOV r8, imm8
        0xB0..=0xB7 => pos + 1,

        // MOV r32/r64, imm32/imm64
        0xB8..=0xBF => {
            if has_rex_w {
                pos + 8
            } else {
                pos + 4
            }
        }

        // Two-byte opcodes (0x0F prefix)
        0x0F => {
            if pos >= code.len() {
                return 0;
            }
            let op2 = code[pos];
            pos += 1;
            match op2 {
                // NOP/ENDBR with ModR/M
                0x1E | 0x1F => pos + modrm_len(&code[pos..]),
                // Jcc rel32
                0x80..=0x8F => pos + 4,
                // SETcc — ModR/M
                0x90..=0x9F => pos + modrm_len(&code[pos..]),
                // MOVZX, MOVSX with ModR/M
                0xB6 | 0xB7 | 0xBE | 0xBF => pos + modrm_len(&code[pos..]),
                // CMOVcc with ModR/M
                0x40..=0x4F => pos + modrm_len(&code[pos..]),
                // MOVAPS/MOVUPS/MOVAPD/MOVUPD
                0x10 | 0x11 | 0x28 | 0x29 => pos + modrm_len(&code[pos..]),
                // XORPS/ANDPS/ORPS etc
                0x54..=0x59 => pos + modrm_len(&code[pos..]),
                // Other 0F opcodes with ModR/M (best effort)
                _ => pos + modrm_len(&code[pos..]),
            }
        }

        // Opcodes with ModR/M, no immediate
        0x00..=0x03
        | 0x08..=0x0B
        | 0x10..=0x13
        | 0x18..=0x1B
        | 0x20..=0x23
        | 0x28..=0x2B
        | 0x30..=0x33
        | 0x38..=0x3B
        | 0x62
        | 0x63
        | 0x84..=0x8B
        | 0x8D
        | 0x8E
        | 0x8F => pos + modrm_len(&code[pos..]),

        // ALU r/m, imm8
        0x80 | 0x82 | 0x83 => {
            if pos < code.len() {
                pos + modrm_len(&code[pos..]) + 1
            } else {
                0
            }
        }

        // ALU r/m, imm32
        0x81 => {
            if pos < code.len() {
                pos + modrm_len(&code[pos..]) + 4
            } else {
                0
            }
        }

        // MOV r/m8, imm8
        0xC6 => {
            if pos < code.len() {
                pos + modrm_len(&code[pos..]) + 1
            } else {
                0
            }
        }

        // MOV r/m32, imm32
        0xC7 => {
            if pos < code.len() {
                pos + modrm_len(&code[pos..]) + 4
            } else {
                0
            }
        }

        // TEST r/m, imm (F6/F7 with reg field 0 or 1)
        0xF6 => {
            if pos < code.len() {
                let reg_field = (code[pos] >> 3) & 7;
                let ml = modrm_len(&code[pos..]);
                if reg_field < 2 {
                    pos + ml + 1
                } else {
                    pos + ml
                }
            } else {
                0
            }
        }
        0xF7 => {
            if pos < code.len() {
                let reg_field = (code[pos] >> 3) & 7;
                let ml = modrm_len(&code[pos..]);
                if reg_field < 2 {
                    pos + ml + 4
                } else {
                    pos + ml
                }
            } else {
                0
            }
        }

        // SHIFT/ROT with implicit 1 or CL
        0xD0..=0xD3 => {
            if pos < code.len() {
                pos + modrm_len(&code[pos..])
            } else {
                0
            }
        }

        // SHIFT/ROT with imm8
        0xC0 | 0xC1 => {
            if pos < code.len() {
                pos + modrm_len(&code[pos..]) + 1
            } else {
                0
            }
        }

        // INC/DEC/CALL/JMP/PUSH with ModR/M
        0xFE | 0xFF => {
            if pos < code.len() {
                pos + modrm_len(&code[pos..])
            } else {
                0
            }
        }

        // LEAVE, RET imm16, INT3 already covered
        0xC9 => pos,
        0xC2 => pos + 2, // RET imm16

        // Unknown opcode — can't decode
        _ => 0,
    }
}

/// Decode the byte-length contribution of a ModR/M byte (including SIB and displacement).
fn modrm_len(code: &[u8]) -> usize {
    if code.is_empty() {
        return 1; // Just the ModR/M byte itself, assume register-direct
    }

    let modrm = code[0];
    let mod_field = (modrm >> 6) & 3;
    let rm_field = modrm & 7;

    let mut len = 1; // ModR/M byte

    match mod_field {
        0b00 => {
            if rm_field == 0b100 {
                // SIB byte follows
                len += 1;
                if code.len() > 1 && (code[1] & 7) == 0b101 {
                    len += 4; // SIB with base=101 in mod=00 → disp32
                }
            } else if rm_field == 0b101 {
                len += 4; // RIP-relative: disp32
            }
        }
        0b01 => {
            if rm_field == 0b100 {
                len += 1; // SIB byte
            }
            len += 1; // disp8
        }
        0b10 => {
            if rm_field == 0b100 {
                len += 1; // SIB byte
            }
            len += 4; // disp32
        }
        0b11 => {
            // Register-direct: no SIB or displacement
        }
        _ => unreachable!(),
    }

    len
}

// ============================================================================
// Helper functions
// ============================================================================

unsafe fn clear_cache_ptr(ptr: *mut u8, size: usize) {
    #[cfg(target_os = "windows")]
    {
        let process = GetCurrentProcess();
        FlushInstructionCache(process, ptr as *const libc::c_void, size);
    }

    #[cfg(target_os = "linux")]
    {
        __clear_cache(ptr, ptr.add(size));
    }
}

unsafe fn free_jit_block(ptr: *mut u8, _size: usize) {
    if ptr.is_null() {
        return;
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        libc::munmap(ptr as *mut libc::c_void, _size);
    }

    #[cfg(target_os = "windows")]
    {
        VirtualFree(ptr as *mut libc::c_void, 0, MEM_RELEASE);
    }
}
