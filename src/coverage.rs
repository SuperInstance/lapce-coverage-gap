//! Coverage data parsing and analysis.
//!
//! Parses `rustc` coverage output in JSON format (the "instrument-coverage"
//! format) and represents the data as a set of covered and uncovered regions.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Representation of a single coverage region.
#[derive(Debug, Clone, PartialEq)]
pub struct CoverageRegion {
    /// The file path this region belongs to.
    pub file: String,
    /// Start line (1-based).
    pub line_start: usize,
    /// End line (1-based, inclusive).
    pub line_end: usize,
    /// Whether this region was actually executed.
    pub executed: bool,
    /// The count of executions (0 if not executed).
    pub count: u64,
    /// The kind of coverage region.
    pub kind: RegionKind,
}

/// The kind of code region.
#[derive(Debug, Clone, PartialEq)]
pub enum RegionKind {
    /// Regular code / statement.
    Code,
    /// Branch condition.
    Branch,
    /// Match arm.
    MatchArm,
    /// Loop body.
    Loop,
    /// Closure body.
    Closure,
    /// Generic function / impl.
    Generic,
    /// Unknown or other.
    Other,
}

/// Coverage data for a single function.
#[derive(Debug, Clone)]
pub struct FunctionCoverage {
    /// Function name (mangled).
    pub name: String,
    /// Demangled / display name.
    pub demangled_name: String,
    /// Source file.
    pub file: String,
    /// Start line.
    pub line_start: usize,
    /// End line.
    pub line_end: usize,
    /// Total regions in this function.
    pub total_regions: usize,
    /// Regions that were executed.
    pub executed_regions: usize,
    /// The set of feature kinds found in this function.
    pub features: BTreeSet<String>,
    /// Whether the function has any uncovered regions.
    pub has_uncovered: bool,
}

impl FunctionCoverage {
    /// Coverage ratio for this function (0.0 - 1.0).
    pub fn coverage_ratio(&self) -> f64 {
        if self.total_regions == 0 {
            1.0
        } else {
            self.executed_regions as f64 / self.total_regions as f64
        }
    }

    /// True if this function is fully covered.
    pub fn is_fully_covered(&self) -> bool {
        self.executed_regions == self.total_regions
    }
}

/// Collection of coverage data across an entire crate.
#[derive(Debug, Clone)]
pub struct CoverageData {
    /// All functions with their coverage info.
    pub functions: Vec<FunctionCoverage>,
    /// Feature-key presence across all functions.
    pub feature_presence: BTreeMap<String, usize>,
    /// Overall stats.
    pub stats: CoverageStats,
}

/// Aggregate coverage statistics.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CoverageStats {
    /// Total functions analyzed.
    pub total_functions: usize,
    /// Functions with any uncovered region.
    pub partial_functions: usize,
    /// Fully covered functions.
    pub full_functions: usize,
    /// Total regions across all functions.
    pub total_regions: usize,
    /// Executed regions across all functions.
    pub executed_regions: usize,
    /// Unique feature kinds found across the codebase.
    pub feature_kinds: usize,
}

impl CoverageStats {
    /// Overall line/region coverage ratio.
    pub fn coverage_ratio(&self) -> f64 {
        if self.total_regions == 0 {
            1.0
        } else {
            self.executed_regions as f64 / self.total_regions as f64
        }
    }
}

