//! Shared contract test suite.
//!
//! Both OS adapters must pass [`run_contract_tests`] so behaviour stays identical across
//! macOS and Windows.

use crate::Platform;

/// Run the shared contract suite against an adapter.
///
/// Placeholder: the real assertions land with the first working adapter.
pub fn run_contract_tests<P: Platform>(p: &P) {
    let _ = p;
}
