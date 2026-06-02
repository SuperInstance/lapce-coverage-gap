//! # lapce-coverage-gap
//!
//! **Find what your tests DON'T cover.**
//!
//! Your tests cover 80% of lines. But *which* 20% is missing? And is it the
//! *important* 20%?
//!
//! `lapce-coverage-gap` analyzes Rust test coverage data and builds a
//! **simplicial complex** from code features — branches, loops, match arms,
//! generics, closures — then computes **Betti numbers** to find topological
//! holes in your test coverage.
//!
//! Uses [`negative-space-testing`] for property-based negative space tests,
//! and its own topological analysis engine for coverage gap detection.
//!
//! ## How it works
//!
//! 1. **Parses** coverage data (JSON format)
//! 2. **Extracts** code features from each function, forming a feature-vector space
//! 3. **Builds** a simplicial complex where each function is a vertex, and
//!    tested-together features form simplices
//! 4. **Computes** Betti numbers to reveal:
//!    - β₀ — disconnected test islands
//!    - β₁ — circular/cavity gaps in coverage
//!    - β₂ — feature voids (entire families untested)
//! 5. **Ranks** uncovered features by priority
//! 6. **Outputs** a terminal-friendly report + JSON for editor integration

pub mod coverage;
pub mod feature_space;
pub mod gap;
pub mod report;
pub mod topology;

// Re-exports for convenience
pub use coverage::*;
pub use feature_space::*;
pub use gap::*;
pub use report::*;
