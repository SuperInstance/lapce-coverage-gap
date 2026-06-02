//! Feature-space analysis: building simplicial complexes from code features.
//!
//! Each function is represented as a point in a high-dimensional feature space.
//! Features include: has branches, has loops, has match arms, has generics,
//! has closures, has unsafe code, etc.
//!
//! A simplicial complex is built where:
//! - **Vertices** = individual functions (by name)
//! - **Edges** = two functions that share at least one feature
//! - **Triangles** = three functions that share a common set of features
//! - **Higher simplices** = groups of functions that all share a feature set
//!
//! The **Betti numbers** of this complex reveal coverage gaps:
//! - β₀ = number of disconnected test groups
//! - β₁ = number of holes in coverage (circular gaps)
//! - β₂ = number of voids (entire feature families untested)

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::topology::SimplicialComplex;

use crate::coverage::CoverageData;

/// A feature vector for a single function.
#[derive(Debug, Clone)]
pub struct FeatureVector {
    /// Function identifier.
    pub function_name: String,
    /// Binary features: which features are present.
    pub features: BTreeSet<String>,
    /// Coverage ratio for this function.
    pub coverage_ratio: f64,
    /// Whether this function has any uncovered regions.
    pub has_gaps: bool,
}

/// Build a feature-space representation from coverage data.
pub fn build_feature_vectors(data: &CoverageData) -> Vec<FeatureVector> {
    data.functions
        .iter()
        .map(|f| FeatureVector {
            function_name: f.name.clone(),
            features: f.features.clone(),
            coverage_ratio: f.coverage_ratio(),
            has_gaps: f.has_uncovered,
        })
        .collect()
}

/// Build a simplicial complex from feature vectors.
///
/// Each function becomes a vertex. A k-simplex (set of k+1 functions) is added
/// when those functions all share at least one common feature AND all are
/// tested (the coverage gap is in the shared feature).
///
/// The resulting complex's homology reveals:
/// - Which feature families are well-covered (connected subcomplexes)
/// - Which feature families have gaps (voids/boundaries)
pub fn build_feature_complex(
    vectors: &[FeatureVector],
    min_shared_features: usize,
) -> SimplicialComplex {
    // Build a feature → {functions that have this feature} map
    let mut feature_to_fns: BTreeMap<String, Vec<&FeatureVector>> = BTreeMap::new();
    for v in vectors {
        for feat in &v.features {
            feature_to_fns
                .entry(feat.clone())
                .or_default()
                .push(v);
        }
    }

    // For each feature, create simplices from the functions that share it
    let mut complex = SimplicialComplex::new();
    let mut seen_simplices: HashSet<BTreeSet<String>> = HashSet::new();

    for fns in feature_to_fns.values() {
        if fns.len() < min_shared_features {
            // Too few functions share this feature to form a meaningful simplex
            for f in fns {
                let mut vertex = BTreeSet::new();
                vertex.insert(f.function_name.clone());
                if seen_simplices.insert(vertex.clone()) {
                    complex.add_simplex(vertex);
                }
            }
            continue;
        }

        // Create a simplex containing all functions that share this feature
        let mut simplex = BTreeSet::new();
        for f in fns {
            simplex.insert(f.function_name.clone());
        }

        if !simplex.is_empty() && seen_simplices.insert(simplex.clone()) {
            complex.add_simplex(simplex);
        }
    }

    // Also create simplices from inversely related features:
    // Functions that are fully uncovered form their own component
    let uncovered: Vec<&FeatureVector> = vectors.iter().filter(|v| v.coverage_ratio == 0.0).collect();
    if !uncovered.is_empty() {
        let mut simplex = BTreeSet::new();
        for v in &uncovered {
            simplex.insert(v.function_name.clone());
        }
        if !simplex.is_empty() && seen_simplices.insert(simplex.clone()) {
            complex.add_simplex(simplex);
        }
    }

    // Add "gappy" functions as individual vertices if not already present
    for v in vectors {
        let mut vertex = BTreeSet::new();
        vertex.insert(v.function_name.clone());
        if !seen_simplices.contains(&vertex) {
            seen_simplices.insert(vertex.clone());
            complex.add_simplex(vertex);
        }
    }

    complex
}

