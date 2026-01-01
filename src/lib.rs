#![cfg_attr(feature = "web-worklet", no_std)]

#[cfg(feature = "chip")]
pub mod chip;

#[cfg(feature = "sdl")]
pub mod sdl;
#[cfg(feature = "sdl")]
pub use sdl::{OPL, OPLSettings};

#[cfg(feature = "web")]
pub mod web;
#[cfg(feature = "web")]
pub use web::{OPL, OPLSettings};

#[cfg(feature = "web-worklet")]
pub mod web_worklet;

#[cfg(feature = "catalog")]
pub mod catalog;