impl CoverageData {
    /// Parse coverage data from a `rustc` JSON export file.
    ///
    /// The expected format is the JSON output from `-Zinstrument-coverage`
    /// with `--emit=llvm-ir,link` and `-Cinstrument-coverage`, then processed
    /// through `llvm-cov export --format=text`.
    ///
    /// For testing purposes, also accepts a simplified format (see test data).
    pub fn from_json_file(path: impl AsRef<Path>) -> Result<Self, CoverageError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| CoverageError::Io(e.to_string()))?;
        Self::from_json_str(&content)
    }

    /// Parse from a JSON string.
    pub fn from_json_str(json: &str) -> Result<Self, CoverageError> {
        // Try the full llvm-cov export format first
        if let Ok(data) = parse_llvm_cov_json(json) {
            return Ok(data);
        }
        // Fall back to the simplified test format
        parse_simple_json(json)
    }

    /// Build from pre-processed function list (for programmatic use).
    pub fn from_functions(functions: Vec<FunctionCoverage>) -> Self {
        let mut feature_presence: BTreeMap<String, usize> = BTreeMap::new();
        let mut total_regions = 0;
        let mut executed_regions = 0;
        let mut partial = 0;
        let mut full = 0;

        for f in &functions {
            for feat in &f.features {
                *feature_presence.entry(feat.clone()).or_insert(0) += 1;
            }
            total_regions += f.total_regions;
            executed_regions += f.executed_regions;
            if f.has_uncovered {
                partial += 1;
            } else {
                full += 1;
            }
        }

        let stats = CoverageStats {
            total_functions: functions.len(),
            partial_functions: partial,
            full_functions: full,
            total_regions,
            executed_regions,
            feature_kinds: feature_presence.len(),
        };

        CoverageData {
            functions,
            feature_presence,
            stats,
        }
    }

    /// Get the set of functions that are fully uncovered (0% coverage).
    pub fn uncovered_functions(&self) -> Vec<&FunctionCoverage> {
        self.functions
            .iter()
            .filter(|f| f.executed_regions == 0 && f.total_regions > 0)
            .collect()
    }

    /// Find feature kinds that have no test coverage at all.
    pub fn uncovered_feature_kinds(&self) -> Vec<String> {
        let mut result = Vec::new();
        for feature in self.feature_presence.keys() {
            let covered = self
                .functions
                .iter()
                .filter(|f| f.features.contains(feature) && !f.has_uncovered)
                .count();
            if covered == 0 {
                result.push(feature.clone());
            }
        }
        result
    }
}

/// Error type for coverage parsing.
#[derive(Debug)]
pub enum CoverageError {
    Io(String),
    Parse(String),
    Format(String),
}

impl std::fmt::Display for CoverageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoverageError::Io(msg) => write!(f, "I/O error: {msg}"),
            CoverageError::Parse(msg) => write!(f, "parse error: {msg}"),
            CoverageError::Format(msg) => write!(f, "format error: {msg}"),
        }
    }
}

impl std::error::Error for CoverageError {}

// ─── Parse helpers ───────────────────────────────────────────────────

/// Parse the full llvm-cov JSON export format.
fn parse_llvm_cov_json(json: &str) -> Result<CoverageData, CoverageError> {
    #[derive(serde::Deserialize)]
    struct LlvmCovExport {
        data: Vec<LlvmCovData>,
    }
    #[derive(serde::Deserialize)]
    struct LlvmCovData {
        files: Vec<LlvmCovFile>,
    }
    #[allow(dead_code)]
    #[derive(serde::Deserialize)]
    struct LlvmCovFile {
        filename: String,
        #[serde(default)]
        segments: Vec<LlvmCovSegment>,
        #[serde(default)]
        expansions: Vec<LlvmCovExpansion>,
        #[serde(default)]
        summary: LlvmCovSummary,
    }
    #[allow(dead_code)]
    #[derive(serde::Deserialize)]
    struct LlvmCovSegment {
        line: usize,
        #[serde(default)]
        col: usize,
        #[serde(default)]
        count: u64,
        #[serde(default)]
        has_count: bool,
        #[serde(default)]
        is_region_entry: bool,
    }
    #[allow(dead_code)]
    #[derive(serde::Deserialize)]
    struct LlvmCovExpansion {
        #[serde(default)]
        target: String,
    }
    #[allow(dead_code)]
    #[derive(serde::Deserialize, Default)]
    struct LlvmCovSummary {
        #[serde(default)]
        lines: LlvmCovLineSummary,
        #[serde(default)]
        functions: LlvmCovFuncSummary,
    }
    #[allow(dead_code)]
    #[derive(serde::Deserialize, Default)]
    struct LlvmCovLineSummary {
        #[serde(default)]
        count: usize,
        #[serde(default)]
        covered: usize,
    }
    #[allow(dead_code)]
    #[derive(serde::Deserialize, Default)]
    struct LlvmCovFuncSummary {
        #[serde(default)]
        count: usize,
        #[serde(default)]
        covered: usize,
    }

    let export: LlvmCovExport =
        serde_json::from_str(json).map_err(|e| CoverageError::Parse(e.to_string()))?;

    let mut functions = Vec::new();
    let mut all_regions = 0usize;
    let mut all_executed = 0usize;
    let mut feature_presence: BTreeMap<String, usize> = BTreeMap::new();

    for file_data in export.data.iter().flat_map(|d| &d.files) {
        let filename = &file_data.filename;

        // Group segments into "regions" by contiguous line ranges
        let mut regions: Vec<CoverageRegion> = Vec::new();
        let mut i = 0;
        while i < file_data.segments.len() {
            let seg = &file_data.segments[i];
            if !seg.has_count {
                i += 1;
                continue;
            }
            // Find contiguous segment block
            let mut end_line = seg.line;
            let mut j = i + 1;
            while j < file_data.segments.len() {
                let next = &file_data.segments[j];
                if next.has_count {
                    end_line = next.line;
                }
                if next.is_region_entry || !next.has_count {
                    break;
                }
                j += 1;
            }
            let kind = infer_region_kind(seg.count);
            regions.push(CoverageRegion {
                file: filename.clone(),
                line_start: seg.line,
                line_end: end_line,
                executed: seg.count > 0,
                count: seg.count,
                kind,
            });
            i = j.max(i + 1);
        }

        // Build function from file summary
        for _ in 0..file_data.summary.functions.count.max(1) {
            let total = file_data.summary.lines.count;
            let covered = file_data.summary.lines.covered;

            let features = detect_features(filename, &regions);
            let has_uncovered = total > covered;

            let fc = FunctionCoverage {
                name: filename.rsplit('/').next().unwrap_or(filename).to_string(),
                demangled_name: filename.clone(),
                file: filename.clone(),
                line_start: 1,
                line_end: total.max(1),
                total_regions: total,
                executed_regions: covered,
                features: features.clone(),
                has_uncovered,
            };

            for feat in &features {
                *feature_presence.entry(feat.clone()).or_insert(0) += 1;
            }
            functions.push(fc);
            all_regions += total;
            all_executed += covered;
        }
    }

    Ok(CoverageData {
        stats: CoverageStats {
            total_functions: functions.len(),
            partial_functions: functions.iter().filter(|f| f.has_uncovered).count(),
            full_functions: functions.iter().filter(|f| !f.has_uncovered).count(),
            total_regions: all_regions,
            executed_regions: all_executed,
            feature_kinds: feature_presence.len(),
        },
        functions,
        feature_presence,
    })
}

