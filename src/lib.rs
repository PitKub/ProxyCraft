//! ProxyCraft: fast proxy creator for video editing.
//!
//! The engine (`engine`) has no user-interface dependency: the window runs a
//! [`engine::batch::ProxyBatch`] on worker threads and receives events through
//! a channel.

pub mod engine;
pub mod help_text;
pub mod settings;
pub mod tools;

pub const APP_NAME: &str = "ProxyCraft";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DONATION_ADDRESS: &str = "pitkub@gmail.com";
pub const REPO_URL: &str = "https://github.com/PitKub/ProxyCraft";
