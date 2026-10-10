use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::graph::{Edge, EdgeKind, Graph, NodeIdx, NodeKey};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecommendConfig {
    pub k: usize,
    pub max_depth: u8,
    pub max_fanout: usize,
    pub depth_decay: f64,
    pub fill_with_popular: bool,
}

impl Default for RecommendConfig {
    fn default() -> Self {
        Self {
            k: 5,
            max_depth: 3,
            max_fanout: 256,
            depth_decay: 0.85,
            fill_with_popular: true,
        }
    }
}

pub fn kind_factor(kind: EdgeKind) -> f64 {
    match kind {
        EdgeKind::Purchase => 1.0,
        EdgeKind::Rating => 1.0,
        EdgeKind::Similar => 0.9,
        EdgeKind::Interest => 0.7,
        EdgeKind::BelongsTo => 0.35,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scored {
    pub node: NodeIdx,
    pub score: f64,
}

pub fn strongest_edges<'a>(edges: &'a [Edge], k: usize, buf: &'a mut Vec<Edge>) -> &'a [Edge] {
    if edges.len() <= k {
        return edges;
    }
    buf.clear();
    buf.extend_from_slice(edges);
    buf.select_nth_unstable_by(k - 1, |a, b| {
        b.weight.total_cmp(&a.weight).then(a.to.cmp(&b.to))
    });
    &buf[..k]
}

pub fn propagate_scores(
    graph: &Graph,
    start: NodeIdx,
    cfg: &RecommendConfig,
    exclude: &HashSet<NodeIdx>,
) -> HashMap<NodeIdx, f64> {
    let mut scores: HashMap<NodeIdx, f64> = HashMap::new();
    let mut visited: HashSet<NodeIdx> = HashSet::new();
    visited.insert(start);
    let mut frontier: Vec<(NodeIdx, f64)> = vec![(start, 1.0)];
    let mut buf: Vec<Edge> = Vec::new();
    let mut level_factor = 1.0;

    for _ in 0..cfg.max_depth {
        let mut next: HashMap<NodeIdx, f64> = HashMap::new();
        for &(node, mass) in &frontier {
            let total = graph.weight_sum(node) as f64;
            if total <= 0.0 {
                continue;
            }
            for e in strongest_edges(graph.neighbors(node), cfg.max_fanout, &mut buf) {
                let m = mass * (e.weight as f64 / total) * kind_factor(e.kind);
                if m <= 0.0 {
                    continue;
                }
                if graph.key(e.to).is_product() && !exclude.contains(&e.to) {
                    *scores.entry(e.to).or_insert(0.0) += m * level_factor;
                }
                if !visited.contains(&e.to) {
                    *next.entry(e.to).or_insert(0.0) += m;
                }
            }
        }
        if next.is_empty() {
            break;
        }
        visited.extend(next.keys().copied());
        frontier = next.into_iter().collect();
        frontier.sort_unstable_by_key(|&(n, _)| n);
        level_factor *= cfg.depth_decay;
    }
    scores
}

#[derive(Debug, PartialEq)]
struct HeapItem {
    score: f64,
    id: u32,
    node: NodeIdx,
}

impl Eq for HeapItem {}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .total_cmp(&other.score)
            .then_with(|| other.id.cmp(&self.id))
    }
}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub fn top_k(graph: &Graph, scores: HashMap<NodeIdx, f64>, k: usize) -> Vec<Scored> {
    if k == 0 {
        return Vec::new();
    }
    let mut heap: BinaryHeap<Reverse<HeapItem>> = BinaryHeap::with_capacity(k + 1);
    for (node, score) in scores {
        if score.is_nan() || score <= 0.0 {
            continue;
        }
        let NodeKey::Product(id) = graph.key(node) else {
            continue;
        };
        heap.push(Reverse(HeapItem { score, id, node }));
        if heap.len() > k {
            heap.pop();
        }
    }
    heap.into_sorted_vec()
        .into_iter()
        .map(|Reverse(item)| Scored {
            node: item.node,
            score: item.score,
        })
        .collect()
}

pub fn best_path(
    graph: &Graph,
    from: NodeIdx,
    to: NodeIdx,
    max_depth: usize,
    fanout: usize,
) -> Option<(Vec<NodeIdx>, f64)> {
    let mut best: Option<(Vec<NodeIdx>, f64)> = None;
    let mut path = vec![from];
    let mut on_path: HashSet<NodeIdx> = HashSet::from([from]);
    dfs_best(
        graph,
        to,
        max_depth,
        fanout,
        1.0,
        &mut path,
        &mut on_path,
        &mut best,
    );
    best
}