/// Parse the simplified JSON format (used in tests).
fn parse_simple_json(json: &str) -> Result<CoverageData, CoverageError> {
    #[derive(serde::Deserialize)]
    struct SimpleCoverage {
        functions: Vec<SimpleFunction>,
    }
    #[derive(serde::Deserialize)]
    struct SimpleFunction {
        name: String,
        file: String,
        line_start: usize,
        line_end: usize,
        total_regions: usize,
        executed_regions: usize,
        features: Vec<String>,
    }

    let sc: SimpleCoverage =
        serde_json::from_str(json).map_err(|e| CoverageError::Parse(e.to_string()))?;

    let mut functions = Vec::new();
    let mut feature_presence: BTreeMap<String, usize> = BTreeMap::new();

    for sf in sc.functions {
        let has_uncovered = sf.executed_regions < sf.total_regions;
        let features: BTreeSet<String> = sf.features.into_iter().collect();
        for feat in &features {
            *feature_presence.entry(feat.clone()).or_insert(0) += 1;
        }
        functions.push(FunctionCoverage {
            name: sf.name.clone(),
            demangled_name: sf.name,
            file: sf.file,
            line_start: sf.line_start,
            line_end: sf.line_end,
            total_regions: sf.total_regions,
            executed_regions: sf.executed_regions,
            features,
            has_uncovered,
        });
    }

    let total = functions.len();
    let partial = functions.iter().filter(|f| f.has_uncovered).count();
    let full_regions: usize = functions.iter().map(|f| f.total_regions).sum();
    let exec_regions: usize = functions.iter().map(|f| f.executed_regions).sum();

    Ok(CoverageData {
        stats: CoverageStats {
            total_functions: total,
            partial_functions: partial,
            full_functions: total - partial,
            total_regions: full_regions,
            executed_regions: exec_regions,
            feature_kinds: feature_presence.len(),
        },
        functions,
        feature_presence,
    })
}

// ─── Feature detection ───────────────────────────────────────────────

/// Infer the region kind from segment data.
#[allow(dead_code)]
fn infer_region_kind(_count: u64) -> RegionKind {
    // In a more sophisticated implementation, this would inspect the
    // expanded macro data to determine region kinds.
    RegionKind::Code
}

