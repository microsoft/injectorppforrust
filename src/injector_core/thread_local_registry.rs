#![cfg(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "arm"))]

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

#[cfg(target_arch = "aarch64")]
use crate::injector_core::arm64_codegenerator::*;
#[cfg(target_arch = "aarch64")]
use crate::injector_core::utils::*;

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

#[allow(dead_code)] // Fields are stored to keep JIT memory allocations alive
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

        // Free extra JIT block (e.g., return-boolean code) if any.
        // This is safe because tls_remove above already ensures no dispatcher
        // will route to this block from the current thread.
        if let Some((ptr, _size)) = self.extra_jit {
            unsafe {
                free_jit_block(ptr, _size);
            }
        }

        // Decrement ref_count in global registry.
        // We intentionally do NOT restore the original function bytes or free the
        // dispatcher/trampoline when ref_count reaches 0. The dispatcher remains
        // patched into the function permanently. When no thread has a replacement
        // registered, the dispatcher routes through the trampoline to the original
        // function, preserving correct behavior.
        //
        // This avoids a race condition on ARM64 (and theoretically x86_64) where
        // restoring the original bytes and freeing the dispatcher/trampoline can
        // race with another CPU core still executing inside the dispatcher or
        // trampoline from a prior call. On ARM64, instruction cache invalidation
        // is asynchronous across cores, so another core may still be fetching
        // pre-invalidation instructions when the memory is freed.
        //
        // The tradeoff is a small amount of leaked JIT memory (~300 bytes per
        // unique function ever patched) and a minor overhead for calling unpatched
        // functions (one TLS lookup per call). Both are negligible for test code.
        let mut registry = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = registry.get_mut(&self.method_key) {
            entry.ref_count = entry.ref_count.saturating_sub(1);
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
    // This is critical on Windows x86_64 where extern functions go through an IAT thunk.
    let raw_addr = func_ptr.as_ptr() as *mut u8;

    #[cfg(target_arch = "x86_64")]
    let func_addr = unsafe { resolve_function_address(raw_addr) };

    #[cfg(target_arch = "aarch64")]
    let func_addr = raw_addr;

    #[cfg(target_arch = "arm")]
    let func_addr = raw_addr;
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
/// On Windows x86_64, extern "C" functions often go through an import address table (IAT) thunk:
/// `jmp [rip+disp32]` (FF 25 xx xx xx xx). This reads the target from the IAT and returns
/// the real function address. For non-thunk functions, returns the address unchanged.
///
/// This is important because the trampoline copies the original function bytes, and
/// RIP-relative instructions would resolve to wrong addresses in the trampoline.
#[cfg(target_arch = "x86_64")]
unsafe fn resolve_function_address(func_addr: *mut u8) -> *mut u8 {
    let code = std::slice::from_raw_parts(func_addr, 6);
    if code[0] == 0xFF && code[1] == 0x25 {
        // jmp [rip+disp32]: target address is at *(rip_after_insn + disp32)
        let disp = i32::from_le_bytes([code[2], code[3], code[4], code[5]]);
        let rip_after_insn = func_addr.add(6);
        let iat_entry = rip_after_insn.offset(disp as isize) as *const *mut u8;
        let real_addr = std::ptr::read(iat_entry);
        return resolve_function_address(real_addr);
    }
    func_addr
}

/// Install the dispatcher infrastructure for a function:
/// 1. Create a trampoline (original bytes + jump back)
/// 2. Generate the dispatcher JIT code
/// 3. Patch the original function to jump to the dispatcher
fn install_dispatcher(func_addr: *mut u8, method_key: usize) -> MethodEntry {
    #[cfg(target_arch = "x86_64")]
    {
        install_dispatcher_x86_64(func_addr, method_key)
    }

    #[cfg(target_arch = "aarch64")]
    {
        install_dispatcher_aarch64(func_addr, method_key)
    }

    #[cfg(target_arch = "arm")]
    {
        install_dispatcher_arm32(func_addr, method_key)
    }
}

#[cfg(target_arch = "x86_64")]
fn install_dispatcher_x86_64(func_addr: *mut u8, method_key: usize) -> MethodEntry {
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
// ARM64 Dispatcher JIT Code Generation
// ============================================================================

#[cfg(target_arch = "aarch64")]
fn install_dispatcher_aarch64(func_addr: *mut u8, method_key: usize) -> MethodEntry {
    // ARM64 uses a dynamic patch size based on the distance between the function
    // and its dispatcher:
    //
    // - If within ±128MB: 4-byte patch (single B instruction).
    //   A 4-byte aligned write is atomic on ARM64, ensuring cross-core visibility.
    //   Another core sees either the old instruction or the new B, never a partial mix.
    //
    // - If beyond ±128MB: 12-byte patch (ADRP + ADD + BR x16).
    //   This is needed for system library functions (e.g. getenv, memset) on macOS
    //   where the shared cache addresses can be far from allocatable JIT memory.
    //   The 12-byte write is NOT atomic across cores, but the initial patching happens
    //   under the REGISTRY lock (only one thread patches each function). Once installed,
    //   the dispatcher is safe for concurrent execution.

    let near_src =
        unsafe { FuncPtrInternal::new(std::ptr::NonNull::new(func_addr as *mut ()).unwrap()) };

    // Step 1: Pre-allocate dispatcher buffer to determine its address.
    // We allocate with the maximum possible code size so the address is stable.
    const DISPATCHER_MAX_SIZE: usize = 256;
    let dispatcher = allocate_jit_memory(&near_src, DISPATCHER_MAX_SIZE);
    let dispatcher_addr = dispatcher as usize;

    // Step 2: Determine patch size based on actual distance to dispatcher.
    // If within ±128MB, B instruction (4 bytes) reaches; otherwise ADRP+ADD+BR (12 bytes).
    let branch_instrs = crate::injector_core::arm64_codegenerator::maybe_emit_long_jump(
        func_addr as usize,
        dispatcher_addr,
    );
    let patch_size = branch_instrs.len() * 4;

    // Step 3: Create trampoline with the correct copy_size (must match patch_size)
    let (trampoline, trampoline_size) = create_trampoline_aarch64(func_addr, patch_size);
    let trampoline_addr = trampoline as usize;

    // Step 4: Generate dispatcher code with the real trampoline address and write
    // to the pre-allocated buffer.
    let dispatcher_code =
        build_dispatcher_code_aarch64(method_key as u64, trampoline_addr as u64);
    assert!(
        dispatcher_code.len() <= DISPATCHER_MAX_SIZE,
        "Dispatcher code ({} bytes) exceeds pre-allocated buffer ({} bytes)",
        dispatcher_code.len(),
        DISPATCHER_MAX_SIZE
    );
    unsafe {
        inject_asm_code(&dispatcher_code, dispatcher);
    }

    // Step 5: Generate branch patch
    let patch = generate_branch_patch_aarch64(func_addr as usize, dispatcher_addr);
    assert_eq!(
        patch.len(),
        patch_size,
        "Branch patch size changed unexpectedly"
    );

    // Read original bytes before patching
    let original_bytes = unsafe { read_bytes(func_addr, patch_size) };

    // Step 6: Patch the original function
    unsafe {
        patch_function(func_addr, &patch);
    }

    MethodEntry {
        trampoline,
        trampoline_size,
        dispatcher_jit: dispatcher,
        dispatcher_jit_size: DISPATCHER_MAX_SIZE,
        original_bytes,
        func_ptr: func_addr,
        patch_size,
        ref_count: 0,
    }
}

/// Generate branch patch bytes for ARM64.
/// Returns 4 bytes (single B) if within ±128MB, or 12 bytes (ADRP+ADD+BR) otherwise.
#[cfg(target_arch = "aarch64")]
fn generate_branch_patch_aarch64(from: usize, to: usize) -> Vec<u8> {
    let instrs: Vec<u32> =
        crate::injector_core::arm64_codegenerator::maybe_emit_long_jump(from, to);
    let mut bytes = Vec::with_capacity(instrs.len() * 4);
    for insn in &instrs {
        bytes.extend_from_slice(&insn.to_le_bytes());
    }
    bytes
}

/// Build the ARM64 dispatcher code bytes without allocating JIT memory.
#[cfg(target_arch = "aarch64")]
fn build_dispatcher_code_aarch64(method_key_val: u64, trampoline_val: u64) -> Vec<u8> {
    let fn_addr = get_thread_target as *const () as u64;

    let mut code: Vec<u8> = Vec::with_capacity(256);

    // ARM64 calling convention:
    // x0-x7: integer arguments (must save/restore)
    // x8: indirect result location (must save/restore)
    // x9-x15: caller-saved temporaries (can clobber)
    // x16-x17: intra-procedure-call scratch (can clobber)
    // x29: frame pointer, x30: link register
    // q0-q7 (v0-v7): floating point/SIMD arguments (must save/restore)

    // Save x0-x7, x8, x30 (link register) using STP (store pair)
    // STP x0, x1, [sp, #-16]! etc.
    // We need to save: x0-x7 (8 regs), x8, x30 = 10 regs = 80 bytes
    // Plus q0-q7 = 8 × 16 = 128 bytes
    // Total: 208 bytes, round up to 224 for 16-byte alignment

    // sub sp, sp, #224
    emit_sub_sp_imm(&mut code, 224);

    // Save integer registers: x0-x7, x8, x30
    emit_stp_x(&mut code, 0, 1, 0);     // stp x0, x1, [sp, #0]
    emit_stp_x(&mut code, 2, 3, 16);    // stp x2, x3, [sp, #16]
    emit_stp_x(&mut code, 4, 5, 32);    // stp x4, x5, [sp, #32]
    emit_stp_x(&mut code, 6, 7, 48);    // stp x6, x7, [sp, #48]
    emit_stp_x(&mut code, 8, 30, 64);   // stp x8, x30, [sp, #64]

    // Save SIMD/FP registers: q0-q7
    emit_stp_q(&mut code, 0, 1, 80);    // stp q0, q1, [sp, #80]
    emit_stp_q(&mut code, 2, 3, 112);   // stp q2, q3, [sp, #112]
    emit_stp_q(&mut code, 4, 5, 144);   // stp q4, q5, [sp, #144]
    emit_stp_q(&mut code, 6, 7, 176);   // stp q6, q7, [sp, #176]

    // Load arguments for get_thread_target(method_key, trampoline_addr)
    // x0 = method_key, x1 = trampoline_addr
    emit_mov_x_imm64(&mut code, 0, method_key_val);
    emit_mov_x_imm64(&mut code, 1, trampoline_val);

    // Load function address and call
    emit_mov_x_imm64(&mut code, 9, fn_addr);
    // BLR x9
    emit_blr(&mut code, 9);

    // Save return value (target address) in x9
    // MOV x9, x0
    emit_mov_reg(&mut code, 9, 0);

    // Restore SIMD/FP registers: q0-q7
    emit_ldp_q(&mut code, 0, 1, 80);
    emit_ldp_q(&mut code, 2, 3, 112);
    emit_ldp_q(&mut code, 4, 5, 144);
    emit_ldp_q(&mut code, 6, 7, 176);

    // Restore integer registers: x0-x7, x8, x30
    emit_ldp_x(&mut code, 0, 1, 0);
    emit_ldp_x(&mut code, 2, 3, 16);
    emit_ldp_x(&mut code, 4, 5, 32);
    emit_ldp_x(&mut code, 6, 7, 48);
    emit_ldp_x(&mut code, 8, 30, 64);

    // add sp, sp, #224
    emit_add_sp_imm(&mut code, 224);

    // BR x9 (jump to target)
    let br = emit_br(u8_to_bits::<5>(9));
    code.extend_from_slice(&bool_array_to_u32(br).to_le_bytes());

    code
}

/// Create a trampoline for ARM64: copy original instructions + absolute branch back.
/// ARM64 instructions are fixed 4 bytes, so copy_size is always instruction-aligned.
/// PC-relative instructions (ADRP, ADR, B/BL, LDR literal, etc.) are adjusted to
/// account for the trampoline's different address.
#[cfg(target_arch = "aarch64")]
fn create_trampoline_aarch64(
    func_addr: *mut u8,
    copy_size: usize,
) -> (*mut u8, usize) {
    // The jump-back uses MOVZ + MOVK×3 + BR = 20 bytes (5 instructions)
    let jump_back_size = 20;
    let trampoline_total = copy_size + jump_back_size;

    let near_src =
        unsafe { FuncPtrInternal::new(std::ptr::NonNull::new(func_addr as *mut ()).unwrap()) };
    let trampoline = allocate_jit_memory(&near_src, trampoline_total);

    // Build the complete trampoline content in a local buffer
    let mut buf = vec![0u8; trampoline_total];

    // Copy original instruction bytes into buffer
    unsafe {
        std::ptr::copy_nonoverlapping(func_addr, buf.as_mut_ptr(), copy_size);
    }

    // Fix up PC-relative instructions in the buffer (using trampoline's target address)
    fixup_aarch64_pc_relative_buf(&mut buf, trampoline, func_addr, copy_size);

    // Append absolute jump back to original + copy_size.
    // Use x17 (IP1) instead of x16 (IP0) because the copied instructions may use x16
    // (e.g., Windows ARM64 import thunks start with ADRP x16). The jump-back must not
    // clobber registers set by the copied instructions before they're consumed by the
    // original code at func_addr + copy_size.
    let jump_back_target = (func_addr as usize + copy_size) as u64;
    let reg: [bool; 5] = u8_to_bits::<5>(17); // x17 (IP1) scratch register

    let instrs: [u32; 5] = [
        bool_array_to_u32(emit_movz_from_address(
            jump_back_target,
            0,
            true,
            u8_to_bits::<2>(0),
            reg,
        )),
        bool_array_to_u32(emit_movk_from_address(
            jump_back_target,
            16,
            true,
            u8_to_bits::<2>(1),
            reg,
        )),
        bool_array_to_u32(emit_movk_from_address(
            jump_back_target,
            32,
            true,
            u8_to_bits::<2>(2),
            reg,
        )),
        bool_array_to_u32(emit_movk_from_address(
            jump_back_target,
            48,
            true,
            u8_to_bits::<2>(3),
            reg,
        )),
        bool_array_to_u32(emit_br(reg)),
    ];
    for (i, insn) in instrs.iter().enumerate() {
        buf[copy_size + i * 4..copy_size + (i + 1) * 4].copy_from_slice(&insn.to_le_bytes());
    }

    // Write the entire trampoline using inject_asm_code (handles macOS W^X + cache flush)
    unsafe {
        inject_asm_code(&buf, trampoline);
    }

    (trampoline, trampoline_total)
}

/// Fix up PC-relative instructions in an ARM64 trampoline buffer.
///
/// ARM64 PC-relative instructions use offsets relative to the instruction's address.
/// When copied to a trampoline at a different address, these offsets must be adjusted
/// by the delta between the original and trampoline addresses.
///
/// `buf` contains the copied instructions (modified in-place).
/// `trampoline_addr` is the target address where the buffer will be written.
/// `original_addr` is where the instructions were originally located.
///
/// If the adjusted offset overflows the instruction's immediate field, the instruction
/// is NOP-ed out (safe because the trampoline is followed by a jump back to the
/// original function which will re-execute the correct code path).
#[cfg(target_arch = "aarch64")]
fn fixup_aarch64_pc_relative_buf(
    buf: &mut [u8],
    trampoline_addr: *mut u8,
    original_addr: *mut u8,
    copy_size: usize,
) {
    let num_insns = copy_size / 4;
    let nop: u32 = 0xd503201f;

    for i in 0..num_insns {
        let offset = i * 4;
        let insn = u32::from_le_bytes([buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3]]);

        let orig_pc = original_addr as i64 + (i as i64) * 4;
        let tramp_pc = trampoline_addr as i64 + (i as i64) * 4;
        let delta_bytes = orig_pc - tramp_pc;
        let delta_insns = delta_bytes / 4;

        let op_top6 = (insn >> 26) & 0x3F;
        let op_top8 = (insn >> 24) & 0xFF;

        // ADRP: bit[31]=1, bits[28:24]=10000
        // ADR:  bit[31]=0, bits[28:24]=10000
        if (insn >> 24) & 0x1F == 0x10 {
            let is_adrp = (insn >> 31) != 0;
            // Extract immhi (bits 23:5) and immlo (bits 30:29)
            let immlo = (insn >> 29) & 0x3;
            let immhi = (insn >> 5) & 0x7FFFF;
            let imm21 = (immhi << 2) | immlo;
            let imm21_signed = if (imm21 >> 20) & 1 != 0 {
                ((imm21 | 0xFFE00000) as i32) as i64
            } else {
                imm21 as i64
            };

            let new_imm21 = if is_adrp {
                // ADRP: target = (PC & ~0xFFF) + (imm21 << 12)
                let page_delta = ((orig_pc & !0xFFF) - (tramp_pc & !0xFFF)) >> 12;
                imm21_signed + page_delta
            } else {
                // ADR: target = PC + imm21
                imm21_signed + delta_bytes
            };

            if new_imm21 >= -(1 << 20) && new_imm21 < (1 << 20) {
                let new_u = (new_imm21 as u32) & 0x1FFFFF;
                let new_immhi = (new_u >> 2) & 0x7FFFF;
                let new_immlo = new_u & 0x3;
                let new_insn = (insn & !(0x7FFFF << 5) & !(0x3 << 29))
                    | (new_immhi << 5)
                    | (new_immlo << 29);
                buf[offset..offset + 4].copy_from_slice(&new_insn.to_le_bytes());
            } else {
                buf[offset..offset + 4].copy_from_slice(&nop.to_le_bytes());
            }
            continue;
        }

        // B/BL: 26-bit signed offset (instruction-scaled)
        if op_top6 == 0b000101 || op_top6 == 0b100101 {
            let imm26 = insn & 0x03FFFFFF;
            let imm26_signed = if (imm26 >> 25) & 1 != 0 {
                (imm26 | 0xFC000000) as i32
            } else {
                imm26 as i32
            };
            let new_imm26 = (imm26_signed as i64) + delta_insns;
            if new_imm26 >= -(1 << 25) && new_imm26 < (1 << 25) {
                let new_insn =
                    (insn & 0xFC000000) | ((new_imm26 as u32) & 0x03FFFFFF);
                buf[offset..offset + 4].copy_from_slice(&new_insn.to_le_bytes());
            } else {
                buf[offset..offset + 4].copy_from_slice(&nop.to_le_bytes());
            }
            continue;
        }

        // B.cond / CBZ / CBNZ / LDR literal: 19-bit signed offset (instruction-scaled)
        if matches!(
            op_top8,
            0x54 | 0x34 | 0xB4 | 0x35 | 0xB5 | 0x18 | 0x58 | 0x98 | 0x1C | 0x5C | 0x9C
        ) {
            let imm19 = (insn >> 5) & 0x7FFFF;
            let imm19_signed = if (imm19 >> 18) & 1 != 0 {
                (imm19 | 0xFFF80000) as i32
            } else {
                imm19 as i32
            };
            let new_imm19 = (imm19_signed as i64) + delta_insns;
            if new_imm19 >= -(1 << 18) && new_imm19 < (1 << 18) {
                let new_insn =
                    (insn & !(0x7FFFF << 5)) | (((new_imm19 as u32) & 0x7FFFF) << 5);
                buf[offset..offset + 4].copy_from_slice(&new_insn.to_le_bytes());
            } else {
                buf[offset..offset + 4].copy_from_slice(&nop.to_le_bytes());
            }
            continue;
        }

        // TBZ / TBNZ: 14-bit signed offset (instruction-scaled)
        if matches!(op_top8, 0x36 | 0xB6 | 0x37 | 0xB7) {
            let imm14 = (insn >> 5) & 0x3FFF;
            let imm14_signed = if (imm14 >> 13) & 1 != 0 {
                (imm14 | 0xFFFFC000) as i32
            } else {
                imm14 as i32
            };
            let new_imm14 = (imm14_signed as i64) + delta_insns;
            if new_imm14 >= -(1 << 13) && new_imm14 < (1 << 13) {
                let new_insn =
                    (insn & !(0x3FFF << 5)) | (((new_imm14 as u32) & 0x3FFF) << 5);
                buf[offset..offset + 4].copy_from_slice(&new_insn.to_le_bytes());
            } else {
                buf[offset..offset + 4].copy_from_slice(&nop.to_le_bytes());
            }
        }
    }
}

// ============================================================================
// ARM64 Assembly Emission Helpers
// ============================================================================

/// SUB SP, SP, #imm12
#[cfg(target_arch = "aarch64")]
fn emit_sub_sp_imm(code: &mut Vec<u8>, imm: u16) {
    // 1 1 0 1 0 0 0 1 0 0 [imm12] [Rn=SP(31)] [Rd=SP(31)]
    let insn: u32 = 0xD1000000 | ((imm as u32 & 0xFFF) << 10) | (31 << 5) | 31;
    code.extend_from_slice(&insn.to_le_bytes());
}

/// ADD SP, SP, #imm12
#[cfg(target_arch = "aarch64")]
fn emit_add_sp_imm(code: &mut Vec<u8>, imm: u16) {
    // 1 0 0 1 0 0 0 1 0 0 [imm12] [Rn=SP(31)] [Rd=SP(31)]
    let insn: u32 = 0x91000000 | ((imm as u32 & 0xFFF) << 10) | (31 << 5) | 31;
    code.extend_from_slice(&insn.to_le_bytes());
}

/// STP Xt1, Xt2, [SP, #imm7*8]
#[cfg(target_arch = "aarch64")]
fn emit_stp_x(code: &mut Vec<u8>, rt1: u8, rt2: u8, offset: i16) {
    let imm7 = ((offset / 8) as u32) & 0x7F;
    // 10 1 0 1 0 0 1 0 0 [imm7] [Rt2] [Rn=SP(31)] [Rt1]
    let insn: u32 = 0xA9000000 | (imm7 << 15) | ((rt2 as u32) << 10) | (31 << 5) | (rt1 as u32);
    code.extend_from_slice(&insn.to_le_bytes());
}

/// LDP Xt1, Xt2, [SP, #imm7*8]
#[cfg(target_arch = "aarch64")]
fn emit_ldp_x(code: &mut Vec<u8>, rt1: u8, rt2: u8, offset: i16) {
    let imm7 = ((offset / 8) as u32) & 0x7F;
    // 10 1 0 1 0 0 1 0 1 [imm7] [Rt2] [Rn=SP(31)] [Rt1]
    let insn: u32 = 0xA9400000 | (imm7 << 15) | ((rt2 as u32) << 10) | (31 << 5) | (rt1 as u32);
    code.extend_from_slice(&insn.to_le_bytes());
}

/// STP Qt1, Qt2, [SP, #imm7*16]
#[cfg(target_arch = "aarch64")]
fn emit_stp_q(code: &mut Vec<u8>, rt1: u8, rt2: u8, offset: i16) {
    let imm7 = ((offset / 16) as u32) & 0x7F;
    // 10 1 0 1 1 0 0 1 0 0 [imm7] [Rt2] [Rn=SP(31)] [Rt1]
    let insn: u32 = 0xAD000000 | (imm7 << 15) | ((rt2 as u32) << 10) | (31 << 5) | (rt1 as u32);
    code.extend_from_slice(&insn.to_le_bytes());
}

/// LDP Qt1, Qt2, [SP, #imm7*16]
#[cfg(target_arch = "aarch64")]
fn emit_ldp_q(code: &mut Vec<u8>, rt1: u8, rt2: u8, offset: i16) {
    let imm7 = ((offset / 16) as u32) & 0x7F;
    // 10 1 0 1 1 0 1 0 1 [imm7] [Rt2] [Rn=SP(31)] [Rt1]
    let insn: u32 = 0xAD400000 | (imm7 << 15) | ((rt2 as u32) << 10) | (31 << 5) | (rt1 as u32);
    code.extend_from_slice(&insn.to_le_bytes());
}

/// Load a 64-bit immediate into register Xd using MOVZ + MOVK sequence.
/// Uses 2-4 instructions depending on the value.
#[cfg(target_arch = "aarch64")]
fn emit_mov_x_imm64(code: &mut Vec<u8>, rd: u8, val: u64) {
    let reg: [bool; 5] = u8_to_bits::<5>(rd);
    let movz = emit_movz_from_address(val, 0, true, u8_to_bits::<2>(0), reg);
    code.extend_from_slice(&bool_array_to_u32(movz).to_le_bytes());

    let movk1 = emit_movk_from_address(val, 16, true, u8_to_bits::<2>(1), reg);
    code.extend_from_slice(&bool_array_to_u32(movk1).to_le_bytes());

    let movk2 = emit_movk_from_address(val, 32, true, u8_to_bits::<2>(2), reg);
    code.extend_from_slice(&bool_array_to_u32(movk2).to_le_bytes());

    let movk3 = emit_movk_from_address(val, 48, true, u8_to_bits::<2>(3), reg);
    code.extend_from_slice(&bool_array_to_u32(movk3).to_le_bytes());
}

/// BLR Xn (Branch with Link to Register)
#[cfg(target_arch = "aarch64")]
fn emit_blr(code: &mut Vec<u8>, rn: u8) {
    // 1101011 0 0 01 11111 0000 0 0 [Rn] 00000
    let insn: u32 = 0xD63F0000 | ((rn as u32) << 5);
    code.extend_from_slice(&insn.to_le_bytes());
}

/// MOV Xd, Xn (alias for ORR Xd, XZR, Xn)
#[cfg(target_arch = "aarch64")]
fn emit_mov_reg(code: &mut Vec<u8>, rd: u8, rn: u8) {
    // 1 01 01010 00 0 [Rm] 000000 11111 [Rd]
    let insn: u32 = 0xAA0003E0 | ((rn as u32) << 16) | (rd as u32);
    code.extend_from_slice(&insn.to_le_bytes());
}

// ============================================================================
// ARM32 Dispatcher JIT Code Generation
// ============================================================================

#[cfg(target_arch = "arm")]
fn install_dispatcher_arm32(func_addr: *mut u8, method_key: usize) -> MethodEntry {
    // Detect if function is Thumb mode (LSB of original address was set)
    let is_thumb = (func_addr as usize) & 1 != 0;
    // Clear the Thumb bit for actual memory operations
    let func_addr_clean = (func_addr as usize & !1) as *mut u8;

    let near_src =
        unsafe { FuncPtrInternal::new(std::ptr::NonNull::new(func_addr_clean as *mut ()).unwrap()) };

    // Step 1: Pre-allocate dispatcher buffer to determine its address.
    // 64 bytes fits an optional 12-byte Thumb stub + 52-byte ARM dispatcher.
    let dispatcher_max_size = 64;
    let dispatcher = allocate_jit_memory(&near_src, dispatcher_max_size);
    let dispatcher_addr = dispatcher as usize;

    // Step 2: Determine patch size based on distance to dispatcher.
    // B.W (Thumb) has ±16MB range, B (ARM) has ±32MB range.
    let distance = dispatcher_addr.abs_diff(func_addr_clean as usize);
    let max_b_range = if is_thumb { 16 * 1024 * 1024 } else { 32 * 1024 * 1024 };
    let patch_size = if distance < max_b_range { 4 } else { 12 };

    eprintln!(
        "[arm32-diag] install_dispatcher: func={:#x} clean={:#x} disp={:#x} dist={:#x} thumb={} patch={}",
        func_addr as usize, func_addr_clean as usize, dispatcher_addr, distance, is_thumb, patch_size
    );

    // Step 3: Create trampoline (copies patch_size bytes of original code)
    let (trampoline, trampoline_size) =
        create_trampoline_arm32(func_addr_clean, patch_size, is_thumb);
    let trampoline_addr = trampoline as usize | (if is_thumb { 1 } else { 0 });

    // Step 4: Build and write dispatcher code.
    // For Thumb functions with a 4-byte patch, a Thumb-mode stub at the start
    // of the JIT buffer transitions to the ARM-mode dispatcher, because Thumb
    // B.W cannot switch processor mode.
    let arm_dispatcher_code = build_dispatcher_code_arm32(method_key as u32, trampoline_addr as u32);

    if is_thumb && patch_size == 4 {
        let arm_code_addr = (dispatcher_addr + 12) as u32;
        let stub = build_thumb_to_arm_stub(arm_code_addr);
        let total_size = stub.len() + arm_dispatcher_code.len();
        assert!(total_size <= dispatcher_max_size);

        let mut full_code = Vec::with_capacity(total_size);
        full_code.extend_from_slice(&stub);
        full_code.extend_from_slice(&arm_dispatcher_code);
        unsafe { inject_asm_code(&full_code, dispatcher); }
    } else {
        assert!(arm_dispatcher_code.len() <= dispatcher_max_size);
        unsafe { inject_asm_code(&arm_dispatcher_code, dispatcher); }
    }

    // Step 5: Generate branch patch.
    // 4-byte Thumb B.W → Thumb stub at offset 0; 4-byte ARM B → dispatcher at offset 0.
    // 12-byte patches encode the dispatcher address directly.
    let patch = generate_branch_patch_arm32(
        func_addr_clean as usize, dispatcher_addr, is_thumb, patch_size,
    );

    // Read original bytes before patching
    let original_bytes = unsafe { read_bytes(func_addr_clean, patch_size) };

    // Step 6: Patch the function
    unsafe {
        patch_function(func_addr_clean, &patch);
    }

    MethodEntry {
        trampoline,
        trampoline_size,
        dispatcher_jit: dispatcher,
        dispatcher_jit_size: dispatcher_max_size,
        original_bytes,
        func_ptr: func_addr_clean,
        patch_size,
        ref_count: 0,
    }
}

/// Build the ARM32 dispatcher code bytes.
///
/// The dispatcher runs in ARM mode. It:
/// 1. Saves argument registers (r0-r3) and lr
/// 2. Saves VFP argument registers (d0-d7) for hard-float ABI
/// 3. Calls get_thread_target(method_key, trampoline_addr)
/// 4. Restores all registers
/// 5. Branches to the returned target
#[cfg(target_arch = "arm")]
fn build_dispatcher_code_arm32(method_key: u32, trampoline_addr: u32) -> Vec<u8> {
    let fn_addr = get_thread_target as *const () as u32;

    // Push 6 registers (24 bytes) to maintain 8-byte stack alignment per AAPCS.
    // r4 is included as padding (callee-saved, correctly saved/restored).
    let instructions: [u32; 13] = [
        0xE92D402F, // PUSH {r0-r3, r4, lr}
        0xED2D0B10, // VPUSH {d0-d7}
        0xE59F0018, // LDR r0, [pc, #24]  → method_key
        0xE59F1018, // LDR r1, [pc, #24]  → trampoline_addr
        0xE59FC018, // LDR r12, [pc, #24] → fn_addr
        0xE12FFF3C, // BLX r12
        0xE1A0C000, // MOV r12, r0
        0xECBD0B10, // VPOP {d0-d7}
        0xE8BD402F, // POP {r0-r3, r4, lr}
        0xE12FFF1C, // BX r12
        method_key,
        trampoline_addr,
        fn_addr,
    ];

    let mut code = Vec::with_capacity(52);
    for insn in &instructions {
        code.extend_from_slice(&insn.to_le_bytes());
    }
    code
}

/// Build a 12-byte Thumb-mode stub that transitions to an ARM-mode dispatcher.
///
/// Layout: LDR.W r12, [pc, #4]; BX r12; NOP; .word arm_addr
/// The stub executes in Thumb mode and uses BX to switch to ARM mode.
#[cfg(target_arch = "arm")]
fn build_thumb_to_arm_stub(arm_dispatcher_addr: u32) -> Vec<u8> {
    let mut stub = vec![0u8; 12];
    // LDR.W r12, [pc, #4]: F8DF C004
    stub[0] = 0xDF;
    stub[1] = 0xF8;
    stub[2] = 0x04;
    stub[3] = 0xC0;
    // BX r12: 4760
    stub[4] = 0x60;
    stub[5] = 0x47;
    // NOP: BF00
    stub[6] = 0x00;
    stub[7] = 0xBF;
    // .word arm_dispatcher_addr (no Thumb bit — triggers ARM mode switch)
    stub[8..12].copy_from_slice(&arm_dispatcher_addr.to_le_bytes());
    stub
}

/// Create a trampoline for ARM32: copy original instructions + jump back to original.
#[cfg(target_arch = "arm")]
fn create_trampoline_arm32(
    func_addr: *mut u8,
    copy_size: usize,
    is_thumb: bool,
) -> (*mut u8, usize) {
    // Jump-back size: 12 bytes for both ARM and Thumb modes
    let jump_back_size = 12;
    let trampoline_total = copy_size + jump_back_size;

    let near_src =
        unsafe { FuncPtrInternal::new(std::ptr::NonNull::new(func_addr as *mut ()).unwrap()) };
    let trampoline = allocate_jit_memory(&near_src, trampoline_total);

    let mut buf = vec![0u8; trampoline_total];

    // Copy original instruction bytes
    unsafe {
        std::ptr::copy_nonoverlapping(func_addr, buf.as_mut_ptr(), copy_size);
    }

    // Fix up PC-relative instructions in the copied bytes
    fixup_arm32_pc_relative(&mut buf, trampoline, func_addr, copy_size, is_thumb);

    // Append jump-back sequence
    let jump_back_target = func_addr as u32 + copy_size as u32;

    if is_thumb {
        // Thumb mode: LDR.W r12, [pc, #4]; BX r12; NOP; .word target
        // LDR.W r12, [pc, #4]: F8DF C004 → stored as [0xDF, 0xF8, 0x04, 0xC0]
        buf[copy_size] = 0xDF;
        buf[copy_size + 1] = 0xF8;
        buf[copy_size + 2] = 0x04;
        buf[copy_size + 3] = 0xC0;
        // BX r12: 4760
        buf[copy_size + 4] = 0x60;
        buf[copy_size + 5] = 0x47;
        // NOP for alignment: BF00
        buf[copy_size + 6] = 0x00;
        buf[copy_size + 7] = 0xBF;
        // .word target (with Thumb bit set)
        let target_with_thumb = jump_back_target | 1;
        buf[copy_size + 8..copy_size + 12].copy_from_slice(&target_with_thumb.to_le_bytes());
    } else {
        // ARM mode: LDR r12, [pc, #0]; BX r12; .word target
        buf[copy_size..copy_size + 4].copy_from_slice(&0xE59FC000u32.to_le_bytes());
        buf[copy_size + 4..copy_size + 8].copy_from_slice(&0xE12FFF1Cu32.to_le_bytes());
        buf[copy_size + 8..copy_size + 12].copy_from_slice(&jump_back_target.to_le_bytes());
    }

    unsafe {
        inject_asm_code(&buf, trampoline);
    }

    (trampoline, trampoline_total)
}

/// Fix up PC-relative instructions in an ARM32 trampoline buffer.
///
/// For ARM32 function prologues, PC-relative instructions are rare.
/// Most prologues consist of PUSH, SUB SP, MOV fp, etc.
/// We implement basic fixup for common patterns and NOP out any problematic instructions.
#[cfg(target_arch = "arm")]
fn fixup_arm32_pc_relative(
    buf: &mut [u8],
    trampoline_addr: *mut u8,
    original_addr: *mut u8,
    copy_size: usize,
    is_thumb: bool,
) {
    if is_thumb {
        // Thumb mode: instructions are 16-bit or 32-bit.
        // Function prologues in Thumb mode rarely have PC-relative operations.
        return;
    }

    // ARM mode: all instructions are 32-bit
    let nop: u32 = 0xE1A00000; // MOV r0, r0 (ARM NOP)
    let num_insns = copy_size / 4;

    for i in 0..num_insns {
        let offset = i * 4;
        let insn =
            u32::from_le_bytes([buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3]]);

        let orig_pc = original_addr as i64 + (i as i64) * 4 + 8; // ARM: PC = instruction + 8
        let tramp_pc = trampoline_addr as i64 + (i as i64) * 4 + 8;

        // Check if instruction uses PC (r15) as Rn (base register for loads/stores)
        // LDR/STR with Rn=PC: bits [19:16] = 0xF
        let rn = (insn >> 16) & 0xF;
        if rn == 15 {
            // PC-relative load/store — adjust offset
            let is_load = (insn >> 20) & 1 != 0;
            let is_add = (insn >> 23) & 1 != 0;
            let imm12 = (insn & 0xFFF) as i64;
            let orig_offset = if is_add { imm12 } else { -imm12 };
            let orig_target = orig_pc + orig_offset;
            let new_offset = orig_target - tramp_pc;

            if new_offset >= -4095 && new_offset <= 4095 && is_load {
                let u_bit = if new_offset >= 0 { 1u32 << 23 } else { 0 };
                let abs_offset = new_offset.unsigned_abs() as u32 & 0xFFF;
                let new_insn = (insn & 0xFF70F000) | u_bit | abs_offset;
                buf[offset..offset + 4].copy_from_slice(&new_insn.to_le_bytes());
            } else {
                buf[offset..offset + 4].copy_from_slice(&nop.to_le_bytes());
            }
            continue;
        }

        // B/BL instructions: bits [27:24] = 0b1010 (B) or 0b1011 (BL)
        let op = (insn >> 24) & 0xF;
        if op == 0xA || op == 0xB {
            let imm24 = insn & 0x00FFFFFF;
            let imm24_signed = if (imm24 >> 23) & 1 != 0 {
                (imm24 | 0xFF000000) as i32
            } else {
                imm24 as i32
            };
            let orig_target = orig_pc + ((imm24_signed as i64) << 2);
            let new_offset_bytes = orig_target - tramp_pc;
            let new_imm24 = new_offset_bytes >> 2;
            if new_imm24 >= -(1 << 23) && new_imm24 < (1 << 23) {
                let new_insn = (insn & 0xFF000000) | ((new_imm24 as u32) & 0x00FFFFFF);
                buf[offset..offset + 4].copy_from_slice(&new_insn.to_le_bytes());
            } else {
                buf[offset..offset + 4].copy_from_slice(&nop.to_le_bytes());
            }
            continue;
        }

        // ADR (ADD/SUB Rd, PC, #imm): data processing with Rn=PC
        if (insn >> 16) & 0xF == 15 && ((insn >> 21) & 0xF) <= 0xD {
            buf[offset..offset + 4].copy_from_slice(&nop.to_le_bytes());
        }
    }
}

