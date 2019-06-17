//! OS-specific functionality.

#![stable(feature = "os", since = "1.0.0")]
#![allow(missing_docs, nonstandard_style, missing_debug_implementations)]

#[cfg(target_os = "rvl-ios")]
pub mod rvl_ios;

pub mod raw;