/// Detect features present in a file based on regions.
fn detect_features(filename: &str, regions: &[CoverageRegion]) -> BTreeSet<String> {
    let mut features = BTreeSet::new();
    features.insert("code".to_string());

    if regions.is_empty() {
        return features;
    }

    // Heuristic detection based on filename patterns and region info
    let lower = filename.to_lowercase();

    if lower.contains("branch") || lower.contains("cond") || lower.contains("if_") {
        features.insert("branches".to_string());
    }
    if lower.contains("loop") || lower.contains("for_") || lower.contains("while") {
        features.insert("loops".to_string());
    }
    if lower.contains("match") || lower.contains("arm") {
        features.insert("match_arms".to_string());
    }
    if lower.contains("generic") || lower.contains("trait") || lower.contains("impl") {
        features.insert("generics".to_string());
    }
    if lower.contains("closure") || lower.contains("lambda") || lower.contains("fn_") {
        features.insert("closures".to_string());
    }
    if lower.contains("unsafe") || lower.contains("raw") {
        features.insert("unsafe".to_string());
    }
    if lower.contains("async") || lower.contains("await") || lower.contains("future") {
        features.insert("async".to_string());
    }
    if lower.contains("macro") {
        features.insert("macros".to_string());
    }
    if lower.contains("unsized") || lower.contains("dyn") || lower.contains("trait_obj") {
        features.insert("trait_objects".to_string());
    }
    if lower.contains("alloc") || lower.contains("box") || lower.contains("rc") || lower.contains("arc") {
        features.insert("heap_allocation".to_string());
    }
    if lower.contains("panic") || lower.contains("unwrap") || lower.contains("expect") {
        features.insert("panics".to_string());
    }

    features
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_coverage() {
        let data = CoverageData::from_functions(vec![]);
        assert_eq!(data.stats.total_functions, 0);
        assert_eq!(data.stats.total_regions, 0);
    }

    #[test]
    fn test_single_covered_function() {
        let f = FunctionCoverage {
            name: "foo".into(),
            demangled_name: "foo".into(),
            file: "src/lib.rs".into(),
            line_start: 1,
            line_end: 10,
            total_regions: 5,
            executed_regions: 5,
            features: ["code".into(), "branches".into()].into(),
            has_uncovered: false,
        };
        let data = CoverageData::from_functions(vec![f]);
        assert_eq!(data.stats.full_functions, 1);
        assert_eq!(data.stats.partial_functions, 0);
        assert_eq!(data.stats.total_functions, 1);
        assert_eq!(data.feature_presence.len(), 2);
        assert!(data.uncovered_functions().is_empty());
    }

    #[test]
    fn test_partial_coverage() {
        let f = FunctionCoverage {
            name: "bar".into(),
            demangled_name: "bar".into(),
            file: "src/bar.rs".into(),
            line_start: 1,
            line_end: 20,
            total_regions: 10,
            executed_regions: 6,
            features: ["code".into(), "loops".into()].into(),
            has_uncovered: true,
        };
        let data = CoverageData::from_functions(vec![f]);
        assert_eq!(data.stats.full_functions, 0);
        assert_eq!(data.stats.partial_functions, 1);
        assert_eq!(data.stats.coverage_ratio(), 0.6);
    }

    #[test]
    fn test_coverage_ratio_and_uncovered() {
        let f1 = FunctionCoverage {
            name: "full_fn".into(),
            demangled_name: "full_fn".into(),
            file: "src/lib.rs".into(),
            line_start: 1,
            line_end: 5,
            total_regions: 3,
            executed_regions: 3,
            features: ["code".into()].into(),
            has_uncovered: false,
        };
        let f2 = FunctionCoverage {
            name: "partial_fn".into(),
            demangled_name: "partial_fn".into(),
            file: "src/lib.rs".into(),
            line_start: 10,
            line_end: 20,
            total_regions: 8,
            executed_regions: 4,
            features: ["branches".into()].into(),
            has_uncovered: true,
        };
        let data = CoverageData::from_functions(vec![f1, f2]);
        assert_eq!(data.stats.coverage_ratio(), 7.0 / 11.0);
    }

    #[test]
    fn test_uncovered_feature_kinds() {
        let f1 = FunctionCoverage {
            name: "fn1".into(),
            demangled_name: "fn1".into(),
            file: "src/a.rs".into(),
            line_start: 1,
            line_end: 5,
            total_regions: 2,
            executed_regions: 2,
            features: ["code".into(), "branches".into()].into(),
            has_uncovered: false,
        };
        let f2 = FunctionCoverage {
            name: "fn2".into(),
            demangled_name: "fn2".into(),
            file: "src/b.rs".into(),
            line_start: 10,
            line_end: 15,
            total_regions: 3,
            executed_regions: 0,
            features: ["loops".into()].into(),
            has_uncovered: true,
        };
        let data = CoverageData::from_functions(vec![f1, f2]);
        let uncovered = data.uncovered_feature_kinds();
        assert!(uncovered.contains(&"loops".to_string()));
        assert!(!uncovered.contains(&"code".to_string()));
    }

    #[test]
    fn test_parse_simple_json() {
        let json = r#"{
            "functions": [
                {
                    "name": "do_foo",
                    "file": "src/lib.rs",
                    "line_start": 1,
                    "line_end": 10,
                    "total_regions": 8,
                    "executed_regions": 6,
                    "features": ["code", "branches"]
                },
                {
                    "name": "do_bar",
                    "file": "src/lib.rs",
                    "line_start": 12,
                    "line_end": 25,
                    "total_regions": 5,
                    "executed_regions": 5,
                    "features": ["code", "loops", "closures"]
                }
            ]
        }"#;
        let data = CoverageData::from_json_str(json).unwrap();
        assert_eq!(data.stats.total_functions, 2);
        assert_eq!(data.stats.full_functions, 1);
        assert_eq!(data.stats.partial_functions, 1);
        assert_eq!(data.stats.feature_kinds, 4);
        assert!(data.feature_presence.contains_key("code"));
        assert!(data.feature_presence.contains_key("branches"));
        assert!(data.feature_presence.contains_key("loops"));
        assert!(data.feature_presence.contains_key("closures"));
    }

    #[test]
    fn test_feature_detection_from_filename() {
        let regions = vec![CoverageRegion {
            file: "src/branch_check.rs".into(),
            line_start: 1,
            line_end: 5,
            executed: true,
            count: 3,
            kind: RegionKind::Code,
        }];
        let features = detect_features("src/branch_check.rs", &regions);
        assert!(features.contains("branches"));
        assert!(features.contains("code"));
    }

    #[test]
    fn test_feature_detection_empty() {
        let features = detect_features("src/util.rs", &[]);
        assert_eq!(features.len(), 1);
        assert!(features.contains("code"));
    }

    #[test]
    fn test_function_coverage_one_fn_fully_covered() {
        let mut f = FunctionCoverage {
            name: "covered".into(),
            demangled_name: "covered".into(),
            file: "src/lib.rs".into(),
            line_start: 1,
            line_end: 3,
            total_regions: 5,
            executed_regions: 5,
            features: ["code".into()].into(),
            has_uncovered: false,
        };
        assert!(f.is_fully_covered());
        assert!((f.coverage_ratio() - 1.0).abs() < f64::EPSILON);

        f.executed_regions = 3;
        assert!(!f.is_fully_covered());
        assert!((f.coverage_ratio() - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_stats_coverage_ratio_empty() {
        let stats = CoverageStats {
            total_functions: 0,
            partial_functions: 0,
            full_functions: 0,
            total_regions: 0,
            executed_regions: 0,
            feature_kinds: 0,
        };
        assert!((stats.coverage_ratio() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_invalid_json() {
        let result = CoverageData::from_json_str("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_uncovered_functions_list() {
        let f1 = FunctionCoverage {
            name: "dead_fn".into(),
            demangled_name: "dead_fn".into(),
            file: "src/dead.rs".into(),
            line_start: 1,
            line_end: 5,
            total_regions: 4,
            executed_regions: 0,
            features: ["code".into()].into(),
            has_uncovered: true,
        };
        let f2 = FunctionCoverage {
            name: "live_fn".into(),
            demangled_name: "live_fn".into(),
            file: "src/live.rs".into(),
            line_start: 1,
            line_end: 5,
            total_regions: 4,
            executed_regions: 4,
            features: ["code".into()].into(),
            has_uncovered: false,
        };
        let data = CoverageData::from_functions(vec![f1, f2]);
        let uncovered = data.uncovered_functions();
        assert_eq!(uncovered.len(), 1);
        assert_eq!(uncovered[0].name, "dead_fn");
    }
}
