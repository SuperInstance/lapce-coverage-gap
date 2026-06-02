//! Simplicial complex for code feature analysis.
//!
//! A simplicial complex is a collection of simplices (generalized triangles)
//! that is closed under taking faces. Here:
//! - Vertices (0-simplices) = individual functions
//! - Edges (1-simplices) = pairs of functions sharing a feature
//! - Triangles (2-simplices) = triples sharing features
//! - Higher simplices = groups sharing common feature sets

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// An abstract simplicial complex built from code feature data.
///
/// Vertices are named code features or functions. A k-simplex is a set
/// of k+1 vertices that were all tested together.
#[derive(Debug, Clone)]
pub struct SimplicialComplex {
    /// All simplices, keyed by dimension.
    pub(crate) simplices: BTreeMap<usize, HashSet<BTreeSet<String>>>,
    /// All vertices.
    pub(crate) vertices: HashSet<String>,
    /// Maximum dimension.
    pub(crate) max_dim: usize,
}

impl SimplicialComplex {
    /// Create a new empty simplicial complex.
    pub fn new() -> Self {
        Self {
            simplices: BTreeMap::new(),
            vertices: HashSet::new(),
            max_dim: 0,
        }
    }

    /// Add a simplex (set of vertices) to the complex.
    ///
    /// Automatically adds all faces to maintain the closure property.
    pub fn add_simplex(&mut self, vertices: BTreeSet<String>) {
        if vertices.is_empty() {
            return;
        }
        let dim = vertices.len().saturating_sub(1);
        self.max_dim = self.max_dim.max(dim);

        // Add the simplex itself
        self.simplices.entry(dim).or_default().insert(vertices.clone());

        // Add all vertices
        for v in &vertices {
            self.vertices.insert(v.clone());
        }

        // Add all faces recursively
        if vertices.len() >= 2 {
            let verts: Vec<_> = vertices.iter().cloned().collect();
            for i in 0..verts.len() {
                let mut face = BTreeSet::new();
                for (j, v) in verts.iter().enumerate() {
                    if j != i {
                        face.insert(v.clone());
                    }
                }
                self.add_simplex(face);
            }
        }
    }

    /// Build a complex from feature traces.
    ///
    /// Each trace is a set of assertion/feature names checked together.
    pub fn from_traces(traces: &[Vec<String>]) -> Self {
        let mut complex = Self::new();
        for trace in traces {
            let simplex: BTreeSet<String> = trace.iter().cloned().collect();
            complex.add_simplex(simplex);
        }
        complex
    }

    /// Get the dimension (max simplex dimension).
    pub fn dimension(&self) -> usize {
        self.max_dim
    }

    /// Count simplices of a given dimension.
    pub fn simplex_count(&self, dim: usize) -> usize {
        self.simplices.get(&dim).map_or(0, |s| s.len())
    }

    /// Total number of unique simplices across all dimensions.
    pub fn total_simplex_count(&self) -> usize {
        (0..=self.max_dim).map(|d| self.simplex_count(d)).sum()
    }

    /// Number of unique vertices.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Get all simplices of a given dimension.
    pub fn simplices_of_dim(&self, dim: usize) -> Vec<BTreeSet<String>> {
        self.simplices
            .get(&dim)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Check if the complex contains a specific simplex.
    pub fn contains(&self, simplex: &BTreeSet<String>) -> bool {
        if simplex.is_empty() {
            return false;
        }
        let dim = simplex.len() - 1;
        self.simplices
            .get(&dim)
            .is_some_and(|ss| ss.contains(simplex))
    }

    /// Compute the boundary of a simplex.
    pub fn boundary(&self, simplex: &BTreeSet<String>) -> Vec<BTreeSet<String>> {
        if simplex.len() <= 1 {
            return Vec::new();
        }
        let verts: Vec<_> = simplex.iter().cloned().collect();
        let mut faces = Vec::new();
        for i in 0..verts.len() {
            let mut face = BTreeSet::new();
            for (j, v) in verts.iter().enumerate() {
                if j != i {
                    face.insert(v.clone());
                }
            }
            faces.push(face);
        }
        faces
    }

    /// Compute the Euler characteristic.
    ///
    /// χ = Σ (-1)^k · (number of k-simplices)
    pub fn euler_characteristic(&self) -> i64 {
        let mut chi: i64 = 0;
        for k in 0..=self.max_dim {
            let count = self.simplex_count(k) as i64;
            if k % 2 == 0 {
                chi += count;
            } else {
                chi -= count;
            }
        }
        chi
    }

    /// Compute connected components using union-find.
    pub fn connected_components(&self) -> usize {
        if self.vertices.is_empty() {
            return 0;
        }

        let mut parent: HashMap<String, String> = HashMap::new();
        for v in &self.vertices {
            parent.insert(v.clone(), v.clone());
        }

        fn find(parent: &mut HashMap<String, String>, x: &str) -> String {
            let p = parent.get(x).cloned().unwrap_or_else(|| x.to_string());
            if p == x {
                return p;
            }
            let root = find(parent, &p);
            parent.insert(x.to_string(), root.clone());
            root
        }

        // Union vertices connected by edges
        for edge in self.simplices_of_dim(1) {
            let verts: Vec<_> = edge.iter().cloned().collect();
            if verts.len() == 2 {
                let a = find(&mut parent, &verts[0]);
                let b = find(&mut parent, &verts[1]);
                if a != b {
                    parent.insert(a, b);
                }
            }
        }

        // Union across higher-dimensional simplices
        for dim in 2..=self.max_dim {
            for simplex in self.simplices_of_dim(dim) {
                let verts: Vec<_> = simplex.iter().cloned().collect();
                if verts.len() >= 2 {
                    let root = find(&mut parent, &verts[0]);
                    for v in &verts[1..] {
                        let v_root = find(&mut parent, v);
                        if root != v_root {
                            parent.insert(v_root, root.clone());
                        }
                    }
                }
            }
        }

        let mut roots = HashSet::new();
        for v in &self.vertices {
            roots.insert(find(&mut parent, v));
        }
        roots.len()
    }
}

impl Default for SimplicialComplex {
    fn default() -> Self {
        Self::new()
    }
}
