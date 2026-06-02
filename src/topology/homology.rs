//! Homology computation: Betti numbers via Smith normal form.
//!
//! Betti numbers measure the number of topological features:
//! - β₀ = number of connected components
//! - β₁ = number of 1-dimensional holes
//! - β₂ = number of 2-dimensional voids (coverage voids!)
//!
//! We compute β_k = dim(ker ∂_k) - dim(im ∂_{k+1}) using boundary
//! matrices and Smith normal form.

use std::collections::{BTreeSet, HashMap, HashSet};

use super::complex::SimplicialComplex;

/// Compute Smith normal form of an integer matrix.
///
/// Returns the diagonal entries (invariant factors). Rank is the
/// number of non-zero entries.
#[allow(clippy::needless_range_loop, clippy::type_complexity)]
fn smith_normal_form(matrix: &[Vec<i64>], rows: usize, cols: usize) -> Vec<i64> {
    if rows == 0 || cols == 0 {
        return Vec::new();
    }
    let mut m: Vec<Vec<i64>> = matrix.to_vec();
    let min_dim = rows.min(cols);

    for pivot in 0..min_dim {
        // Find smallest non-zero element
        let mut found = false;
        let mut best_row = pivot;
        let mut best_col = pivot;
        let mut best_val = i64::MAX;

        for i in pivot..rows {
            for j in pivot..cols {
                let v = m[i][j].abs();
                if v > 0 && v < best_val {
                    best_val = v;
                    best_row = i;
                    best_col = j;
                    found = true;
                }
            }
        }

        if !found {
            break;
        }

        // Swap rows and columns
        if best_row != pivot {
            m.swap(pivot, best_row);
        }
        if best_col != pivot {
            for i in 0..rows {
                m[i].swap(pivot, best_col);
            }
        }

        // Eliminate
        loop {
            let mut col_clear = true;
            for i in (pivot + 1)..rows {
                if m[i][pivot] != 0 {
                    let q = m[i][pivot] / m[pivot][pivot];
                    for j in pivot..cols {
                        m[i][j] -= q * m[pivot][j];
                    }
                    if m[i][pivot] != 0 {
                        col_clear = false;
                    }
                }
            }

            let mut row_clear = true;
            for j in (pivot + 1)..cols {
                if m[pivot][j] != 0 {
                    let q = m[pivot][j] / m[pivot][pivot];
                    for i in pivot..rows {
                        m[i][j] -= q * m[i][pivot];
                    }
                    if m[pivot][j] != 0 {
                        row_clear = false;
                    }
                }
            }

            if col_clear && row_clear {
                break;
            }
        }

        // Make pivot positive
        if m[pivot][pivot] < 0 {
            for j in 0..cols {
                m[pivot][j] = -m[pivot][j];
            }
        }
    }

    (0..min_dim).map(|i| m[i][i]).collect()
}

