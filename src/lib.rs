//! Library target. The binary is a thin `main`; everything here is reachable
//! from integration tests without going through HTTP.

pub mod common;
pub mod config;
pub mod error;
pub mod modules;
pub mod state;
