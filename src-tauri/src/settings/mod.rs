//! Settings — local + vault settings stores, keychain, validation.
//!
//! L4 lands the keychain layer (used by extraction and X bearer). The other
//! submodules remain L5 stubs.

pub mod local_store;
pub mod vault_store;
pub mod keychain;
pub mod secret_store;
pub mod validation;
