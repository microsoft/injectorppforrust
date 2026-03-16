/// Issue #73 variant: same problem with &[u8] return type.
/// The function returns &'static [u8] but user writes &[u8].
use injectorpp::interface::injector::*;

fn get_bytes(_x: i32) -> &'static [u8] {
    b"hello"
}

fn main() {
    let _f = injectorpp::func!(fn (get_bytes)(i32) -> &[u8]);
}
