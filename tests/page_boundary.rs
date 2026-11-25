//! Regression tests for faking a function whose patched bytes cross a page boundary.
//!
//! injectorpp overwrites the first bytes of the function to fake with a branch. When the
//! function starts fewer bytes before the end of a page than the branch is long, the write
//! continues into the next page, so that page must be made writable as well.
//!
//! The functions below are placed with `global_asm!` so that each one starts `EDGE_OFFSET`
//! bytes before a page boundary:
//! - x86_64: 2 bytes before the boundary. `new()` and `new_global()` both write a 5-byte `jmp`.
//! - aarch64: 8 bytes before the boundary. `new_global()` writes 12 bytes. `new()` writes a
//!   4-byte `B` to its dispatcher, which ends before the boundary.
//!
//! Each function is 16 bytes long, so the patch stays inside the function.
#![cfg(all(
    any(target_os = "linux", target_os = "windows", target_os = "macos"),
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

use injectorpp::interface::injector::*;

#[cfg(target_arch = "x86_64")]
const EDGE_OFFSET: usize = 2;
#[cfg(target_arch = "aarch64")]
const EDGE_OFFSET: usize = 8;

// 64 KB is a page boundary for 4 KB, 16 KB and 64 KB pages.
#[cfg(target_os = "linux")]
const SECTION_ALIGNMENT: usize = 64 * 1024;
// link.exe rejects a section alignment above its /ALIGN value, which is 4 KB by default.
// Windows uses 4 KB pages on x86_64 and aarch64, so 4 KB is a page boundary there.
#[cfg(target_os = "windows")]
const SECTION_ALIGNMENT: usize = 4 * 1024;
// macOS uses 16 KB pages on aarch64 and 4 KB pages on x86_64, so 16 KB is a page boundary on both.
#[cfg(target_os = "macos")]
const SECTION_ALIGNMENT: usize = 16 * 1024;

const ORIGINAL_VALUE: i32 = 42;
const FAKE_VALUE: i32 = 7;

#[cfg(target_os = "linux")]
macro_rules! push_code_section {
    ($name:literal) => {
        concat!(".pushsection .text.", $name, ",\"ax\",%progbits")
    };
}

#[cfg(target_os = "windows")]
macro_rules! push_code_section {
    ($name:literal) => {
        concat!(".pushsection .text$", $name, ",\"xr\"")
    };
}

// Ten 1-byte NOPs, `mov eax, imm32` (5 bytes) and `ret` (1 byte).
#[cfg(target_arch = "x86_64")]
macro_rules! function_body {
    () => {
        "nop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nnop\nmov eax, {value}\nret"
    };
}

// Two NOPs, `mov w0, #imm` and `ret`, 4 bytes each.
#[cfg(target_arch = "aarch64")]
macro_rules! function_body {
    () => {
        "nop\nnop\nmov w0, #{value}\nret"
    };
}

/// Defines a function that returns `ORIGINAL_VALUE` and starts `EDGE_OFFSET` bytes before a
/// `SECTION_ALIGNMENT` boundary.
#[cfg(not(target_os = "macos"))]
macro_rules! page_edge_function {
    ($name:literal) => {
        core::arch::global_asm!(
            push_code_section!($name),
            ".balign {alignment}",
            ".space {padding}",
            concat!(".globl ", $name),
            concat!($name, ":"),
            function_body!(),
            ".popsection",
            alignment = const SECTION_ALIGNMENT,
            padding = const SECTION_ALIGNMENT - EDGE_OFFSET,
            value = const ORIGINAL_VALUE,
        );
    };
}

/// Defines a function that returns `ORIGINAL_VALUE` and starts `EDGE_OFFSET` bytes before a
/// `SECTION_ALIGNMENT` boundary.
///
/// Mach-O needs three changes compared to ELF and COFF:
/// - C symbol names start with an underscore.
/// - Section names have at most 16 characters, so both functions share the `__page_edge`
///   section.
/// - The linker splits a section into atoms at each symbol, and `-dead_strip` can remove an
///   atom that holds only padding, which moves the function to another address. The `_page`
///   label starts an atom at the aligned address, and `.alt_entry` keeps the function inside
///   that atom, so the padding stays in front of the function.
#[cfg(target_os = "macos")]
macro_rules! page_edge_function {
    ($name:literal) => {
        core::arch::global_asm!(
            ".pushsection __TEXT,__page_edge,regular,pure_instructions",
            ".balign {alignment}",
            concat!("_", $name, "_page:"),
            ".space {padding}",
            concat!(".globl _", $name),
            concat!(".alt_entry _", $name),
            concat!("_", $name, ":"),
            function_body!(),
            ".popsection",
            alignment = const SECTION_ALIGNMENT,
            padding = const SECTION_ALIGNMENT - EDGE_OFFSET,
            value = const ORIGINAL_VALUE,
        );
    };
}

page_edge_function!("injectorpp_page_edge_thread_local");
page_edge_function!("injectorpp_page_edge_global");

extern "C" {
    fn injectorpp_page_edge_thread_local() -> i32;
    fn injectorpp_page_edge_global() -> i32;
}

fn page_size() -> usize {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
    }

    #[cfg(target_os = "windows")]
    {
        4096
    }
}

/// Checks that the function was placed as intended, so the tests really patch across a page
/// boundary.
fn assert_starts_before_page_boundary(func: unsafe extern "C" fn() -> i32) {
    let addr = func as usize;
    let page_size = page_size();
    assert_eq!(
        addr % page_size,
        page_size - EDGE_OFFSET,
        "the function at {addr:#x} must start {EDGE_OFFSET} bytes before a page boundary \
         (page size {page_size:#x})"
    );
}

/// `new()` patches the start of the function with a branch to the thread-local dispatcher.
#[test]
fn test_thread_local_fake_of_function_at_page_boundary() {
    assert_starts_before_page_boundary(injectorpp_page_edge_thread_local);
    assert_eq!(
        unsafe { injectorpp_page_edge_thread_local() },
        ORIGINAL_VALUE
    );

    {
        let mut injector = InjectorPP::new();
        injector
            .when_called(injectorpp::func!(
                unsafe{} extern "C" fn (injectorpp_page_edge_thread_local)() -> i32
            ))
            .will_execute(injectorpp::fake!(
                func_type: unsafe extern "C" fn() -> i32,
                returns: FAKE_VALUE,
                times: 1
            ));

        assert_eq!(unsafe { injectorpp_page_edge_thread_local() }, FAKE_VALUE);
    }

    // The dispatcher stays installed and routes the call to the original code.
    assert_eq!(
        unsafe { injectorpp_page_edge_thread_local() },
        ORIGINAL_VALUE
    );
}

/// `new_global()` patches the function directly, and dropping the injector writes the original
/// bytes back, so both writes cross the page boundary.
#[test]
fn test_global_fake_of_function_at_page_boundary() {
    assert_starts_before_page_boundary(injectorpp_page_edge_global);
    assert_eq!(unsafe { injectorpp_page_edge_global() }, ORIGINAL_VALUE);

    {
        let mut injector = InjectorPP::new_global();
        injector
            .when_called(injectorpp::func!(
                unsafe{} extern "C" fn (injectorpp_page_edge_global)() -> i32
            ))
            .will_execute(injectorpp::fake!(
                func_type: unsafe extern "C" fn() -> i32,
                returns: FAKE_VALUE,
                times: 1
            ));

        assert_eq!(unsafe { injectorpp_page_edge_global() }, FAKE_VALUE);
    }

    assert_eq!(unsafe { injectorpp_page_edge_global() }, ORIGINAL_VALUE);
}