#[allow(clippy::too_many_arguments)]
fn dfs_best(
    graph: &Graph,
    target: NodeIdx,
    max_depth: usize,
    fanout: usize,
    mass: f64,
    path: &mut Vec<NodeIdx>,
    on_path: &mut HashSet<NodeIdx>,
    best: &mut Option<(Vec<NodeIdx>, f64)>,
) {
    let node = *path.last().expect("caminho nunca fica vazio");
    if node == target {
        if best.as_ref().is_none_or(|(_, b)| mass > *b) {
            *best = Some((path.clone(), mass));
        }
        return;
    }
    if path.len() > max_depth {
        return;
    }
    let total = graph.weight_sum(node) as f64;
    if total <= 0.0 {
        return;
    }
    let mut buf = Vec::new();
    let edges: Vec<Edge> = strongest_edges(graph.neighbors(node), fanout, &mut buf).to_vec();
    for e in edges {
        if on_path.contains(&e.to) {
            continue;
        }
        let m = mass * (e.weight as f64 / total) * kind_factor(e.kind);
        if best.as_ref().is_some_and(|(_, b)| m <= *b) {
            continue;
        }
        path.push(e.to);
        on_path.insert(e.to);
        dfs_best(graph, target, max_depth, fanout, m, path, on_path, best);
        on_path.remove(&e.to);
        path.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::WeightMode;

    fn collaborative() -> (Graph, Vec<NodeIdx>, Vec<NodeIdx>) {
        let mut g = Graph::new();
        let c: Vec<NodeIdx> = (0..3).map(|i| g.add_node(NodeKey::Customer(i))).collect();
        let p: Vec<NodeIdx> = (0..4).map(|i| g.add_node(NodeKey::Product(i))).collect();
        let buys = [(0, 1), (1, 1), (1, 2), (2, 1), (2, 2), (2, 3)];
        for (ci, pi) in buys {
            g.upsert_edge(
                c[ci],
                p[pi],
                EdgeKind::Purchase,
                1.0,
                WeightMode::Accumulate,
            );
        }
        (g, c, p)
    }

    #[test]
    fn colaborativa_ranqueia_mais_coocorrente_primeiro() {
        let (g, c, p) = collaborative();
        let exclude = HashSet::from([c[0], p[1]]);
        let scores = propagate_scores(&g, c[0], &RecommendConfig::default(), &exclude);
        assert!(
            !scores.contains_key(&p[1]),
            "produto já comprado é excluído"
        );
        assert!(
            !scores.contains_key(&p[0]),
            "produto sem conexão não pontua"
        );
        assert!(
            scores[&p[2]] > scores[&p[3]],
            "p2 foi comprado por 2 vizinhos, p3 por 1"
        );
        let top = top_k(&g, scores, 5);
        assert_eq!(
            top.iter().map(|s| s.node).collect::<Vec<_>>(),
            vec![p[2], p[3]]
        );
    }

    #[test]
    fn profundidade_limita_alcance() {
        let (g, c, p) = collaborative();
        let cfg = RecommendConfig {
            max_depth: 2,
            ..Default::default()
        };
        let exclude = HashSet::from([c[0], p[1]]);
        let scores = propagate_scores(&g, c[0], &cfg, &exclude);
        assert!(scores.is_empty(), "com 2 saltos só chegamos a clientes");
    }

    #[test]
    fn top_k_ordena_e_desempata_por_id() {
        let mut g = Graph::new();
        let nodes: Vec<NodeIdx> = (0..5)
            .map(|i| g.add_node(NodeKey::Product(10 + i)))
            .collect();
        let scores: HashMap<NodeIdx, f64> = [
            (nodes[0], 0.5),
            (nodes[1], 0.9),
            (nodes[2], 0.5),
            (nodes[3], 0.1),
            (nodes[4], 0.0),
        ]
        .into_iter()
        .collect();
        let top = top_k(&g, scores, 3);
        let ids: Vec<NodeIdx> = top.iter().map(|s| s.node).collect();
        assert_eq!(ids, vec![nodes[1], nodes[0], nodes[2]]);
        assert!(top_k(&g, HashMap::new(), 3).is_empty());
    }

    #[test]
    fn fanout_seleciona_arestas_mais_fortes() {
        let edges: Vec<Edge> = (0..10)
            .map(|i| Edge {
                to: i,
                rev: 0,
                weight: i as f32,
                kind: EdgeKind::Similar,
            })
            .collect();
        let mut buf = Vec::new();
        let mut picked: Vec<u32> = strongest_edges(&edges, 3, &mut buf)
            .iter()
            .map(|e| e.to)
            .collect();
        picked.sort();
        assert_eq!(picked, vec![7, 8, 9]);
        assert_eq!(strongest_edges(&edges, 50, &mut buf).len(), 10);
    }

    #[test]
    fn melhor_caminho_explica_recomendacao() {
        let (g, c, p) = collaborative();
        let (path, strength) = best_path(&g, c[0], p[2], 4, 32).unwrap();
        assert_eq!(path.first(), Some(&c[0]));
        assert_eq!(path.last(), Some(&p[2]));
        assert_eq!(path.len(), 4);
        assert!(strength > 0.0 && strength <= 1.0);
        assert!(best_path(&g, c[0], p[0], 4, 32).is_none());
    }
}
