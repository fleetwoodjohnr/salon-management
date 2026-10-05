//! Tauri command handlers: deserialize (unknown fields rejected) → validate → db/domain.
//! Handlers are `async` so they run off the UI thread.

pub mod app;
pub mod core;
pub mod inventory;
pub mod profiles;
pub mod providers;
pub mod reports;
pub mod sales;
pub mod services;
pub mod tax;
