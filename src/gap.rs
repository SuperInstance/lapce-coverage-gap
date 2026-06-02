//! Core gap analysis: the `CoverageGapReport` type and associated utilities.
//!
//! This module ties together coverage parsing, feature-space analysis, and
//! the negative-space-testing topological tools to produce a comprehensive
//! report of what your tests DON'T cover.

use crate::coverage::{CoverageData, CoverageStats};
use crate::feature_space::{
    analyze_feature_gaps, rank_gaps,
    FeatureGapAnalysis, GapCategory, PriorityItem,
};

/// The top-level report produced by coverage gap analysis.
///
/// Combines traditional coverage stats with topological gap finding.
#[derive(Debug, Clone)]
pub struct CoverageGapReport {
    /// Traditional coverage statistics.
    pub stats: CoverageStats,
    /// Topological gap analysis (Betti numbers, etc.).
    pub gap_analysis: FeatureGapAnalysis,
    /// Priority-ranked list of gaps to fix.
    pub priority_items: Vec<PriorityItem>,
    /// Missing feature families — entire classes of code untested.
    pub missing_feature_families: Vec<String>,
    /// Betti numbers (convenience access).
    pub betti_numbers: Vec<usize>,
    /// Overall health assessment.
    pub health: ReportHealth,
}

/// Overall health of the test suite based on gap analysis.
#[derive(Debug, Clone, PartialEq)]
pub enum ReportHealth {
    /// Coverage is solid — no significant gaps.
    Healthy,
    /// Some gaps exist, but nothing critical.
    Warning,
    /// Major gaps — attention needed.
    Critical,
    /// No data to analyze.
    Unknown,
}

impl ReportHealth {
    /// A simple 0-100 score suitable for editor integration.
    pub fn score(&self) -> u8 {
        match self {
            ReportHealth::Healthy => 100,
            ReportHealth::Warning => 60,
            ReportHealth::Critical => 20,
            ReportHealth::Unknown => 0,
        }
    }

    /// Color-code for terminal output.
    pub fn color_code(&self) -> &'static str {
        match self {
            ReportHealth::Healthy => "\x1b[32m",   // green
            ReportHealth::Warning => "\x1b[33m",   // yellow
            ReportHealth::Critical => "\x1b[31m",  // red
            ReportHealth::Unknown => "\x1b[90m",   // gray
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ReportHealth::Healthy => "HEALTHY",
            ReportHealth::Warning => "WARNING",
            ReportHealth::Critical => "CRITICAL",
            ReportHealth::Unknown => "UNKNOWN",
        }
    }
}

impl CoverageGapReport {
    /// Run the full analysis on a `CoverageData` object.
    pub fn from_coverage_data(data: &CoverageData) -> Self {
        let gap_analysis = analyze_feature_gaps(data);
        let priority_items = rank_gaps(data);

        let betti = gap_analysis.betti_numbers.clone();
        let missing: Vec<String> = gap_analysis.uncovered_feature_kinds.clone();

        let health = Self::determine_health(&gap_analysis, &priority_items);

        CoverageGapReport {
            stats: data.stats.clone(),
            gap_analysis: gap_analysis.clone(),
            priority_items,
            missing_feature_families: missing,
            betti_numbers: betti,
            health,
        }
    }

    /// Determine overall health from analysis results.
    fn determine_health(
        analysis: &FeatureGapAnalysis,
        items: &[PriorityItem],
    ) -> ReportHealth {
        if analysis.feature_vectors.is_empty() {
            return ReportHealth::Unknown;
        }

        // Critical: any β₂ > 0 (voids = entire feature families untested)
        if analysis.betti_numbers.len() > 2 && analysis.betti_numbers[2] > 0 {
            return ReportHealth::Critical;
        }

        // Critical: any dead functions (0% coverage)
        if !analysis.uncovered_functions.is_empty() {
            return ReportHealth::Critical;
        }

        // Critical: any feature kind with zero coverage across all functions
        if !analysis.uncovered_feature_kinds.is_empty() {
            return ReportHealth::Critical;
        }

        // Warning: holes in coverage (β₁ > 0)
        if analysis.betti_numbers.len() > 1 && analysis.betti_numbers[1] > 0 {
            return ReportHealth::Warning;
        }

        // Warning: many disconnected components or partially-covered features
        if !items.is_empty() {
            return ReportHealth::Warning;
        }

        ReportHealth::Healthy
    }

