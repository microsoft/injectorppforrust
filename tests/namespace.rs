//! These tests ensure that injectorpp does not require `use` for types not referenced by consuming
//! crates. Unlike other test modules in this crate, this module should have zero imports from
//! injectorpp.

fn generic<T>(_: T) {}

fn non_generic() {}

async fn _async() {}

#[test]
fn test_func_generic_separate() {
    let _ = injectorpp::func!(generic, fn(usize));
}

#[test]
fn test_func_non_generic_separate() {
    let _ = injectorpp::func!(non_generic, fn());
}

#[test]
fn test_func_non_generic_together() {
    let _ = injectorpp::func!(fn (non_generic)());
}

#[test]
fn test_func_unchecked_generic_separate() {
    let _ = unsafe { injectorpp::func_unchecked!(generic::<usize>) };
}

#[test]
fn test_func_unchecked_non_generic_separate() {
    let _ = unsafe { injectorpp::func_unchecked!(non_generic) };
}

#[test]
fn test_func_unchecked_non_generic_together() {
    let _ = injectorpp::func_unchecked!(fn (non_generic)());
}

#[test]
fn test_closure() {
    let _ = injectorpp::closure!(|| {}, fn());
}

#[test]
fn test_closure_unchecked() {
    let _ = unsafe { injectorpp::closure_unchecked!(|| {}, fn()) };
}

#[tokio::test]
async fn test_async_func() {
    let _ = injectorpp::async_func!(_async(), ());
}

#[tokio::test]
async fn test_async_func_unchecked() {
    let _ = injectorpp::async_func_unchecked!(_async()).await;
}

#[test]
fn test_async_return() {
    let _ = injectorpp::async_return!((), ());
}

#[test]
fn test_async_return_unchecked() {
    let _ = unsafe { injectorpp::async_return_unchecked!((), ()) };
}

#[test]
fn test_fake_when_assign_returns_times() {
    let _ = injectorpp::fake!(
        func_type: fn(x: &mut usize) -> usize,
        when: *x == 0,
        assign: {*x = 0},
        returns: *x,
        times: 0
    );
}

#[test]
fn test_fake_when_assign_returns() {
    let _ = injectorpp::fake!(
        func_type: fn(x: &mut usize) -> usize,
        assign: {*x = 0},
        returns: *x
    );
}

#[test]
fn test_fake_when_returns_times() {
    let _ = injectorpp::fake!(
        func_type: fn() -> usize,
        returns: 0,
        times: 0
    );
}

#[test]
fn test_fake_when_returns() {
    let _ = injectorpp::fake!(
        func_type: fn(x: usize) -> usize,
        when: x == 0,
        returns: 0
    );
}

#[test]
fn test_fake_assign_returns_times() {
    let _ = injectorpp::fake!(
        func_type: fn(x: &mut usize) -> usize,
        assign: {*x = 0},
        returns: 0,
        times: 0
    );
}

#[test]
fn test_fake_assign_returns() {
    let _ = injectorpp::fake!(
        func_type: fn(x: &mut usize) -> usize,
        assign: {*x = 0},
        returns: 0
    );
}

#[test]
fn test_fake_returns_times() {
    let _ = injectorpp::fake!(
        func_type: fn() -> usize,
        returns: 0,
        times: 0
    );
}

#[test]
fn test_fake_returns() {
    let _ = injectorpp::fake!(
        func_type: fn() -> usize,
        returns: 0
    );
}

#[test]
fn test_fake_unit_when_assign_times() {
    let _ = injectorpp::fake!(
        func_type: fn(x: &mut usize) -> (),
        when: *x == 0,
        assign: {*x = 0},
        times: 0
    );
}

#[test]
fn test_fake_unit_when_times() {
    let _ = injectorpp::fake!(
        func_type: fn(x: usize) -> (),
        when: x == 0,
        times: 0
    );
}

#[test]
fn test_fake_unit_when_assign() {
    let _ = injectorpp::fake!(
        func_type: fn(x: &mut usize) -> (),
        when: *x == 0,
        assign: {*x = 0}
    );
}

#[test]
fn test_fake_unit_assign() {
    let _ = injectorpp::fake!(
        func_type: fn(x: &mut usize) -> (),
        assign: {*x = 0}
    );
}

#[test]
fn test_fake_unit_assign_times() {
    let _ = injectorpp::fake!(
        func_type: fn(x: &mut usize) -> (),
        assign: {*x = 0},
        times: 0
    );
}

#[test]
fn test_fake_unit_times() {
    let _ = injectorpp::fake!(
        func_type: fn() -> (),
        times: 0
    );
}

#[test]
fn test_fake_unit() {
    let _ = injectorpp::fake!(
        func_type: fn() -> ()
    );
}

#[test]
fn test_fake_unsafe_when_assign_returns() {
    let _ = injectorpp::fake!(
        func_type: unsafe fn(x: &mut usize) -> usize,
        assign: {*x = 0},
        returns: *x
    );
}

#[test]
fn test_fake_unsafe_when_returns_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe fn() -> usize,
        returns: 0,
        times: 0
    );
}

#[test]
fn test_fake_unsafe_assign_returns_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe fn(x: &mut usize) -> usize,
        assign: {*x = 0},
        returns: 0,
        times: 0
    );
}

#[test]
fn test_fake_unsafe_assign_returns() {
    let _ = injectorpp::fake!(
        func_type: unsafe fn(x: &mut usize) -> usize,
        assign: {*x = 0},
        returns: 0
    );
}

