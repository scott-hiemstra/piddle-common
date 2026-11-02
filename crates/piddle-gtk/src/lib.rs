//! GTK 4 pieces shared by the PIDdle family's GUIs (`piddle-gui` and
//! `kpiddle-gui`): the bubble canvas widget, small GtkSnapshot drawing
//! helpers, and the details windows' building blocks and type-the-name
//! confirmation.
//!
//! This is the only crate in piddle-common that links GTK; the terminal UIs
//! must never depend on it.

pub mod canvas;
pub mod details;
pub mod draw;

pub use canvas::{Canvas, Paint};
