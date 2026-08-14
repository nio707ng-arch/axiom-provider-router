//! Darial — privacy-preserving, provider-neutral LLM gateway.
//!
//! This library crate contains shared modules used by the `darial` binary roles:
//! agent, relay, and gateway. It is not published as a reusable SDK.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod agent;
pub mod auth;
pub mod canonical_ir;
pub mod config;
pub mod crypto_envelope;
pub mod dlp;
pub mod error;
pub mod gateway;
pub mod policy;
pub mod protocol;
pub mod providers;
pub mod relay;
pub mod telemetry;

pub use error::{DarialError, ErrorCode, Result};
