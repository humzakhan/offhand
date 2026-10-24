//! Shared contract test suite.
//!
//! Both OS adapters must pass [`run_contract_tests`] so behaviour stays identical across
//! macOS and Windows.

use crate::Platform;

/// Run the shared contract suite against an adapter.
///
/// This is a placeholder and asserts nothing yet. Real fixtures and assertions are added in
/// weeks 3-4, once the first adapters can read a tree and perform actions.
pub fn run_contract_tests<P: Platform>(p: &P) {
    let _ = p;
}