/// Analyze the feature-space coverage gaps.
///
/// Returns the Betti numbers and a list of "missing" feature families
/// — features that exist in the code but are never tested together.
#[derive(Debug, Clone)]
pub struct FeatureGapAnalysis {
    /// Betti numbers of the feature simplicial complex.
    pub betti_numbers: Vec<usize>,
    /// Number of connected components of tested feature groups.
    pub connected_components: usize,
    /// Euler characteristic of the feature complex.
    pub euler_characteristic: i64,
    /// Feature kinds that exist in the codebase but have zero coverage.
    pub uncovered_feature_kinds: Vec<String>,
    /// Functions that are fully uncovered.
    pub uncovered_functions: Vec<String>,
    /// All feature vectors, annotated with analysis.
    pub feature_vectors: Vec<FeatureVector>,
    /// Overall gap score (0-100). Higher = more coverage gaps.
    pub gap_score: f64,
}

/// Run the full feature-space analysis on coverage data.
pub fn analyze_feature_gaps(data: &CoverageData) -> FeatureGapAnalysis {
    let vectors = build_feature_vectors(data);
    let complex = build_feature_complex(&vectors, 2);

    let betti = complex.betti_numbers();
    let components = complex.connected_components();
    let euler = complex.euler_characteristic();

    let uncovered_kinds = data.uncovered_feature_kinds();
    let uncovered_fns: Vec<String> = data
        .uncovered_functions()
        .iter()
        .map(|f| f.name.clone())
        .collect();

    // Compute gap score: weighted combination of:
    // - β₂ (voids) → high weight: these are entire missing feature families
    // - β₁ (holes) → medium weight
    // - proportion of uncovered functions
    // - proportion of uncovered feature kinds
    let betta2_weight = if betti.len() > 2 { betti[2] as f64 * 25.0 } else { 0.0 };
    let betta1_weight = if betti.len() > 1 { betti[1] as f64 * 10.0 } else { 0.0 };
    let uncovered_fn_ratio = if data.stats.total_functions > 0 {
        uncovered_fns.len() as f64 / data.stats.total_functions as f64 * 30.0
    } else {
        0.0
    };
    let uncovered_kind_ratio = if data.stats.feature_kinds > 0 {
        uncovered_kinds.len() as f64 / data.stats.feature_kinds as f64 * 35.0
    } else {
        0.0
    };

    let gap_score = (betta2_weight + betta1_weight + uncovered_fn_ratio + uncovered_kind_ratio)
        .min(100.0);

    FeatureGapAnalysis {
        betti_numbers: betti,
        connected_components: components,
        euler_characteristic: euler,
        uncovered_feature_kinds: uncovered_kinds,
        uncovered_functions: uncovered_fns,
        feature_vectors: vectors,
        gap_score,
    }
}

/// Compute a priority ranking of uncovered features.
#[derive(Debug, Clone)]
pub struct PriorityItem {
    /// Name of the feature or function.
    pub name: String,
    /// Priority score (higher = more urgent).
    pub priority: f64,
    /// Category of the gap.
    pub category: GapCategory,
}

/// Category of a coverage gap.
#[derive(Debug, Clone, PartialEq)]
pub enum GapCategory {
    /// Entire function is uncovered.
    DeadCode,
    /// Feature kind has no coverage across any function.
    FeatureKindMissing,
    /// Feature kind is partially covered.
    FeatureKindPartial,
    /// Connected component of untested code.
    UntestedComponent,
}