    /// Generate a terminal-friendly report.
    pub fn to_terminal_report(&self) -> String {
        let mut out = String::new();

        out.push_str("═══════════════════════════════════════════════════\n");
        out.push_str("    lapce-coverage-gap — Coverage Gap Report\n");
        out.push_str("═══════════════════════════════════════════════════\n\n");

        // Overall health
        out.push_str(&format!(
            "Health: {}{}\x1b[0m (score: {})\n\n",
            self.health.color_code(),
            self.health.as_str(),
            self.health.score(),
        ));

        // Coverage stats
        out.push_str("── Coverage Stats ──\n");
        out.push_str(&format!("  Functions:       {} total, {} full, {} partial\n",
            self.stats.total_functions, self.stats.full_functions, self.stats.partial_functions));
        out.push_str(&format!("  Regions:         {} total, {} executed\n",
            self.stats.total_regions, self.stats.executed_regions));
        out.push_str(&format!("  Coverage ratio:  {:.1}%\n",
            self.stats.coverage_ratio() * 100.0));
        out.push_str(&format!("  Feature kinds:   {}\n\n", self.stats.feature_kinds));

        // Topological analysis
        out.push_str("── Topological Analysis ──\n");
        for (i, beta) in self.betti_numbers.iter().enumerate() {
            let desc = match i {
                0 => "β₀ (components)",
                1 => "β₁ (holes)",
                2 => "β₂ (voids)",
                _ => &format!("β{} ", i),
            };
            out.push_str(&format!("  {} = {}\n", desc, beta));
        }
        out.push_str(&format!("  Euler χ = {}\n", self.gap_analysis.euler_characteristic));
        out.push_str(&format!("  Gap score = {:.1}/100\n\n", self.gap_analysis.gap_score));

        // Missing feature families
        if !self.missing_feature_families.is_empty() {
            out.push_str("── Missing Feature Families ──\n");
            for fam in &self.missing_feature_families {
                out.push_str(&format!("  [ ] {}\n", fam));
            }
            out.push('\n');
        }

        // Priority items
        if !self.priority_items.is_empty() {
            out.push_str("── Priority Ranking ──\n");
            for (i, item) in self.priority_items.iter().enumerate() {
                let cat_str = match item.category {
                    GapCategory::DeadCode => "DEAD",
                    GapCategory::FeatureKindMissing => "MISSING",
                    GapCategory::FeatureKindPartial => "PARTIAL",
                    GapCategory::UntestedComponent => "ISOLATED",
                };
                out.push_str(&format!("  {}. [{:>8}] (p={:.0}) {}\n",
                    i + 1, cat_str, item.priority, item.name));
            }
            out.push('\n');
        }

        // Interpretation
        out.push_str("── Interpretation ──\n");
        if self.gap_analysis.gap_score == 0.0 {
            out.push_str("  Your tests are covering the right things. No gaps found.\n");
        } else if self.gap_analysis.betti_numbers.len() > 2 && self.gap_analysis.betti_numbers[2] > 0 {
            out.push_str("  ⚠  β₂ > 0 means entire feature families are untested.\n");
            out.push_str("     Add tests that exercise these feature types together.\n");
        } else if self.gap_analysis.betti_numbers.len() > 1 && self.gap_analysis.betti_numbers[1] > 0 {
            out.push_str("  ⚠  β₁ > 0 means there are holes in your coverage.\n");
            out.push_str("     Some feature combinations are tested in isolation but never together.\n");
        } else {
            out.push_str("  Coverage looks well-connected. Keep it up!\n");
        }
        out.push_str("═══════════════════════════════════════════════════\n");

        out
    }

    /// Generate a JSON report for editor integration.
    pub fn to_json_report(&self) -> Result<String, serde_json::Error> {
        #[derive(serde::Serialize)]
        struct JsonReport<'a> {
            health: &'a str,
            health_score: u8,
            stats: &'a CoverageStats,
            gap_score: f64,
            betti_numbers: &'a [usize],
            euler_characteristic: i64,
            connected_components: usize,
            missing_feature_families: &'a [String],
            priority_items: Vec<JsonPriorityItem>,
            uncovered_functions: &'a [String],
        }
        #[derive(serde::Serialize)]
        struct JsonPriorityItem {
            name: String,
            priority: f64,
            category: String,
        }

        let json = JsonReport {
            health: self.health.as_str(),
            health_score: self.health.score(),
            stats: &self.stats,
            gap_score: self.gap_analysis.gap_score,
            betti_numbers: &self.betti_numbers,
            euler_characteristic: self.gap_analysis.euler_characteristic,
            connected_components: self.gap_analysis.connected_components,
            missing_feature_families: &self.missing_feature_families,
            priority_items: self
                .priority_items
                .iter()
                .map(|p| JsonPriorityItem {
                    name: p.name.clone(),
                    priority: p.priority,
                    category: format!("{:?}", p.category),
                })
                .collect(),
            uncovered_functions: &self.gap_analysis.uncovered_functions,
        };

