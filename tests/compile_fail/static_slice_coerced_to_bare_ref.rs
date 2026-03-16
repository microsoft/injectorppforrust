/// Bare &[u8] return when function actually returns &'static [u8].
/// Same class of bug as &str but with a slice type.
use injectorpp::interface::injector::*;

fn get_bytes(_x: i32) -> &'static [u8] {
    b"hello"
}

fn main() {
    let _f = injectorpp::func!(fn (get_bytes)(i32) -> &[u8]);
}
