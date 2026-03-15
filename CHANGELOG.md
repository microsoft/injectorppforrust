# 0.5.0 (March 15, 2026)

- Add macOS support.
- Add ARM32 (armv7/thumbv7) support.
- Add thread-local dispatch for parallel test execution on x86_64, ARM64, and ARM32.
- Add macro to support unsafe system function.
- Add feature to prevent injectorpp instance creation.
- Improve error message for fake function called time and arguments mismatch.
- Fix JIT allocation range restriction to AArch64 only.
- Fix test failure on x86-64 Windows by adding static lifetime.

# 0.4.0 (June 23, 2025)

- Introduce type check for major APIs.
- Breaking change for `func!` to require user provide function types.
- Provide unsafe APIs to bypass type check.

# 0.3.3 (May 28, 2025)

- Fix cache coherency issue and thread safety issue.
- Add documents.

# 0.3.2 (May 20, 2025)

- Add support to fake async functions.

# 0.3.1 (May 6, 2025)

- Fix underflow issue when counting call times.
- Use non-poison lock to avoid mess the test execution when panic throws.

# 0.3.0 (May 1, 2025)

- Add Windows support.

# 0.2.0 (Apr 28, 2025)

- Add amd64 support.

# 0.1.0 (Apr 15, 2025)

- Initial version. Supports arm64 and only works on Linux.