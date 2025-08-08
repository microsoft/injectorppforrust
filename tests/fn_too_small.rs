#![cfg(all(target_os = "linux", target_arch = "aarch64"))]



use injectorpp::interface::injector::*;

#[inline(never)]
#[no_mangle] 
fn ret_only() {
}


#[inline(never)]
#[no_mangle]
fn returns_false() -> bool {
    false
}

/// Should panic because the very first instruction is `RET` at +0.
#[test]
#[should_panic(expected = "Target function too small")]
fn panics_on_ret_at_entry() {
    let mut injector = InjectorPP::new();


    injector
        .when_called(injectorpp::func!(fn (ret_only)() -> ()))
        .will_execute_raw(injectorpp::closure!(|| {}, fn()));
}


#[test]
#[should_panic(expected = "Target function too small")]
fn panics_on_ret_within_window() {
    let mut injector = InjectorPP::new();

    injector
        .when_called(injectorpp::func!(fn (returns_false)() -> bool))
        .will_return_boolean(true); 
}
