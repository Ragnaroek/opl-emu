#![cfg_attr(not(feature = "sdl"), no_std)]

#[cfg(feature = "sdl")]
pub mod backend_sdl;

#[cfg(feature = "sdl")]
pub fn new() -> Result<backend_sdl::OPL, &'static str> {
    return backend_sdl::new();
}

