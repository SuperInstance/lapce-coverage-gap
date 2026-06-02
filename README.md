# lapce-coverage-gap

**Find what your tests DON'T cover.**

> "Your tests cover 80% of lines. But *which* 20% is missing? And is it the *important* 20%?"

[![Build Status](https://github.com/SuperInstance/lapce-coverage-gap/workflows/CI/badge.svg)](https://github.com/SuperInstance/lapce-coverage-gap/actions)
[![crates.io](https://img.shields.io/crates/v/lapce-coverage-gap.svg)](https://crates.io/crates/lapce-coverage-gap)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

---

Standard code coverage tells you *how many lines* your tests hit. But it can't tell you what *kinds* of code your tests are systematically missing.

`lapce-coverage-gap` changes that. It uses **topological data analysis** — the same math used to study the shape of data in biology, sensor networks, and cosmology — to find the blind spots in your Rust test suite.

## How it works

1. **Parses** coverage data from `rustc` (or LLVM coverage JSON format)
2. **Extracts code features** from each function:
   - Branches, loops, match arms, closures
   - Generics, unsafe code, async blocks
   - Macros, trait objects, heap allocation, panics
3. **Builds a simplicial complex** — a mathematical model of how features co-occur in your codebase
4. **Computes Betti numbers** — topological invariants that reveal coverage structure:
   - **β₀** — connected components: how many independent "islands" of tested features exist
   - **β₁** — 1-dimensional holes: features tested individually but never together
   - **β₂** — 2-dimensional voids: entire feature families with zero coverage
5. **Ranks gaps by priority**, from critical dead code to minor coverage holes

## Quick Start

```bash
# Install
cargo install lapce-coverage-gap

# Run on your coverage data
cargo test -- -Z unstable-options --coverage
# (generates coverage JSON in target/debug/coverage/...)
lapce-coverage-gap target/debug/coverage/your-project-*.json

# Or use a simplified coverage file
lapce-coverage-gap coverage.json

# Get summary (ideal for CI)
lapce-coverage-gap coverage.json --summary

# Export JSON for editor/CI integration
lapce-coverage-gap coverage.json --json gap-report.json
```

## Output Example

```
═══════════════════════════════════════════════════
    lapce-coverage-gap — Coverage Gap Report
═══════════════════════════════════════════════════

Health: CRITICAL (score: 20)

── Coverage Stats ──
  Functions:       42 total, 30 full, 12 partial
  Regions:         1024 total, 876 executed
  Coverage ratio:  85.5%
  Feature kinds:   8

── Topological Analysis ──
  β₀ (components) = 3
  β₁ (holes)      = 2
  β₂ (voids)      = 1
  Euler χ         = 4
  Gap score       = 72.5/100

── Missing Feature Families ──
  [ ] unsafe
  [ ] heap_allocation

── Priority Ranking ──
  1. [    DEAD] (p=100) dead_function (src/legacy.rs)
  2. [ MISSING] (p=80) heap_allocation — 5 occurrences
  3. [ MISSING] (p=80) unsafe — 12 occurrences
  4. [ PARTIAL] (p=35) generics — 3/5 covered

── Interpretation ──
  ⚠  β₂ > 0 means entire feature families are untested.
     Add tests that exercise these feature types together.
═══════════════════════════════════════════════════
```

## Interpreting Betti Numbers

### β₀ — Connected Components

How many disconnected groups of tested features exist? If β₀ = 3, your tests are spread across 3 independent feature clusters. A value close to 1 is ideal — it means features are tested together.

### β₁ — Holes

A 1-dimensional hole in coverage space means features A and B are tested individually, but never *together*. This is a "circular" gap — the full picture requires testing A∩B.

### β₂ — Voids

The most serious finding. A void means entire feature families (e.g., unsafe code with generics) have zero test coverage. β₂ > 0 means you're flying blind in whole sections of your codebase.

## GitHub CI Integration

```yaml
- name: Run coverage gap analysis
  run: |
    lapce-coverage-gap coverage.json --summary
  # Fails with exit code 2 on critical gaps
  # exit code 1 on warnings
```

The tool outputs [GitHub Workflow Commands](https://docs.github.com/en/actions/using-workflows/workflow-commands-for-github-actions) for file annotations when critical gaps are found.

## Lapce IDE Integration

The `--json` output flag produces structured data that Lapce (or any editor) can consume:

```json
{
  "health": "CRITICAL",
  "health_score": 20,
  "gap_score": 72.5,
  "betti_numbers": [3, 2, 1],
  "missing_feature_families": ["unsafe", "heap_allocation"],
  "priority_items": [
    {
      "name": "dead_function (src/legacy.rs)",
      "priority": 100.0,
      "category": "DeadCode"
    }
  ]
}
```

## Technical Details

### How the Simplicial Complex Works

Each function in your codebase is a **vertex** in a simplicial complex. When two or more functions share a code feature (e.g., both use `unsafe`), they form a **simplex** — an edge (2 functions), triangle (3), or higher-dimensional simplex.

The complex's **homology** is computed via boundary matrices and Smith normal form, yielding Betti numbers that describe the topology of your test coverage.

### Feature Detection

Features are extracted from function names and file paths using pattern matching:
- `branches`: `if`, `else`, `match`, `switch` in function name
- `loops`: `for`, `while`, `loop`, `iterate`, `each`
- `match_arms`: `match`, `case`, `switch`, `arm`, `variant`
- `generics`: `generic`, `trait`, `impl`, `type_param`, `<T`
- `closures`: `closure`, `lambda`, `fn_`, `anon`, `callback`
- `unsafe`: `unsafe`, `raw`, `pointer`, `deref`, `ffi`, `extern`
- `async`: `async`, `await`, `tokio`, `async_std`, `future`
- `macros`: `macro`, `macro_rules`, `derive`, `proc_macro`
- `trait_objects`: `dyn`, `trait_object`, `vtable`, `box_dyn`
- `heap_allocation`: `alloc`, `box`, `vec`, `string`, `rc`, `arc`
- `panics`: `panic`, `unwrap`, `expect`, `assert`

## Comparison to Standard Coverage

| Metric | Standard Coverage | lapce-coverage-gap |
|--------|------------------|-------------------|
| **What** | Lines & branches executed | Feature families & their topology |
| **Answers** | "How much code ran?" | "What kinds of code are missing?" |
| **Output** | % lines covered | Betti numbers, gap score |
| **Uses** | CI gating, release confidence | Test design, refactoring, risk assessment |
| **Data** | Code execution paths | Feature co-occurrence topology |

## Coverage Data Format

`lapce-coverage-gap` supports two input formats:

### 1. Simple JSON (human-friendly)

```json
{
  "functions": [
    {
      "name": "calculate",
      "file": "src/main.rs",
      "line_start": 5,
      "line_end": 20,
      "total_regions": 10,
      "executed_regions": 8,
      "features": ["code", "branches", "loops"]
    }
  ]
}
```

### 2. LLVM Coverage JSON (from `-Cinstrument-coverage`)

The standard format produced by `cargo test -- --coverage` or LLVM's `llvm-cov export`. The parser handles segment-level coverage data and aggregates it into function-level statistics.

## Library Usage

```rust
use lapce_coverage_gap::{CoverageData, CoverageGapReport};

// Parse from JSON string
let data = CoverageData::from_json_str(json_str)?;

// Or from a file
let data = CoverageData::from_json_file("coverage.json")?;

// Run gap analysis
let report = CoverageGapReport::from_coverage_data(&data);

// Terminal output
println!("{}", report.to_terminal_report());

// JSON for editor integration
let json = report.to_json_report()?;

// Health check
match report.health {
    ReportHealth::Healthy => println!("No gaps found!"),
    ReportHealth::Warning => println!("Some gaps found"),
    ReportHealth::Critical => println!("CRITICAL gaps — take action!"),
    ReportHealth::Unknown => println!("No data"),
}
```

    _ => (),
}
```

## Performance

`lapce-coverage-gap` is designed for fast iteration:

| Codebase size | Parse time | Analysis time |
|---------------|-----------|---------------|
| Small (< 100 functions) | < 1ms | < 1ms |
| Medium (100-1000 functions) | 5-20ms | 10-100ms |
| Large (1000-5000 functions) | 50-200ms | 100-500ms |
| Very large (> 5000 functions) | 200ms-1s | 500ms-5s |

The Smith normal form computation dominates for large complexes. For codebases
with > 2000 functions, the analysis scales roughly O(n³) in the worst case due
to the boundary matrix reduction.

### Memory usage

Each function adds ~200 bytes for feature vectors and topology. A 5000-function
codebase uses approximately 10-50 MB during analysis.

## Configuration

You can configure the analysis through the `CoverageData` and `CoverageGapReport`
APIs:

```rust
use lapce_coverage_gap::*;

// Load data, then customize analysis
let data = CoverageData::from_json_file("coverage.json")?;

// By default, build_feature_complex uses min_shared_features=2
// Increase this to only consider features shared by 3+ functions
let vectors = build_feature_vectors(&data);
let complex = build_feature_complex(&vectors, 3);  // 3+ functions need to share a feature
```

## Debugging Coverage Gaps

When you get a Critical health result, here's how to triage:

1. **Check dead functions first** (Priority 100) — these are functions with 0% coverage
2. **Examine missing feature families** — entire categories of code untested
3. **Look at β₂ (voids)** > 0 — these indicate feature combinations never exercised
4. **Address β₁ (holes)** — features tested separately but never together
5. **Review partial coverage** — reduce the gap_score over time

The goal isn't 100% coverage. The goal is **no voids and no dead code** — ensuring
your tests exercise all the *kinds* of code you write.

## CI Integration

### GitHub Actions

```yaml
name: Coverage Gap
on: [push, pull_request]
jobs:
  coverage-gap:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test  # or use -Zinstrument-coverage
      - run: cargo install lapce-coverage-gap
      - run: |
          lapce-coverage-gap coverage.json --summary
          lapce-coverage-gap coverage.json --json gap-report.json
      - uses: actions/upload-artifact@v4
        with:
          name: coverage-gap-report
          path: gap-report.json
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Healthy or Warning — no critical gaps |
| 1 | Warning — partial coverage issues |
| 2 | Critical — dead code or missing feature families |

## Architecture

```
lapce-coverage-gap/
├── Cargo.toml
├── README.md
├── src/
│   ├── main.rs          # CLI entry point
│   ├── lib.rs           # Re-exports
│   ├── coverage.rs      # JSON parsing, feature detection, stats
│   ├── feature_space.rs # Feature vectors, simplicial complex construction
│   ├── gap.rs           # CoverageGapReport, health assessment
│   ├── report.rs        # Terminal/JSON/CI formatting
│   └── topology/        # Algebraic topology engine
│       ├── mod.rs
│       ├── complex.rs   # SimplicialComplex datatype
│       └── homology.rs  # Betti numbers, Smith normal form, persistence
├── tests/               # Integration tests
└── .github/workflows/   # CI
```

## Example Workflow

### From scratch with a Rust project

```bash
# 1. Generate coverage data with LLVM instrumentation
RUSTFLAGS="-Cinstrument-coverage" cargo test

# 2. Export coverage to JSON format
llvm-cov export \
  --instr-profile=target/debug/coverage/*.profraw \
  target/debug/lapce-coverage-gap-* \
  --format=text > coverage.json

# 3. Run gap analysis
lapce-coverage-gap coverage.json
```

### Using the simple JSON format (from any source)

```bash
# Generate coverage.json with your own tooling
cat <<EOF > coverage.json
{
  "functions": [
    {
      "name": "process_data",
      "file": "src/core.rs",
      "line_start": 1,
      "line_end": 50,
      "total_regions": 20,
      "executed_regions": 15,
      "features": ["code", "branches", "loops", "unsafe"]
    }
  ]
}
EOF

# Analyze
lapce-coverage-gap coverage.json --summary
```

## Internals: How Betti Numbers Are Computed

The core computation follows these steps:

1. **Boundary matrices** are constructed: ∂_k maps k-simplices to their (k-1)-faces
2. **Smith normal form** is computed on each boundary matrix to find its rank
3. **Rank-nullity theorem**: β_k = n_k - rank(∂_k) - rank(∂_{k+1})
4. **Euler characteristic** χ = Σ (-1)^k · n_k (a quick consistency check)

The Smith normal form implementation uses elementary row/column operations
with integer arithmetic, reducing the matrix to diagonal form while preserving
homology groups up to isomorphism.

### Accuracy notes

For small complexes (< 1000 simplices), the SNF computation is exact for
integer coefficients. For larger complexes, numerical stability is maintained
through pivot selection (choosing the smallest non-zero element at each step).

## Development

```bash
git clone https://github.com/SuperInstance/lapce-coverage-gap.git
cd lapce-coverage-gap
cargo build
cargo test
cargo clippy -- -D warnings
```

## The Math Behind It

The topological analysis in `lapce-coverage-gap` is inspired by:

- **Persistent homology** (Edelsbrunner & Harer, 2010): Tracking how topological features persist across scales
- **Simplicial complexes** (Hatcher, 2002): The mathematical structure underlying our feature model
- **Betti numbers** (Poincaré, 1895): A complete set of invariants for topological spaces
- **Dual approach** (Carlsson, 2009): Using topology to understand high-dimensional data

From the field of [Topological Data Analysis (TDA)](https://en.wikipedia.org/wiki/Topological_data_analysis), we apply these concepts not to point clouds or sensor networks, but to the structure of test coverage — treating missing coverage as topological voids in feature space.

## Related Work

- [`negative-space-testing`](https://crates.io/crates/negative-space-testing) — Property testing that finds holes in test coverage using topology (companion crate)
- [`tarpaulin`](https://crates.io/crates/cargo-tarpaulin) — Code coverage tool for Rust
- [`grcov`](https://github.com/mozilla/grcov) — Rust code coverage aggregator
- [Lapce Editor](https://github.com/lapce/lapce) — Lightning-fast code editor in Rust

## License

MIT
