//! P2 module. Handlers and types are defined ahead of the service layer so the
//! route contract in docs/API_SPEC.md exists; the service functions return
//! `501` until the phase lands. Remove this allow together with the stubs.
#![allow(dead_code)]

pub mod dtos;
pub mod handlers;
pub mod models;
pub mod service;
