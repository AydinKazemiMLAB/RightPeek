//! Pure, OS-independent logic. Nothing in here may call platform APIs,
//! so every item is unit-testable on any OS (docs/PLAN.md §3).

pub mod capture;
