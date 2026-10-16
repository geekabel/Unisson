//! The commands the web interface can call.
//!
//! Each function here is one entry of the `invoke` API of Tauri. They are
//! thin on purpose: they translate, and let the other crates do the work.
//!
//! A command that only passes an order to the playback session returns at
//! once, and runs on the main thread. A command that reads the database, a
//! file or the network is marked `async`, so that Tauri runs it on another
//! thread and the window never freezes.

pub mod collection;
pub mod home;
pub mod player;
pub mod queue;
pub mod radio;
pub mod search;
pub mod settings;
pub mod spotify;
pub mod window;
