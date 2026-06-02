//! lapce-coverage-gap — CLI tool for topological coverage gap analysis.
//!
//! Analyzes Rust coverage data and finds structural gaps in your test suite
//! using topological data analysis (simplicial complexes, Betti numbers).

use std::path::PathBuf;
use std::process;

use lapce_coverage_gap::CoverageData;
use lapce_coverage_gap::CoverageGapReport;
use lapce_coverage_gap::report::{print_terminal_report, write_json_report, one_line_summary};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let program = args
        .first()
        .map(|s| s.rsplit('/').next().unwrap_or(s))
        .unwrap_or("lapce-coverage-gap");

    if args.len() < 2 {
        eprintln!("Usage: {} <coverage.json> [--json <out.json>]", program);
        eprintln!();
        eprintln!("Analyzes rustc coverage JSON and finds topological gaps.");
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  <coverage.json>    Path to coverage data (llvm-cov JSON format)");
        eprintln!("  --json <out.json>  Also write JSON report to this file");
        eprintln!("  --summary          Print one-line summary only");
        process::exit(1);
    }

    let coverage_path = PathBuf::from(&args[1]);
    let mut json_output: Option<PathBuf> = None;
    let mut summary_only = false;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {
                i += 1;
                if i < args.len() {
                    json_output = Some(PathBuf::from(&args[i]));
                }
            }
            "--summary" => {
                summary_only = true;
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                process::exit(1);
            }
        }
        i += 1;
    }

    // Parse coverage data
    let coverage_data = match CoverageData::from_json_file(&coverage_path) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error reading coverage data from '{}': {}", coverage_path.display(), e);
            process::exit(1);
        }
    };

    // Run gap analysis
    let report = CoverageGapReport::from_coverage_data(&coverage_data);

    // Output
    if summary_only {
        println!("{}", one_line_summary(&report));
    } else {
        print_terminal_report(&report);
    }

    // JSON output
    if let Some(json_path) = json_output {
        match write_json_report(&report, &json_path) {
            Ok(()) => {
                if !summary_only {
                    println!("JSON report written to '{}'", json_path.display());
                }
            }
            Err(e) => {
                eprintln!("Error writing JSON report: {}", e);
                process::exit(1);
            }
        }
    }

    // Exit with code based on health
    match report.health {
        lapce_coverage_gap::ReportHealth::Critical => process::exit(2),
        lapce_coverage_gap::ReportHealth::Warning => process::exit(1),
        _ => process::exit(0),
    }
}