#[test]
fn test_fake_unsafe_returns_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe fn() -> usize,
        returns: 0,
        times: 0
    );
}

#[test]
fn test_fake_unsafe_returns() {
    let _ = injectorpp::fake!(
        func_type: unsafe fn() -> usize,
        returns: 0
    );
}

#[test]
fn test_fake_unsafe_unit_assign() {
    let _ = injectorpp::fake!(
        func_type: unsafe fn(x: &mut usize) -> (),
        assign: {*x = 0}
    );
}

#[test]
fn test_fake_unsafe_unit_assign_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe fn(x: &mut usize) -> (),
        assign: {*x = 0},
        times: 0
    );
}

#[test]
fn test_fake_unsafe_unit_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe fn() -> (),
        times: 0
    );
}

#[test]
fn test_fake_unsafe_unit() {
    let _ = injectorpp::fake!(
        func_type: unsafe fn() -> ()
    );
}

#[test]
fn test_fake_extern_c_when_assign_returns_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "C" fn(x: &mut usize) -> usize,
        when: *x == 0,
        assign: {*x = 0},
        returns: *x,
        times: 0
    );
}

#[test]
fn test_fake_extern_c_when_assign_returns() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "C" fn(x: &mut usize) -> usize,
        assign: {*x = 0},
        returns: *x
    );
}

#[test]
fn test_fake_extern_c_when_returns_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "C" fn() -> usize,
        returns: 0,
        times: 0
    );
}

#[test]
fn test_fake_extern_c_when_returns() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "C" fn(x: usize) -> usize,
        when: x == 0,
        returns: 0
    );
}

#[test]
fn test_fake_extern_c_assign_returns_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "C" fn(x: &mut usize) -> usize,
        assign: {*x = 0},
        returns: 0,
        times: 0
    );
}

#[test]
fn test_fake_extern_c_assign_returns() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "C" fn(x: &mut usize) -> usize,
        assign: {*x = 0},
        returns: 0
    );
}

#[test]
fn test_fake_extern_c_returns_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "C" fn() -> usize,
        returns: 0,
        times: 0
    );
}

#[test]
fn test_fake_extern_c_returns() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "C" fn() -> usize,
        returns: 0
    );
}

#[test]
fn test_fake_extern_c_unit_when_assign_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "C" fn(x: &mut usize) -> (),
        when: *x == 0,
        assign: {*x = 0},
        times: 0
    );
}

#[test]
fn test_fake_extern_c_unit_when_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "C" fn(x: usize) -> (),
        when: x == 0,
        times: 0
    );
}

#[test]
fn test_fake_extern_c_unit_when_assign() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "C" fn(x: &mut usize) -> (),
        when: *x == 0,
        assign: {*x = 0}
    );
}

#[test]
fn test_fake_extern_c_unit_assign() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "C" fn(x: &mut usize) -> (),
        assign: {*x = 0}
    );
}

#[test]
fn test_fake_extern_c_unit_assign_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "C" fn(x: &mut usize) -> (),
        assign: {*x = 0},
        times: 0
    );
}

#[test]
fn test_fake_extern_c_unit_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "C" fn() -> (),
        times: 0
    );
}

#[test]
fn test_fake_extern_c_unit() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "C" fn() -> ()
    );
}

#[test]
fn test_fake_extern_system_when_assign_returns_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "system" fn(x: &mut usize) -> usize,
        when: *x == 0,
        assign: {*x = 0},
        returns: *x,
        times: 0
    );
}

#[test]
fn test_fake_extern_system_when_assign_returns() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "system" fn(x: &mut usize) -> usize,
        assign: {*x = 0},
        returns: *x
    );
}

#[test]
fn test_fake_extern_system_when_returns_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "system" fn() -> usize,
        returns: 0,
        times: 0
    );
}

#[test]
fn test_fake_extern_system_assign_returns_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "system" fn(x: &mut usize) -> usize,
        assign: {*x = 0},
        returns: 0,
        times: 0
    );
}

#[test]
fn test_fake_extern_system_assign_returns() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "system" fn(x: &mut usize) -> usize,
        assign: {*x = 0},
        returns: 0
    );
}

#[test]
fn test_fake_extern_system_returns_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "system" fn() -> usize,
        returns: 0,
        times: 0
    );
}

#[test]
fn test_fake_extern_system_returns() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "system" fn() -> usize,
        returns: 0
    );
}

#[test]
fn test_fake_extern_system_unit_when_assign_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "system" fn(x: &mut usize) -> (),
        when: *x == 0,
        assign: {*x = 0},
        times: 0
    );
}

#[test]
fn test_fake_extern_system_unit_when_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "system" fn(x: usize) -> (),
        when: x == 0,
        times: 0
    );
}

#[test]
fn test_fake_extern_system_unit_when_assign() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "system" fn(x: &mut usize) -> (),
        when: *x == 0,
        assign: {*x = 0}
    );
}

#[test]
fn test_fake_extern_system_unit_assign() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "system" fn(x: &mut usize) -> (),
        assign: {*x = 0}
    );
}

#[test]
fn test_fake_extern_system_unit_assign_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "system" fn(x: &mut usize) -> (),
        assign: {*x = 0},
        times: 0
    );
}

#[test]
fn test_fake_extern_system_unit_times() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "system" fn() -> (),
        times: 0
    );
}

#[test]
fn test_fake_extern_system_unit() {
    let _ = injectorpp::fake!(
        func_type: unsafe extern "system" fn() -> ()
    );
}

#[test]
fn test_verify_func() {
    injectorpp::verify_func!(fn(non_generic)() -> ());
}
