//! FX Bet core: the pure financial model plus the request validation and response shaping
//! behind every API endpoint. Storage-agnostic, so the server and the browser build share it.

pub mod domain;
pub mod error;
pub mod frankfurter;
pub mod model;
pub mod service;

pub use error::{Error, Result};
