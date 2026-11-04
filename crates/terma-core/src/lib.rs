//! Terma-Kern: alles, was ohne Root läuft.

pub mod appimage;
pub mod appimage_update;
pub mod aur;
pub mod aur_build;
pub mod desktop;
pub mod dirs;
pub mod error;
pub mod file_flow;
pub mod flatpak;
pub mod http;
pub mod integrate;
pub mod ops;
pub mod pacman;
pub mod programs;
pub mod runner;
pub mod search;
pub mod settings;
pub mod trust;

pub use error::{BuildFailure, Result, TermaError};
