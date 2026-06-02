//! Report formatting utilities for terminal and JSON output.
//!
//! Provides convenience functions for rendering `CoverageGapReport`
//! to stdout, JSON files, and minimal one-line summaries.

use crate::gap::{CoverageGapReport, ReportHealth};

/// Write a terminal report to stdout.
pub fn print_terminal_report(report: &CoverageGapReport) {
    print!("{}", report.to_terminal_report());
}

/// Write JSON report to a file.
pub fn write_json_report(report: &CoverageGapReport, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
    let json = report
        .to_json_report()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    std::fs::write(path.as_ref(), json)
}

/// Generate a minimal one-line summary suitable for status bars / CI.
pub fn one_line_summary(report: &CoverageGapReport) -> String {
    let health = report.health.as_str();
    let gap = report.gap_analysis.gap_score;
    let coverage = report.stats.coverage_ratio() * 100.0;
    let missing = report.missing_feature_families.len();
    let dead = report.gap_analysis.uncovered_functions.len();

    let betti = if report.betti_numbers.len() >= 3 {
        format!("β0={} β1={} β2={}", report.betti_numbers[0], report.betti_numbers[1], report.betti_numbers[2])
    } else if report.betti_numbers.len() >= 2 {
        format!("β0={} β1={}", report.betti_numbers[0], report.betti_numbers[1])
    } else if !report.betti_numbers.is_empty() {
        format!("β0={}", report.betti_numbers[0])
    } else {
        "β=none".to_string()
    };

    format!(
        "[{health}] cov={coverage:.0}% gap={gap:.0}/100 {betti} missing={missing} dead={dead}"
    )
}

/// Generate a summary suitable for GitHub CI annotations.
pub fn ci_annotation(report: &CoverageGapReport) -> Vec<String> {
    let mut annotations = Vec::new();

    if report.health == ReportHealth::Critical {
        annotations.push(format!(
            "::error title=lapce-coverage-gap::Coverage gaps critical: {} prio items, {} missing feature families",
            report.priority_items.len(),
            report.missing_feature_families.len(),
        ));
    } else if report.health == ReportHealth::Warning {
        annotations.push(format!(
            "::warning title=lapce-coverage-gap::Coverage gaps detected: gap score {:.1}/100",
            report.gap_analysis.gap_score,
        ));
    }

    for item in &report.priority_items {
        annotations.push(item.to_ci_annotation());
    }

    annotations
}

/// CI annotation for a priority item.
#[derive(Debug)]
pub struct PriorityItemCi {
    pub file: String,
    pub line: usize,
    pub message: String,
    pub severity: CiSeverity,
}

#[derive(Debug, PartialEq)]
pub enum CiSeverity {
    Error,
    Warning,
    Notice,
}

impl crate::PriorityItem {
    /// Convert a priority item to a CI annotation.
    pub fn to_ci_annotation(&self) -> String {
        use crate::feature_space::GapCategory;
        let (severity, file, line) = match &self.category {
            GapCategory::DeadCode => {
                let parts: Vec<&str> = self.name.splitn(2, " (").collect();
                let file_part = parts.get(1).unwrap_or(&")").trim_end_matches(')');
                ("error", file_part.to_string(), 1_usize)
            }
            GapCategory::FeatureKindMissing => ("warning", "src/lib.rs".to_string(), 1),
            GapCategory::FeatureKindPartial => ("notice", "src/lib.rs".to_string(), 1),
            GapCategory::UntestedComponent => ("warning", "src/lib.rs".to_string(), 1),
        };

        format!("::{severity} file={file},line={line},title=CoverageGap::{message}",
            severity = severity,
            file = file,
            line = line,
            message = self.name,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coverage::{CoverageData, FunctionCoverage};

    fn make_report(executed_ratios: Vec<(usize, usize)>) -> CoverageGapReport {
        let fns: Vec<FunctionCoverage> = executed_ratios
            .into_iter()
            .enumerate()
            .map(|(i, (total, executed))| {
                let name = format!("fn_{}", i);
                FunctionCoverage {
                    name: name.clone(),
                    demangled_name: name,
                    file: "src/lib.rs".to_string(),
                    line_start: 1,
                    line_end: 5,
                    total_regions: total,
                    executed_regions: executed,
                    features: ["code".into()].into(),
                    has_uncovered: executed < total,
                }
            })
            .collect();
        let data = CoverageData::from_functions(fns);
        CoverageGapReport::from_coverage_data(&data)
    }

    #[test]
    fn test_one_line_summary_healthy() {
        let report = make_report(vec![(3, 3)]);
        let summary = one_line_summary(&report);
        assert!(summary.contains("HEALTHY"));
        assert!(summary.contains("β0="));
    }

    #[test]
    fn test_one_line_summary_critical() {
        let report = make_report(vec![(3, 0)]);
        let summary = one_line_summary(&report);
        assert!(summary.contains("CRITICAL"));
    }

    #[test]
    fn test_ci_annotation_critical() {
        let report = make_report(vec![(3, 0)]);
        let annotations = ci_annotation(&report);
        assert!(annotations.iter().any(|a| a.starts_with("::error")));
    }

    #[test]
    fn test_ci_annotation_healthy() {
        let report = make_report(vec![(3, 3)]);
        let annotations = ci_annotation(&report);
        assert_eq!(annotations.len(), 0);
    }

    #[test]
    fn test_write_json_report() {
        let report = make_report(vec![(3, 3)]);
        let path = "/tmp/lapce-coverage/test-report.json";
        write_json_report(&report, path).unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("health"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_print_terminal_report() {
        let report = make_report(vec![(3, 3)]);
        let buf = report.to_terminal_report();
        assert!(buf.contains("lapce-coverage-gap"));
    }

    #[test]
    fn test_priority_item_annotation() {
        use crate::feature_space::{GapCategory, PriorityItem};
        let item = PriorityItem {
            name: "dead_fn (src/dead.rs)".to_string(),
            priority: 100.0,
            category: GapCategory::DeadCode,
        };
        let annotation = item.to_ci_annotation();
        assert!(annotation.starts_with(":"));
        assert!(annotation.contains("error"));
        assert!(annotation.contains("CoverageGap"));
    }

    #[test]
    fn test_report_health_color() {
        assert_eq!(ReportHealth::Healthy.color_code(), "\x1b[32m");
        assert_eq!(ReportHealth::Warning.color_code(), "\x1b[33m");
        assert_eq!(ReportHealth::Critical.color_code(), "\x1b[31m");
        assert_eq!(ReportHealth::Unknown.color_code(), "\x1b[90m");
    }
}