        serde_json::to_string_pretty(&json)
    }
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
            line_end: 5,
            total_regions: total,
            executed_regions: executed,
            features: features.into_iter().map(|s| s.to_string()).collect(),
            has_uncovered: executed < total,
        }
    }

    #[test]
    fn test_report_from_healthy_data() {
        let data = CoverageData::from_functions(vec![
            make_fn("a", vec!["code", "branches"], 4, 4),
            make_fn("b", vec!["code", "loops"], 5, 5),
        ]);
        let report = CoverageGapReport::from_coverage_data(&data);
        assert_eq!(report.health, ReportHealth::Healthy);
        assert_eq!(report.health.score(), 100);
    }

    #[test]
    fn test_report_from_uncovered_data() {
        let data = CoverageData::from_functions(vec![
            make_fn("dead", vec!["code"], 3, 0),
        ]);
        let report = CoverageGapReport::from_coverage_data(&data);
        assert_eq!(report.health, ReportHealth::Critical);
        assert_eq!(report.health.score(), 20);
        assert!(!report.priority_items.is_empty());
    }

    #[test]
    fn test_report_from_empty_data() {
        let data = CoverageData::from_functions(vec![]);
        let report = CoverageGapReport::from_coverage_data(&data);
        assert_eq!(report.health, ReportHealth::Unknown);
        assert_eq!(report.health.score(), 0);
    }

    #[test]
    fn test_report_terminal_output() {
        let data = CoverageData::from_functions(vec![
            make_fn("a", vec!["code"], 3, 3),
        ]);
        let report = CoverageGapReport::from_coverage_data(&data);
        let terminal = report.to_terminal_report();
        assert!(terminal.contains("lapce-coverage-gap"));
        assert!(terminal.contains("Coverage Stats"));
        assert!(terminal.contains("Topological Analysis"));
    }

    #[test]
    fn test_report_json_output() {
        let data = CoverageData::from_functions(vec![
            make_fn("a", vec!["code"], 3, 3),
            make_fn("b", vec!["code", "loops"], 5, 0),
        ]);
        let report = CoverageGapReport::from_coverage_data(&data);
        let json = report.to_json_report().unwrap();
        assert!(json.contains("health"));
        assert!(json.contains("betti_numbers"));
        assert!(json.contains("priority_items"));
        assert!(json.contains("missing_feature_families"));
        // Verify it parses back
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("health").is_some());
        assert!(parsed.get("gap_score").is_some());
    }

    #[test]
    fn test_report_betti_numbers_accessible() {
        let data = CoverageData::from_functions(vec![
            make_fn("f1", vec!["a"], 2, 2),
            make_fn("f2", vec!["a", "b"], 3, 3),
        ]);
        let report = CoverageGapReport::from_coverage_data(&data);
        assert!(!report.betti_numbers.is_empty());
        assert_eq!(report.betti_numbers.len(), report.gap_analysis.betti_numbers.len());
    }

    #[test]
    fn test_missing_feature_families_mapped() {
        let data = CoverageData::from_functions(vec![
            make_fn("f1", vec!["code"], 3, 3),
            make_fn("f2", vec!["loops", "code"], 5, 0),
        ]);
        let report = CoverageGapReport::from_coverage_data(&data);
        assert!(report.missing_feature_families.contains(&"loops".to_string()));
    }

    #[test]
    fn test_health_warning_for_holes() {
        // All features have at least one fully-covered function, but there are
        // partially-covered items → Warning
        let data = CoverageData::from_functions(vec![
            make_fn("f1_full", vec!["a"], 2, 2),
            make_fn("f1_partial", vec!["a"], 2, 1),
            make_fn("f2_full", vec!["b"], 2, 2),
        ]);
        let report = CoverageGapReport::from_coverage_data(&data);
        // No uncovered functions, no missing feature kinds, but partial coverage → Warning
        assert_eq!(report.health, ReportHealth::Warning);
    }

    #[test]
    fn test_report_converts_both_formats() {
        let data = CoverageData::from_functions(vec![
            make_fn("a", vec!["code", "branches"], 4, 4),
            make_fn("b", vec!["code", "loops", "generics"], 6, 2),
            make_fn("dead", vec!["unsafe"], 2, 0),
        ]);
        let report = CoverageGapReport::from_coverage_data(&data);
        let json = report.to_json_report().unwrap();
        let terminal = report.to_terminal_report();
        assert!(!json.is_empty());
        assert!(!terminal.is_empty());
    }
}
