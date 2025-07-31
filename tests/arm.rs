#![cfg(target_arch = "arm")]

use injectorpp::interface::injector::*;

// int test() { return 42; }
const T32_ALIGNED_32_FUNCTION_OPCODES: [u16; 7] = [
    0xB480, // push {r7}
    0xAF00, // add r7, sp, #0
    0x232A, // movs r3, #42 @ 0x2A
    0x4618, // mov r0, r3
    0x46BD, // mov sp, r7
    0xBC80, // pop {r7}
    0x4770, // bx lr
];

// int test() { return 42; }
const A32_ALIGNED_32_FUNCTION_OPCODES: [u32; 7] = [
    0xE52DB004, // push {fp}
    0xE28DB000, // add fp, sp, #0
    0xE3A0302A, // mov r3, #42
    0xE1A00003, // mov r0, r3
    0xE28BD000, // add sp, fp, #0
    0xE49DB004, // pop {fp}
    0xE12FFF1E, // bx lr
];

#[test]
fn test_arm_t32_32_bit_aligned_patch() {
    unsafe {
        // Define an array to hold the T32 function opcodes
        // It should be aligned on 32-bit
        let allocated_memory: [u8; T32_ALIGNED_32_FUNCTION_OPCODES.len() * 2 + 4] =
            [0; T32_ALIGNED_32_FUNCTION_OPCODES.len() * 2 + 4];
        let allocated_memory_ptr = allocated_memory.as_ptr() as *mut u8;
        let aligned_memory = ((allocated_memory_ptr as usize)
            + (4 - (allocated_memory_ptr as usize % 4)) % 4)
            as *mut u8;
        assert!(
            aligned_memory as usize % 4 == 0,
            "Aligned memory should be 4-byte aligned"
        );

        // Write the T32 function opcodes to the array
        for (i, &opcode) in T32_ALIGNED_32_FUNCTION_OPCODES.iter().enumerate() {
            for (j, b) in opcode.to_le_bytes().iter().enumerate() {
                aligned_memory.add(i * 2 + j + 2).write(*b);
            }
        }

        let mut injector = InjectorPP::new();
        injector
            .when_called(FuncPtr::new(
                aligned_memory.add(1) as *const (), // Add 1 to make it a T32 function
                "fn() -> u32",
            ))
            .will_execute_raw(injectorpp::closure!(|| { 99 }, fn() -> u32));

        // Call the T32 function
        let result =
            std::mem::transmute::<*const (), fn() -> u32>(aligned_memory.add(1) as *const ())();
        assert_eq!(result, 99, "T32 function should return 99");
    }
}

#[test]
fn test_arm_t32_16_bit_aligned_patch() {
    unsafe {
        // Define an array to hold the T32 function opcodes
        // It should be aligned on 16-bit but not on 32-bit
        let allocated_memory: [u8; T32_ALIGNED_32_FUNCTION_OPCODES.len() * 2 + 4] =
            [0; T32_ALIGNED_32_FUNCTION_OPCODES.len() * 2 + 4];
        let allocated_memory_ptr = allocated_memory.as_ptr() as *mut u8;
        let mut aligned_memory = ((allocated_memory_ptr as usize)
            + (2 - (allocated_memory_ptr as usize % 2)) % 2)
            as *mut u8;
        if aligned_memory as usize % 4 == 0 {
            aligned_memory = aligned_memory.add(2); // Ensure to make it not 4-byte aligned
        }
        assert!(
            aligned_memory as usize % 2 == 0,
            "Aligned memory should be 2-byte aligned"
        );
        assert!(
            aligned_memory as usize % 4 != 0,
            "Aligned memory should not be 4-byte aligned"
        );

        // Write the T32 function opcodes to the array
        for (i, &opcode) in T32_ALIGNED_32_FUNCTION_OPCODES.iter().enumerate() {
            for (j, b) in opcode.to_le_bytes().iter().enumerate() {
                aligned_memory.add(i * 2 + j).write(*b);
            }
        }

        let mut injector = InjectorPP::new();
        injector
            .when_called(FuncPtr::new(
                aligned_memory.add(1) as *const (), // Add 1 to make it a T32 function
                "fn() -> u32",
            ))
            .will_execute_raw(injectorpp::closure!(|| { 99 }, fn() -> u32));

        // Call the T32 function
        let result =
            std::mem::transmute::<*const (), fn() -> u32>(aligned_memory.add(1) as *const ())();
        assert_eq!(result, 99, "T32 function should return 99");
    }
}

#[test]
fn test_arm_a32_32_bit_aligned_patch() {
    unsafe {
        // Define an array to hold the A32 function opcodes
        let allocated_memory: [u8; A32_ALIGNED_32_FUNCTION_OPCODES.len() * 4 + 4] =
            [0; A32_ALIGNED_32_FUNCTION_OPCODES.len() * 4 + 4];
        let allocated_memory_ptr = allocated_memory.as_ptr() as *mut u8;
        let aligned_memory = ((allocated_memory_ptr as usize)
            + (4 - (allocated_memory_ptr as usize % 4)) % 4)
            as *mut u8;
        assert!(
            aligned_memory as usize % 4 == 0,
            "Aligned memory should be 4-byte aligned"
        );

        // Write the A32 function opcodes to the array
        for (i, &opcode) in A32_ALIGNED_32_FUNCTION_OPCODES.iter().enumerate() {
            for (j, b) in opcode.to_le_bytes().iter().enumerate() {
                aligned_memory.add(i * 4 + j).write(*b);
            }
        }

        let mut injector = InjectorPP::new();
        injector
            .when_called(FuncPtr::new(aligned_memory as *const (), "fn() -> u32"))
            .will_execute_raw(injectorpp::closure!(|| { 99 }, fn() -> u32));

        // Call the A32 function
        let result = std::mem::transmute::<*const (), fn() -> u32>(aligned_memory as *const ())();
        assert_eq!(result, 99, "A32 function should return 99");
    }
}
