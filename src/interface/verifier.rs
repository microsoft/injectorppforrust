use std::sync::atomic::{AtomicUsize, Ordering};

// Define a verifier guard that checks the counter on Drop.
/// A verifier type that holds a reference to an atomic counter and the expected call count.
pub enum CallCountVerifier {
    /// A real verifier that checks if the fake function was called the expected number of times.
    WithCount {
        counter: &'static AtomicUsize,
        expected: usize,
    },

    /// A dummy verifier that performs no check.
    Dummy,
}

impl Drop for CallCountVerifier {
    fn drop(&mut self) {
        if let CallCountVerifier::WithCount { counter, expected } = self {
            let call_times = counter.load(Ordering::SeqCst);
            if call_times != *expected {
                // Avoid double panic
                if std::thread::panicking() {
                    return;
                }

                panic!(
                    "Fake function was expected to be called {} time(s), but it is actually called {} time(s)",
                    expected, call_times
                );
            }
        }

        // Dummy variant does nothing on drop.
    }
}