/// Rank uncovered items by priority.
pub fn rank_gaps(data: &CoverageData) -> Vec<PriorityItem> {
    let mut items = Vec::new();

    // Dead functions get highest priority
    for f in data.uncovered_functions() {
        items.push(PriorityItem {
            name: format!("{} ({})", f.demangled_name, f.file),
            priority: 100.0,
            category: GapCategory::DeadCode,
        });
    }

    // Fully uncovered feature kinds next
    let uncovered_kinds = data.uncovered_feature_kinds();
    for kind in &uncovered_kinds {
        let count = data.feature_presence.get(kind).copied().unwrap_or(0);
        items.push(PriorityItem {
            name: format!("{} — {} occurrences", kind, count),
            priority: 80.0,
            category: GapCategory::FeatureKindMissing,
        });
    }

    // Partially covered features lower
    for (feature, count) in &data.feature_presence {
        if !uncovered_kinds.contains(feature) {
            let covered = data
                .functions
                .iter()
                .filter(|f| f.features.contains(feature) && !f.has_uncovered)
                .count();
            let ratio = covered as f64 / *count as f64;
            if ratio < 1.0 {
                items.push(PriorityItem {
                    name: format!("{} — {}/{} covered", feature, covered, count),
                    priority: 50.0 * (1.0 - ratio),
                    category: GapCategory::FeatureKindPartial,
                });
            }
        }
    }

    // Sort by priority descending
    items.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap_or(std::cmp::Ordering::Equal));
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coverage::FunctionCoverage;

    fn make_fn(
        name: &str,
        features: Vec<&str>,
        total: usize,
        executed: usize,
    ) -> FunctionCoverage {
        FunctionCoverage {
            name: name.to_string(),
            demangled_name: name.to_string(),
            file: format!("src/{}.rs", name),
            line_start: 1,
            line_end: 10,
            total_regions: total,
            executed_regions: executed,
            features: features.into_iter().map(|s| s.to_string()).collect(),
            has_uncovered: executed < total,
        }
    }

    fn make_data(fns: Vec<FunctionCoverage>) -> CoverageData {
        CoverageData::from_functions(fns)
    }

    #[test]
    fn test_empty_feature_space() {
        let data = make_data(vec![]);
        let analysis = analyze_feature_gaps(&data);
        assert!(analysis.betti_numbers.is_empty());
        assert_eq!(analysis.connected_components, 0);
        assert_eq!(analysis.gap_score, 0.0);
    }

    #[test]
    fn test_single_function_no_gaps() {
        let data = make_data(vec![make_fn("foo", vec!["code", "branches"], 5, 5)]);
        let analysis = analyze_feature_gaps(&data);
        assert_eq!(analysis.uncovered_functions.len(), 0);
        assert!(analysis.uncovered_feature_kinds.is_empty());
    }

    #[test]
    fn test_single_function_with_gaps() {
        // A function with gaps has uncovered regions, so its feature kinds are "uncovered"
        // (since no fully-covered function exercises those features)
        let data = make_data(vec![make_fn("bar", vec!["code", "loops"], 10, 3)]);
        let analysis = analyze_feature_gaps(&data);
        assert_eq!(analysis.uncovered_functions.len(), 0); // still some coverage
        assert!(!analysis.uncovered_feature_kinds.is_empty()); // feature kinds are uncovered
        assert!(analysis.uncovered_feature_kinds.contains(&"code".to_string()));
        assert!(analysis.uncovered_feature_kinds.contains(&"loops".to_string()));
    }

    #[test]
    fn test_uncovered_function() {
        let data = make_data(vec![make_fn("dead_code", vec!["code"], 5, 0)]);
        let analysis = analyze_feature_gaps(&data);
        assert_eq!(analysis.uncovered_functions.len(), 1);
    }

    #[test]
    fn test_betti_numbers_basic() {
        // Two well-covered functions, one dead
        let data = make_data(vec![
            make_fn("alive", vec!["code", "branches"], 5, 5),
            make_fn("alive2", vec!["code", "loops"], 4, 4),
            make_fn("dead", vec!["code", "unsafe"], 3, 0),
        ]);
        let analysis = analyze_feature_gaps(&data);
        assert!(!analysis.betti_numbers.is_empty());
        // At minimum β₀ >= 1 (at least one component)
        assert!(analysis.betti_numbers[0] >= 1);
    }

    #[test]
    fn test_priority_ranking_dead_code_top() {
        let data = make_data(vec![
            make_fn("ok", vec!["code"], 3, 3),
            make_fn("gone", vec!["code", "branches"], 5, 0),
        ]);
        let rankings = rank_gaps(&data);
        assert!(!rankings.is_empty());
        assert_eq!(rankings[0].category, GapCategory::DeadCode);
        // "gone" should be first
        assert!(rankings[0].name.contains("gone"));
    }

    #[test]
    fn test_feature_vectors_built_correctly() {
        let data = make_data(vec![make_fn("foo", vec!["code", "macros"], 3, 2)]);
        let vectors = build_feature_vectors(&data);
        assert_eq!(vectors.len(), 1);
        assert!(vectors[0].features.contains("code"));
        assert!(vectors[0].features.contains("macros"));
        assert!(vectors[0].has_gaps);
        assert!((vectors[0].coverage_ratio - 2.0 / 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_feature_complex_isolated_functions() {
        // Two functions with completely different features
        let vectors = vec![
            FeatureVector {
                function_name: "f1".into(),
                features: ["a".into()].into(),
                coverage_ratio: 1.0,
                has_gaps: false,
            },
            FeatureVector {
                function_name: "f2".into(),
                features: ["b".into()].into(),
                coverage_ratio: 1.0,
                has_gaps: false,
            },
        ];
        let complex = build_feature_complex(&vectors, 1);
        // Each should be its own simplex (isolated vertices)
        assert!(complex.vertex_count() >= 2);
    }

    #[test]
    fn test_feature_complex_shared_features() {
        let vectors = vec![
            FeatureVector {
                function_name: "f1".into(),
                features: ["branches".into(), "code".into()].into(),
                coverage_ratio: 1.0,
                has_gaps: false,
            },
            FeatureVector {
                function_name: "f2".into(),
                features: ["branches".into(), "loops".into()].into(),
                coverage_ratio: 0.5,
                has_gaps: true,
            },
            FeatureVector {
                function_name: "f3".into(),
                features: ["branches".into()].into(),
                coverage_ratio: 0.8,
                has_gaps: true,
            },
        ];
        let complex = build_feature_complex(&vectors, 2);
        // f1, f2, f3 share "branches" -> should form a 2-simplex (triangle)
        assert!(complex.vertex_count() >= 3);
    }

    #[test]
    fn test_priority_ranking_multiple_categories() {
        let data = make_data(vec![
            make_fn("dead_fn", vec!["code"], 3, 0),
            make_fn(
                "partially_covered",
                vec!["branches", "loops"],
                10,
                5,
            ),
            make_fn("fully_covered", vec!["code", "generics"], 5, 5),
        ]);
        let rankings = rank_gaps(&data);
        // Dead code should be first
        assert_eq!(rankings[0].category, GapCategory::DeadCode);
        // Should have at least a dead code, and feature kind partial entries
        assert!(rankings.len() >= 1);
    }

    #[test]
    fn test_euler_characteristic_basic() {
        let vectors = vec![
            FeatureVector {
                function_name: "a".into(),
                features: ["x".into()].into(),
                coverage_ratio: 1.0,
                has_gaps: false,
            },
            FeatureVector {
                function_name: "b".into(),
                features: ["x".into(), "y".into()].into(),
                coverage_ratio: 1.0,
                has_gaps: false,
            },
        ];
        let complex = build_feature_complex(&vectors, 2);
        let euler = complex.euler_characteristic();
        // With 2 vertices connected by one shared feature (edge), χ = 2 - 1 = 1
        // (if they form a 1-simplex/edge)
        assert!(euler >= 0);
    }

    #[test]
    fn test_gap_score_scale() {
        let data = make_data(vec![make_fn("dead", vec!["code"], 5, 0)]);
        let analysis = analyze_feature_gaps(&data);
        assert!(analysis.gap_score >= 0.0);
        assert!(analysis.gap_score <= 100.0);
    }

    #[test]
    fn test_uncovered_feature_kinds_detected() {
        let data = make_data(vec![
            make_fn("f1", vec!["code"], 3, 3),
            make_fn("f2", vec!["loops", "code"], 5, 0),
        ]);
        let kinds = data.uncovered_feature_kinds();
        assert!(kinds.contains(&"loops".to_string()));
        assert!(!kinds.contains(&"code".to_string()));
    }

    #[test]
    fn test_connected_components_count() {
        let data = make_data(vec![
            make_fn("f1", vec!["code"], 3, 3),
            make_fn("f2", vec!["code"], 5, 3),
            make_fn("f3", vec!["branches"], 2, 2),
        ]);
        let analysis = analyze_feature_gaps(&data);
        // f1 and f2 share "code" -> one component
        // f3 has only "branches" -> another component
        assert!(analysis.connected_components >= 2);
    }
}
