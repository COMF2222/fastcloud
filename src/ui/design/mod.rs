//! FastCloud's Airwave design system.
//!
//! The hierarchy is deliberately small and explicit:
//!
//! 1. [`primitives`] owns raw colors and measurements.
//! 2. [`tokens`] gives those values semantic UI roles.
//! 3. [`components`] defines stable geometry for shared controls.
//!
//! Views should consume semantic or component tokens. Raw primitives are for
//! building themes, not for painting one-off widgets.

pub mod components;
pub mod primitives;
pub mod tokens;
pub mod widgets;

#[cfg(test)]
mod screenshot_tests;

pub use tokens::SemanticColors;