impl SimplicialComplex {
    /// Build the boundary matrix ∂_k for dimension k.
    ///
    /// Rows = (k-1)-simplices, Columns = k-simplices.
    /// Entry (i,j) = ±1 if the i-th (k-1)-simplex is a face of the j-th k-simplex.
    #[allow(clippy::type_complexity)]
    fn boundary_matrix(
        &self,
        k: usize,
    ) -> (Vec<Vec<i64>>, Vec<BTreeSet<String>>, Vec<BTreeSet<String>>)
    where
        String: Ord,
    {
        if k == 0 {
            let source = self.simplices_of_dim(0);
            return (Vec::new(), Vec::new(), source);
        }
        let target = self.simplices_of_dim(k - 1);
        let source = self.simplices_of_dim(k);

        if source.is_empty() {
            return (Vec::new(), target, source);
        }

        let n_rows = target.len().max(1);
        let n_cols = source.len();

        let target_idx: HashMap<BTreeSet<String>, usize> = target
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, s)| (s, i))
            .collect();

        let mut matrix = vec![vec![0i64; n_cols]; n_rows];

        for (col, simplex) in source.iter().enumerate() {
            let verts: Vec<_> = simplex.iter().cloned().collect();
            for (face_idx, omitted) in verts.iter().enumerate() {
                let mut face = simplex.clone();
                face.remove(omitted);
                if let Some(&row) = target_idx.get(&face) {
                    matrix[row][col] += if face_idx % 2 == 0 { 1 } else { -1 };
                }
            }
        }

        (matrix, target, source)
    }

    /// Compute the Betti numbers of the simplicial complex.
    ///
    /// β_k = n_k - rank(∂_k) - rank(∂_{k+1})
    ///
    /// Interpretation:
    /// - β₀ = connected components (independent test groups)
    /// - β₁ = 1-dimensional holes (circular coverage gaps)
    /// - β₂ = 2-dimensional voids (untested feature families)
    pub fn betti_numbers(&self) -> Vec<usize> {
        if self.vertices.is_empty() {
            return Vec::new();
        }

        let max_dim = self.max_dim.max(1);
        let mut betti = Vec::new();

        for k in 0..=max_dim {
            let n_k = self.simplex_count(k);
            let n_k_minus_1 = if k > 0 { self.simplex_count(k - 1) } else { 0 };

            // Rank of ∂_k
            let rank_dk = if n_k > 0 && n_k_minus_1 > 0 {
                let (mat, _, _) = self.boundary_matrix(k);
                if mat.is_empty() || mat[0].is_empty() {
                    0
                } else {
                    let snf = smith_normal_form(&mat, mat.len(), mat[0].len());
                    snf.iter().filter(|&&v| v != 0).count()
                }
            } else {
                0
            };

            // Rank of ∂_{k+1}
            let n_k_plus_1 = self.simplex_count(k + 1);
            let rank_dk1 = if n_k_plus_1 > 0 && n_k > 0 {
                let (mat, _, _) = self.boundary_matrix(k + 1);
                if mat.is_empty() || mat.is_empty() || mat.first().is_some_and(|r| r.is_empty()) {
                    0
                } else if !mat.is_empty() && !mat[0].is_empty() {
                    let snf = smith_normal_form(&mat, mat.len(), mat[0].len());
                    snf.iter().filter(|&&v| v != 0).count()
                } else {
                    0
                }
            } else {
                0
            };

            let beta = n_k.saturating_sub(rank_dk).saturating_sub(rank_dk1);
            betti.push(beta);
        }

        betti
    }

    /// Compute persistence diagram via a filtration by first appearance.
    ///
    /// Returns persistence points (birth, death, dimension).
    pub fn persistence_diagram(&self) -> Vec<PersistencePoint> {
        if self.vertices.is_empty() {
            return Vec::new();
        }

        let verts: Vec<String> = self.vertices.iter().cloned().collect();

        // Build filtration: each vertex's filtration value is its index
        let mut filt_val: HashMap<String, f64> = HashMap::new();
        for (i, v) in verts.iter().enumerate() {
            filt_val.insert(v.clone(), i as f64);
        }

        // For each simplex, its filtration value is the max of its vertices
        let mut simplex_birth: Vec<(BTreeSet<String>, f64)> = Vec::new();
        for dim in 0..=self.max_dim {
            for s in self.simplices_of_dim(dim) {
                let birth = s.iter().map(|v| filt_val.get(v).copied().unwrap_or(0.0)).fold(0.0_f64, f64::max);
                simplex_birth.push((s, birth));
            }
        }

        // Track births and deaths per dimension
        let mut births_by_dim: HashMap<usize, Vec<(BTreeSet<String>, f64)>> = HashMap::new();
        for (s, birth) in &simplex_birth {
            let dim = s.len().saturating_sub(1);
            births_by_dim.entry(dim).or_default().push((s.clone(), *birth));
        }

        let mut points = Vec::new();

        // For dimension 0: each vertex is born at its filtration, dies when connected
        let mut d0_births: Vec<(String, f64)> = verts
            .iter()
            .enumerate()
            .map(|(i, v)| (v.clone(), i as f64))
            .collect();
        d0_births.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        // Use union-find to track component merges
        let mut parent: HashMap<String, String> = HashMap::new();
        for (v, _) in &d0_births {
            parent.insert(v.clone(), v.clone());
        }

        fn find_uf(parent: &mut HashMap<String, String>, x: &str) -> String {
            let p = parent.get(x).cloned().unwrap_or_else(|| x.to_string());
            if p == x {
                return p;
            }
            let root = find_uf(parent, &p);
            parent.insert(x.to_string(), root.clone());
            root
        }

        // Process edges in order
        let mut edges: Vec<(String, String, f64)> = Vec::new();
        for edge in self.simplices_of_dim(1) {
            let v: Vec<_> = edge.iter().cloned().collect();
            if v.len() == 2 {
                let birth = v.iter().map(|x| filt_val.get(x).copied().unwrap_or(0.0)).fold(0.0, f64::max);
                edges.push((v[0].clone(), v[1].clone(), birth));
            }
        }
        edges.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());

        let mut alive = HashSet::new();
        // Track which vertex is the "oldest" in each component for death tracking
        let mut oldest_in_component: HashMap<String, (String, f64)> = HashMap::new();
        for (v, b) in &d0_births {
            oldest_in_component.insert(v.clone(), (v.clone(), *b));
            alive.insert(v.clone());
        }

        for (a, b, birth) in &edges {
            if !alive.contains(a) || !alive.contains(b) {
                continue;
            }
            let ra = find_uf(&mut parent, a);
            let rb = find_uf(&mut parent, b);
            if ra != rb {
                // Merge components — the older (earlier birth) absorbs the younger
                let (oldest_a, birth_a) = oldest_in_component.get(&ra).cloned().unwrap_or((ra.clone(), 0.0));
                let (oldest_b, birth_b) = oldest_in_component.get(&rb).cloned().unwrap_or((rb.clone(), 0.0));

                if birth_a <= birth_b {
                    // b's component dies
                    points.push(PersistencePoint {
                        birth: birth_b,
                        death: *birth,
                        dimension: 0,
                    });
                    parent.insert(rb, ra.clone());
                    alive.remove(&oldest_b);
                } else {
                    points.push(PersistencePoint {
                        birth: birth_a,
                        death: *birth,
                        dimension: 0,
                    });
                    parent.insert(ra, rb.clone());
                    alive.remove(&oldest_a);
                }
            }
        }

        // Persistent components (never die)
        for v in &alive {
            let birth_val = oldest_in_component.get(v).map(|(_, b)| *b).unwrap_or(0.0);
            points.push(PersistencePoint {
                birth: birth_val,
                death: f64::INFINITY,
                dimension: 0,
            });
        }

        // For higher dimensions, simplified persistence
        // Each k-simplex for k>=1 creates a feature that might persist
        for dim in 1..=self.max_dim {
            if let Some(simplices) = births_by_dim.get(&dim) {
                for (_, birth) in simplices {
                    // Simplified: assume features persist (this is a reasonable
                    // approximation for the coverage gap use case)
                    points.push(PersistencePoint {
                birth: *birth,
                death: f64::INFINITY,
                dimension: dim,
                    });
                }
            }
        }

        points
    }
}

/// A point in a persistence diagram.
#[derive(Debug, Clone, PartialEq)]
pub struct PersistencePoint {
    pub birth: f64,
    pub death: f64,
    pub dimension: usize,
}

impl PersistencePoint {
    pub fn persistence(&self) -> f64 {
        self.death - self.birth
    }

    pub fn is_persistent(&self) -> bool {
        self.death.is_infinite() && self.death.is_sign_positive()
    }
}
