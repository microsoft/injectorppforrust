#![cfg(all(target_arch = "aarch64", target_os = "linux"))]

use injectorpp::interface::injector::*;
use injectorpp::interface::injector::FuncPtr;

#[no_mangle]
#[inline(never)]
pub fn too_small_to_patch() -> u64 {
      unsafe { core::ptr::read_volatile(&1) }
}



#[test]
#[should_panic(expected = "too small")]
fn test_patch_too_small_function_should_panic() {
    let mut injector = InjectorPP::new();
    injector
  .when_called(injectorpp::func!(fn (too_small_to_patch)() -> u64))


        .will_execute(injectorpp::fake!(
            func_type: fn() -> u64,
            returns: 42
        ));
}
