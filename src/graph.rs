use std::collections::{HashMap, VecDeque};
use std::mem::size_of;

use crate::models::{CategoryId, CustomerId, ProductId};

pub type NodeIdx = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NodeKey {
    Customer(CustomerId),
    Product(ProductId),
    Category(CategoryId),
}

impl NodeKey {
    pub fn is_product(&self) -> bool {
        matches!(self, NodeKey::Product(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    Purchase,
    Rating,
    Interest,
    BelongsTo,
    Similar,
}

impl EdgeKind {
    pub const ALL: [EdgeKind; 5] = [
        EdgeKind::Purchase,
        EdgeKind::Rating,
        EdgeKind::Interest,
        EdgeKind::BelongsTo,
        EdgeKind::Similar,
    ];

    pub fn label(self) -> &'static str {
        match self {
            EdgeKind::Purchase => "compra",
            EdgeKind::Rating => "avaliação",
            EdgeKind::Interest => "interesse",
            EdgeKind::BelongsTo => "categoria",
            EdgeKind::Similar => "similar",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub to: NodeIdx,
    pub rev: u32,
    pub weight: f32,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightMode {
    Accumulate,
    Replace,
}

#[derive(Debug, Default, Clone)]
pub struct Graph {
    keys: Vec<NodeKey>,
    index: HashMap<NodeKey, NodeIdx>,
    adj: Vec<Vec<Edge>>,
    weight_sum: Vec<f32>,
    edge_count: usize,
}

impl Graph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(nodes: usize) -> Self {
        Self {
            keys: Vec::with_capacity(nodes),
            index: HashMap::with_capacity(nodes),
            adj: Vec::with_capacity(nodes),
            weight_sum: Vec::with_capacity(nodes),
            edge_count: 0,
        }
    }

    pub fn add_node(&mut self, key: NodeKey) -> NodeIdx {
        if let Some(&idx) = self.index.get(&key) {
            return idx;
        }
        let idx = self.keys.len() as NodeIdx;
        self.keys.push(key);
        self.index.insert(key, idx);
        self.adj.push(Vec::new());
        self.weight_sum.push(0.0);
        idx
    }

    pub fn node(&self, key: &NodeKey) -> Option<NodeIdx> {
        self.index.get(key).copied()
    }

    pub fn key(&self, idx: NodeIdx) -> NodeKey {
        self.keys[idx as usize]
    }

    pub fn node_count(&self) -> usize {
        self.keys.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edge_count
    }

    pub fn neighbors(&self, idx: NodeIdx) -> &[Edge] {
        &self.adj[idx as usize]
    }

    pub fn degree(&self, idx: NodeIdx) -> usize {
        self.adj[idx as usize].len()
    }

    pub fn weight_sum(&self, idx: NodeIdx) -> f32 {
        self.weight_sum[idx as usize]
    }

    fn find(
        &self,
        a: NodeIdx,
        b: NodeIdx,
        kind: EdgeKind,
    ) -> Option<(NodeIdx, usize, NodeIdx, usize)> {
        let (x, y) = if self.adj[a as usize].len() <= self.adj[b as usize].len() {
            (a, b)
        } else {
            (b, a)
        };
        self.adj[x as usize]
            .iter()
            .position(|e| e.to == y && e.kind == kind)
            .map(|p| (x, p, y, self.adj[x as usize][p].rev as usize))
    }

    pub fn has_edge(&self, a: NodeIdx, b: NodeIdx, kind: EdgeKind) -> bool {
        self.find(a, b, kind).is_some()
    }

    pub fn edge_weight(&self, a: NodeIdx, b: NodeIdx, kind: EdgeKind) -> Option<f32> {
        self.find(a, b, kind)
            .map(|(x, p, _, _)| self.adj[x as usize][p].weight)
    }

    pub fn upsert_edge(
        &mut self,
        a: NodeIdx,
        b: NodeIdx,
        kind: EdgeKind,
        weight: f32,
        mode: WeightMode,
    ) -> bool {
        assert_ne!(a, b, "laços não são permitidos no grafo");
        if let Some((x, px, y, py)) = self.find(a, b, kind) {
            let old = self.adj[x as usize][px].weight;
            let new = match mode {
                WeightMode::Accumulate => old + weight,
                WeightMode::Replace => weight,
            };
            self.adj[x as usize][px].weight = new;
            self.adj[y as usize][py].weight = new;
            let delta = new - old;
            self.weight_sum[x as usize] += delta;
            self.weight_sum[y as usize] += delta;
            false
        } else {
            let pa = self.adj[a as usize].len() as u32;
            let pb = self.adj[b as usize].len() as u32;
            self.adj[a as usize].push(Edge {
                to: b,
                rev: pb,
                weight,
                kind,
            });
            self.adj[b as usize].push(Edge {
                to: a,
                rev: pa,
                weight,
                kind,
            });
            self.weight_sum[a as usize] += weight;
            self.weight_sum[b as usize] += weight;
            self.edge_count += 1;
            true
        }
    }

    pub fn remove_edge(&mut self, a: NodeIdx, b: NodeIdx, kind: EdgeKind) -> bool {
        let Some((x, px, y, py)) = self.find(a, b, kind) else {
            return false;
        };
        let w = self.adj[x as usize][px].weight;
        self.remove_half(x, px);
        self.remove_half(y, py);
        self.weight_sum[x as usize] = (self.weight_sum[x as usize] - w).max(0.0);
        self.weight_sum[y as usize] = (self.weight_sum[y as usize] - w).max(0.0);
        self.edge_count -= 1;
        true
    }

    fn remove_half(&mut self, node: NodeIdx, pos: usize) {
        let list = &mut self.adj[node as usize];
        let last = list.len() - 1;
        list.swap_remove(pos);
        if pos != last {
            let moved = list[pos];
            self.adj[moved.to as usize][moved.rev as usize].rev = pos as u32;
        }
    }

    pub fn edge_counts_by_kind(&self) -> Vec<(EdgeKind, usize)> {
        let mut counts = [0usize; 5];
        for (i, list) in self.adj.iter().enumerate() {
            for e in list {
                if (i as NodeIdx) < e.to {
                    let slot = EdgeKind::ALL.iter().position(|k| *k == e.kind).unwrap();
                    counts[slot] += 1;
                }
            }
        }
        EdgeKind::ALL.iter().copied().zip(counts).collect()
    }

    pub fn shortest_path(
        &self,
        from: NodeIdx,
        to: NodeIdx,
        max_depth: usize,
    ) -> Option<Vec<NodeIdx>> {
        if from == to {
            return Some(vec![from]);
        }
        let mut parent: HashMap<NodeIdx, NodeIdx> = HashMap::new();
        parent.insert(from, from);
        let mut queue = VecDeque::new();
        queue.push_back((from, 0usize));
        while let Some((node, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }
            for e in &self.adj[node as usize] {
                if parent.contains_key(&e.to) {
                    continue;
                }
                parent.insert(e.to, node);
                if e.to == to {
                    let mut path = vec![to];
                    let mut cur = to;
                    while cur != from {
                        cur = parent[&cur];
                        path.push(cur);
                    }
                    path.reverse();
                    return Some(path);
                }
                queue.push_back((e.to, depth + 1));
            }
        }
        None
    }

    pub fn dfs_reach(&self, start: NodeIdx, max_depth: usize) -> usize {
        let mut best: HashMap<NodeIdx, usize> = HashMap::new();
        let mut stack = vec![(start, 0usize)];
        while let Some((node, depth)) = stack.pop() {
            if matches!(best.get(&node), Some(&d) if d <= depth) {
                continue;
            }
            best.insert(node, depth);
            if depth == max_depth {
                continue;
            }
            for e in &self.adj[node as usize] {
                if best.get(&e.to).is_none_or(|&d| d > depth + 1) {
                    stack.push((e.to, depth + 1));
                }
            }
        }
        best.len()
    }

    pub fn memory_bytes(&self) -> usize {
        let keys = self.keys.capacity() * size_of::<NodeKey>();
        let index = self.index.capacity() * (size_of::<NodeKey>() + size_of::<NodeIdx>() + 1);
        let headers = self.adj.capacity() * size_of::<Vec<Edge>>();
        let edges: usize = self
            .adj
            .iter()
            .map(|v| v.capacity() * size_of::<Edge>())
            .sum();
        let sums = self.weight_sum.capacity() * size_of::<f32>();
        keys + index + headers + edges + sums
    }

    pub fn matrix_bytes(&self) -> u128 {
        let n = self.node_count() as u128;
        n * n * size_of::<f32>() as u128
    }

    pub fn is_consistent(&self) -> bool {
        let mut sum_ok = true;
        for (i, list) in self.adj.iter().enumerate() {
            let mut s = 0.0f32;
            for (p, e) in list.iter().enumerate() {
                s += e.weight;
                let twin = match self
                    .adj
                    .get(e.to as usize)
                    .and_then(|l| l.get(e.rev as usize))
                {
                    Some(t) => t,
                    None => return false,
                };
                if twin.to as usize != i
                    || twin.rev as usize != p
                    || twin.kind != e.kind
                    || twin.weight != e.weight
                {
                    return false;
                }
            }
            if (s - self.weight_sum[i]).abs() > 1e-3 * (1.0 + s.abs()) {
                sum_ok = false;
            }
        }
        let half_edges: usize = self.adj.iter().map(Vec::len).sum();
        sum_ok && half_edges == 2 * self.edge_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> (Graph, NodeIdx, NodeIdx, NodeIdx, NodeIdx) {
        let mut g = Graph::new();
        let c = g.add_node(NodeKey::Customer(1));
        let p1 = g.add_node(NodeKey::Product(10));
        let p2 = g.add_node(NodeKey::Product(20));
        let cat = g.add_node(NodeKey::Category(7));
        g.upsert_edge(c, p1, EdgeKind::Purchase, 1.0, WeightMode::Accumulate);
        g.upsert_edge(p1, cat, EdgeKind::BelongsTo, 1.0, WeightMode::Replace);
        g.upsert_edge(p2, cat, EdgeKind::BelongsTo, 1.0, WeightMode::Replace);
        (g, c, p1, p2, cat)
    }

    #[test]
    fn add_node_e_idempotente() {
        let mut g = Graph::new();
        let a = g.add_node(NodeKey::Product(5));
        let b = g.add_node(NodeKey::Product(5));
        assert_eq!(a, b);
        assert_eq!(g.node_count(), 1);
        assert_eq!(g.node(&NodeKey::Product(5)), Some(a));
        assert_eq!(
            g.node(&NodeKey::Customer(5)),
            None,
            "tipos diferentes não colidem"
        );
    }

    #[test]
    fn upsert_nao_duplica_arestas() {
        let (mut g, c, p1, _, _) = tiny();
        assert_eq!(g.edge_count(), 3);
        let created = g.upsert_edge(c, p1, EdgeKind::Purchase, 1.0, WeightMode::Accumulate);
        assert!(!created);
        assert_eq!(g.edge_count(), 3);
        assert_eq!(g.degree(c), 1);
        assert_eq!(g.edge_weight(p1, c, EdgeKind::Purchase), Some(2.0));
        assert!(g.is_consistent());
    }

    #[test]
    fn tipos_diferentes_geram_arestas_distintas() {
        let (mut g, c, p1, _, _) = tiny();
        assert!(g.upsert_edge(c, p1, EdgeKind::Rating, 1.5, WeightMode::Replace));
        assert_eq!(g.degree(c), 2);
        g.upsert_edge(c, p1, EdgeKind::Rating, 0.2, WeightMode::Replace);
        assert_eq!(g.edge_weight(c, p1, EdgeKind::Rating), Some(0.2));
        assert!(g.is_consistent());
    }

    #[test]
    fn remove_aresta_mantem_invariantes() {
        let (mut g, c, p1, p2, cat) = tiny();
        g.upsert_edge(c, p2, EdgeKind::Purchase, 1.0, WeightMode::Accumulate);
        assert!(g.remove_edge(c, p1, EdgeKind::Purchase));
        assert!(
            !g.remove_edge(c, p1, EdgeKind::Purchase),
            "remover de novo não faz nada"
        );
        assert!(!g.has_edge(c, p1, EdgeKind::Purchase));
        assert!(g.has_edge(c, p2, EdgeKind::Purchase));
        assert!(g.has_edge(p1, cat, EdgeKind::BelongsTo));
        assert_eq!(g.edge_count(), 3);
        assert!(g.is_consistent());
    }

    #[test]
    fn remocoes_aleatorias_preservam_ponteiros_rev() {
        let mut g = Graph::new();
        let n = 40u32;
        for i in 0..n {
            g.add_node(NodeKey::Product(i));
        }
        let mut seed = 12345u64;
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as u32
        };
        for _ in 0..400 {
            let a = next() % n;
            let b = next() % n;
            if a != b {
                g.upsert_edge(a, b, EdgeKind::Similar, 0.5, WeightMode::Accumulate);
            }
        }
        for _ in 0..300 {
            let a = next() % n;
            let b = next() % n;
            if a != b {
                g.remove_edge(a, b, EdgeKind::Similar);
            }
            assert!(g.is_consistent());
        }
    }

    #[test]
    fn bfs_encontra_menor_caminho() {
        let (g, c, p1, p2, cat) = tiny();
        let path = g.shortest_path(c, p2, 5).unwrap();
        assert_eq!(path, vec![c, p1, cat, p2]);
        assert!(
            g.shortest_path(c, p2, 2).is_none(),
            "limite de profundidade respeitado"
        );
    }

    #[test]
    fn dfs_conta_alcance_por_profundidade() {
        let (g, c, ..) = tiny();
        assert_eq!(g.dfs_reach(c, 0), 1);
        assert_eq!(g.dfs_reach(c, 1), 2);
        assert_eq!(g.dfs_reach(c, 2), 3);
        assert_eq!(g.dfs_reach(c, 3), 4);
    }

    #[test]
    fn contagem_por_tipo_e_memoria() {
        let (g, ..) = tiny();
        let counts: HashMap<EdgeKind, usize> = g.edge_counts_by_kind().into_iter().collect();
        assert_eq!(counts[&EdgeKind::Purchase], 1);
        assert_eq!(counts[&EdgeKind::BelongsTo], 2);
        assert_eq!(g.matrix_bytes(), 4 * 4 * 4);
        assert!(g.memory_bytes() > 0);
        assert_eq!(size_of::<Edge>(), 16);
    }
}
