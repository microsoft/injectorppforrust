//! # InjectorPP for Rust
//!
//! **InjectorPP** is a Rust crate that allows you to dynamically fake Rust functions at runtime without modifying your production code. It simplifies unit testing by enabling you to mock dependencies such as file system operations, network calls, and static methods without introducing additional traits or complex refactoring.
//!
//! ## Why InjectorPP
//!
//! When working with a large codebase, a common challenge in writing unit tests is managing dependencies. Elements such as disk I/O, network operations, and even singleton or static functions can cause the code non-unit testable. Traditionally, writing effective unit tests for such code requires refactoring the production code first. This process may involve introducing additional traits, even when there is only one implementation in production. Consequently, numerous traits are created solely for testing purposes, rather than for actual production use.
//!
//! ### Example Scenario
//!
//! Consider the following function that depends on disk operations:
//!
//! ```rust
//! use std::fs;
//!
//! fn try_repair() -> Result<(), String> {
//!     if let Err(e) = fs::create_dir_all("/tmp/target_files") {
//!         return Err(format!("Could not create directory: {}", e));
//!     }
//!     Ok(())
//! }
//! ```
//!
//! Traditionally, you'd need to refactor this function to inject dependencies or set up the environment. With InjectorPP, you can easily mock the dependency:
//!
//! ```rust
//! use injectorpp::interface::injector::*;
//! use std::fs;
//!
//! fn try_repair() -> Result<(), String> {
//!     if let Err(e) = fs::create_dir_all("/tmp/target_files") {
//!         return Err(format!("Could not create directory: {}", e));
//!     }
//!     Ok(())
//! }
//!
//! let mut injector = InjectorPP::new();
//! injector
//!     .when_called(injectorpp::func!(fs::create_dir_all::<&str>))
//!     .will_execute(injectorpp::fake!(
//!         func_type: fn(path: &str) -> std::io::Result<()>,
//!         when: path == "/tmp/target_files",
//!         returns: Ok(()),
//!         times: 1
//!     ));
//!
//! assert!(try_repair().is_ok());
//! ```
//!
//! Notice that `try_repair()` is not changed whereas `fs::create_dir_all`` is successfully abstracted away. No external dependency, all happen in-memory. The above config make sure when `fs::create_dir_all`` is called with `/tmp/target_files` as its path parameter, it will always return `Ok(())` and it's expected to be executed only once. This approach eliminates the need to make a trait solely for testing purposes. It also ensures that previously non-unit testable code is now unit testable.
//!
//! ## Usage
//!
//! Add InjectorPP to your `Cargo.toml`:
//!
//! ```toml
//! [dev-dependencies]
//! injectorpp = "0.x"
//!
//! [profile.test]
//! opt-level = 0
//! debug = true
//! lto = false
//! codegen-units = 1
//! incremental = false
//! ```
//!
//! Import InjectorPP in your tests:
//!
//! ```rust
//! use injectorpp::interface::injector::*;
//! ```
//!
//! ## Features and Examples
//!
//! ### Simple Boolean Return
//!
//! Patch a function to always return a fixed boolean:
//!
//! ```rust
//! use injectorpp::interface::injector::*;
//! use std::path::Path;
//!
//! let mut injector = InjectorPP::new();
//! injector
//!     .when_called(injectorpp::func!(Path::exists))
//!     .will_return_boolean(true);
//!
//! assert!(Path::new("/nonexistent").exists());
//! ```
//!
//! ### Complex Function Mocking
//!
//! Mock functions with conditions, assignments, and expected call counts:
//!
//! ```rust
//! use injectorpp::interface::injector::*;
//!
//! fn original_func(a: &mut i32) -> bool {
//!     *a = 1;
//!     false
//! }
//!
//! let mut injector = InjectorPP::new();
//! injector
//!     .when_called(injectorpp::func!(original_func))
//!     .will_execute(injectorpp::fake!(
//!         func_type: fn(a: &mut i32) -> bool,
//!         assign: { *a = 6 },
//!         returns: true,
//!         times: 1
//!     ));
//!
//! let mut value = 0;
//! let result = original_func(&mut value);
//!
//! assert_eq!(value, 6);
//! assert_eq!(result, true);
//! ```
//!
//! ### Using Custom Functions or Closures
//!
//! Fully customize behavior using your own functions or closures:
//!
//! ```rust
//! use injectorpp::interface::injector::*;
//! use std::path::Path;
//!
//! fn fake_exists(_path: &Path) -> bool {
//!     true
//! }
//!
//! let mut injector = InjectorPP::new();
//! injector
//!     .when_called(injectorpp::func!(Path::exists))
//!     .will_execute_raw(injectorpp::func!(fake_exists));
//!
//! assert!(Path::new("/nonexistent").exists());
//! ```
//!
//! ### Async Function Mocking
//!
//! Mock asynchronous functions easily:
//!
//! ```rust
//! use injectorpp::interface::injector::*;
//!
//! async fn async_add_one(x: u32) -> u32 {
//!     x + 1
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut injector = InjectorPP::new();
//!     injector
//!         .when_called_async(injectorpp::async_func!(async_add_one(0)))
//!         .will_return_async(injectorpp::async_return!(42, u32));
//!
//!     let result = async_add_one(5).await;
//!     assert_eq!(result, 42);
//! }
//! ```
//!
//! ### Scoped Mocking
//!
//! Limit mocking to a specific scope:
//!
//! ```rust
//! use injectorpp::interface::injector::*;
//! use std::path::Path;
//!
//! {
//!     let mut injector = InjectorPP::new();
//!     injector
//!         .when_called(injectorpp::func!(Path::exists))
//!         .will_return_boolean(true);
//!
//!     assert!(Path::new("/nonexistent").exists());
//! }
//!
//! // Outside the scope, original behavior is restored
//! assert!(!Path::new("/nonexistent").exists());
//! ```
//!
//! ## Supported Platforms
//!
//! - **Operating Systems**: Linux, Windows
//! - **Architectures**: arm64, amd64

mod injector_core;
pub mod interface;