/// Generate the branch patch for ARM32 (4-byte or 12-byte).
///
/// 4-byte patches use a direct branch instruction (Thumb B.W or ARM B).
/// 12-byte patches use register-indirect branches (MOVW/MOVT/BX or LDR/BX/.word).
#[cfg(target_arch = "arm")]
fn generate_branch_patch_arm32(
    src_addr: usize,
    target_addr: usize,
    is_thumb: bool,
    patch_size: usize,
) -> Vec<u8> {
    if patch_size == 4 {
        if is_thumb {
            // Thumb B.W (T4 encoding, unconditional): ±16MB range
            let offset = (target_addr as i32) - (src_addr as i32 + 4);
            let imm = offset >> 1;
            let s = ((imm >> 23) & 1) as u16;
            let imm10 = ((imm >> 11) & 0x3FF) as u16;
            let imm11 = (imm & 0x7FF) as u16;
            let i1 = ((imm >> 22) & 1) as u16;
            let i2 = ((imm >> 21) & 1) as u16;
            let j1 = (!(i1 ^ s)) & 1;
            let j2 = (!(i2 ^ s)) & 1;

            let hw1: u16 = 0xF000 | (s << 10) | imm10;
            let hw2: u16 = 0x9000 | (j1 << 13) | (j2 << 11) | imm11;

            eprintln!(
                "[arm32-diag] B.W: src={:#x} target={:#x} offset={} hw1={:#06x} hw2={:#06x}",
                src_addr, target_addr, offset, hw1, hw2
            );

            // Verify decode: reconstruct offset from encoded values
            let dec_s = ((hw1 >> 10) & 1) as i32;
            let dec_imm10 = (hw1 & 0x3FF) as i32;
            let dec_j1 = ((hw2 >> 13) & 1) as i32;
            let dec_j2 = ((hw2 >> 11) & 1) as i32;
            let dec_imm11 = (hw2 & 0x7FF) as i32;
            let dec_i1 = (!(dec_j1 ^ dec_s)) & 1;
            let dec_i2 = (!(dec_j2 ^ dec_s)) & 1;
            let dec_imm_raw = (dec_s << 23) | (dec_i1 << 22) | (dec_i2 << 21) | (dec_imm10 << 11) | dec_imm11;
            let dec_imm_signed = if dec_s != 0 { dec_imm_raw | !0x00FFFFFF } else { dec_imm_raw };
            let dec_offset = dec_imm_signed << 1;
            let dec_target = (src_addr as i32 + 4 + dec_offset) as usize;
            eprintln!(
                "[arm32-diag] B.W verify: decoded_offset={} decoded_target={:#x} expected={:#x} match={}",
                dec_offset, dec_target, target_addr, dec_target == target_addr
            );
            assert_eq!(dec_target, target_addr, "B.W encoding mismatch");

            let mut patch = vec![0u8; 4];
            patch[0..2].copy_from_slice(&hw1.to_le_bytes());
            patch[2..4].copy_from_slice(&hw2.to_le_bytes());
            patch
        } else {
            // ARM B (unconditional): ±32MB range
            let offset = (target_addr as i32) - (src_addr as i32 + 8);
            let imm24 = ((offset >> 2) as u32) & 0x00FFFFFF;
            let insn: u32 = 0xEA000000 | imm24;
            insn.to_le_bytes().to_vec()
        }
    } else {
        // 12-byte fallback patch
        let mut patch = [0u8; 12];
        let addr = target_addr as u32;

        if is_thumb {
            // MOVW r12, #low16; MOVT r12, #high16; BX r12; NOP
            let low16 = (addr & 0xFFFF) as u16;
            let high16 = ((addr >> 16) & 0xFFFF) as u16;

            let imm4 = ((low16 >> 12) & 0xF) as u16;
            let i = ((low16 >> 11) & 1) as u16;
            let imm3 = ((low16 >> 8) & 0x7) as u16;
            let imm8 = (low16 & 0xFF) as u16;
            let hw1: u16 = 0xF240 | (i << 10) | imm4;
            let hw2: u16 = (imm3 << 12) | (12 << 8) | imm8;
            patch[0..2].copy_from_slice(&hw1.to_le_bytes());
            patch[2..4].copy_from_slice(&hw2.to_le_bytes());

            let imm4 = ((high16 >> 12) & 0xF) as u16;
            let i = ((high16 >> 11) & 1) as u16;
            let imm3 = ((high16 >> 8) & 0x7) as u16;
            let imm8 = (high16 & 0xFF) as u16;
            let hw1: u16 = 0xF2C0 | (i << 10) | imm4;
            let hw2: u16 = (imm3 << 12) | (12 << 8) | imm8;
            patch[4..6].copy_from_slice(&hw1.to_le_bytes());
            patch[6..8].copy_from_slice(&hw2.to_le_bytes());

            patch[8] = 0x60;
            patch[9] = 0x47;
            patch[10] = 0x00;
            patch[11] = 0xBF;
        } else {
            // ARM: LDR r12, [pc, #-0]; BX r12; .word dispatcher_addr
            patch[0..4].copy_from_slice(&0xE51FC000u32.to_le_bytes());
            patch[4..8].copy_from_slice(&0xE12FFF1Cu32.to_le_bytes());
            patch[8..12].copy_from_slice(&addr.to_le_bytes());
        }

        patch.to_vec()
    }
}

// ============================================================================
// x86_64 Dispatcher JIT Code Generation
// ============================================================================

#[cfg(target_arch = "x86_64")]
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

#[cfg(target_arch = "x86_64")]
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
#[cfg(target_arch = "x86_64")]
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
#[cfg(target_arch = "x86_64")]
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

#[cfg(target_arch = "x86_64")]
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

#[cfg(target_arch = "x86_64")]
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

#[cfg(target_arch = "x86_64")]
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

#[cfg(target_arch = "x86_64")]
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

#[cfg(target_arch = "x86_64")]
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

#[cfg(target_arch = "x86_64")]
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

#[cfg(target_arch = "x86_64")]
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

#[allow(dead_code)] // Used by x86_64 trampoline creation; not needed on ARM64
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

    #[cfg(target_os = "macos")]
    {
        crate::injector_core::macosapi::sys_icache_invalidate(ptr, size);
    }

    // Synchronize the instruction pipeline on ARM64.
    #[cfg(target_arch = "aarch64")]
    {
        core::arch::asm!("dsb sy", "isb", options(nostack, nomem));
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
