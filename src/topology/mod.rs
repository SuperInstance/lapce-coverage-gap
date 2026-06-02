//! Algebraic topology for coverage gap analysis.
//!
//! Builds simplicial complexes from code features and computes Betti numbers
//! to find "holes" in test coverage.
//!
//! ## Key concepts
//!
//! - **Betti numbers** reveal the shape of your test space:
//!   - β₀ = connected components = independent test groups
//!   - β₁ = 1-dimensional holes = circular/cavity coverage gaps
//!   - β₂ = 2-dimensional voids = entire feature families untested
//!
//! - **Euler characteristic** is a single-number health metric
//!
//! Reference: Hatcher (2002), "Algebraic Topology"

mod complex;
mod homology;

pub use complex::*;
pub use homology::*;
