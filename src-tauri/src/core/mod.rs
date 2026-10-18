//! Pure, OS-independent logic. Nothing in here may call platform APIs,
//! so every item is unit-testable on any OS (docs/design.md).

pub mod capture;
pub mod log;
pub mod settings;
pub mod text;
