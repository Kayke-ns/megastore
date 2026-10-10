use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::catalog::Catalog;
use crate::error::{Result, StoreError};
use crate::graph::{EdgeKind, Graph, NodeIdx, NodeKey, WeightMode};
use crate::models::{Category, CategoryId, Customer, CustomerId, Product, ProductId};
use crate::recommender::{self, RecommendConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecSource {
    Graph,
    Popular,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Recommendation {
    pub product_id: ProductId,
    pub score: f64,
    pub source: RecSource,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Explanation {
    pub nodes: Vec<NodeKey>,
    pub edges: Vec<EdgeKind>,
    pub strength: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreStats {
    pub products: usize,
    pub customers: usize,
    pub categories: usize,
    pub nodes: usize,
    pub edges: usize,
    pub graph_bytes: usize,
    pub matrix_bytes: u128,
}

#[derive(Debug, Default, Clone)]
pub struct MegaStore {
    catalog: Catalog,
    customers: HashMap<CustomerId, Customer>,
    categories: HashMap<CategoryId, Category>,
    graph: Graph,
    sales: HashMap<ProductId, u64>,
}

fn rating_weight(stars: u8) -> f32 {
    match stars {
        1 => 0.05,
        2 => 0.2,
        3 => 0.6,
        4 => 1.0,
        _ => 1.5,
    }
}

impl MegaStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(products: usize, customers: usize, categories: usize) -> Self {
        Self {
            catalog: Catalog::with_capacity(products, categories),
            customers: HashMap::with_capacity(customers),
            categories: HashMap::with_capacity(categories),
            graph: Graph::with_capacity(products + customers + categories),
            sales: HashMap::with_capacity(products),
        }
    }

    pub fn add_category(&mut self, id: CategoryId, name: impl Into<String>) -> Result<()> {
        if self.categories.contains_key(&id) {
            return Err(StoreError::DuplicateCategory(id));
        }
        self.categories.insert(
            id,
            Category {
                id,
                name: name.into(),
            },
        );
        self.graph.add_node(NodeKey::Category(id));
        Ok(())
    }

    pub fn add_customer(&mut self, id: CustomerId, name: impl Into<String>) -> Result<()> {
        if self.customers.contains_key(&id) {
            return Err(StoreError::DuplicateCustomer(id));
        }
        self.customers.insert(
            id,
            Customer {
                id,
                name: name.into(),
            },
        );
        self.graph.add_node(NodeKey::Customer(id));
        Ok(())
    }

    pub fn add_product(&mut self, product: Product) -> Result<()> {
        let (id, category) = (product.id, product.category);
        let cat_node = self
            .graph
            .node(&NodeKey::Category(category))
            .ok_or(StoreError::CategoryNotFound(category))?;
        self.catalog.insert(product)?;
        let node = self.graph.add_node(NodeKey::Product(id));
        self.graph.upsert_edge(
            node,
            cat_node,
            EdgeKind::BelongsTo,
            1.0,
            WeightMode::Replace,
        );
        Ok(())
    }

    pub fn product(&self, id: ProductId) -> Option<&Product> {
        self.catalog.get(id)
    }

    pub fn customer(&self, id: CustomerId) -> Option<&Customer> {
        self.customers.get(&id)
    }

    pub fn category(&self, id: CategoryId) -> Option<&Category> {
        self.categories.get(&id)
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    pub fn search_products(&self, term: &str) -> Vec<&Product> {
        self.catalog.search_by_name(term)
    }

    pub fn products_in_category(&self, category: CategoryId) -> Vec<&Product> {
        self.catalog
            .in_category(category)
            .iter()
            .filter_map(|id| self.catalog.get(*id))
            .collect()
    }

    pub fn update_price(&mut self, id: ProductId, price_cents: u64) -> Result<()> {
        self.catalog.update_price(id, price_cents)
    }

    pub fn sales_of(&self, id: ProductId) -> u64 {
        self.sales.get(&id).copied().unwrap_or(0)
    }

    pub fn purchased_by(&self, customer: CustomerId) -> Result<Vec<ProductId>> {
        let node = self.customer_node(customer)?;
        let mut ids: Vec<ProductId> = self
            .graph
            .neighbors(node)
            .iter()
            .filter(|e| e.kind == EdgeKind::Purchase)
            .filter_map(|e| match self.graph.key(e.to) {
                NodeKey::Product(p) => Some(p),
                _ => None,
            })
            .collect();
        ids.sort_unstable();
        Ok(ids)
    }

    pub fn record_purchase(
        &mut self,
        customer: CustomerId,
        product: ProductId,
        qty: u32,
    ) -> Result<()> {
        if qty == 0 {
            return Err(StoreError::InvalidQuantity);
        }
        let c = self.customer_node(customer)?;
        let p = self.product_node(product)?;
        let w = 1.0 + (qty as f32).ln();
        self.graph
            .upsert_edge(c, p, EdgeKind::Purchase, w, WeightMode::Accumulate);
        let category = self.catalog.get(product).expect("produto existe").category;
        let cat = self
            .graph
            .node(&NodeKey::Category(category))
            .expect("categoria existe");
        self.graph
            .upsert_edge(p, cat, EdgeKind::BelongsTo, 0.1 * w, WeightMode::Accumulate);
        *self.sales.entry(product).or_insert(0) += qty as u64;
        Ok(())
    }

    pub fn rate_product(
        &mut self,
        customer: CustomerId,
        product: ProductId,
        stars: u8,
    ) -> Result<()> {
        if !(1..=5).contains(&stars) {
            return Err(StoreError::InvalidRating(stars));
        }
        let c = self.customer_node(customer)?;
        let p = self.product_node(product)?;
        self.graph.upsert_edge(
            c,
            p,
            EdgeKind::Rating,
            rating_weight(stars),
            WeightMode::Replace,
        );
        Ok(())
    }

    pub fn add_interest(
        &mut self,
        customer: CustomerId,
        category: CategoryId,
        weight: f32,
    ) -> Result<()> {
        if !(weight > 0.0 && weight <= 1.0) {
            return Err(StoreError::InvalidWeight);
        }
        let c = self.customer_node(customer)?;
        let cat = self
            .graph
            .node(&NodeKey::Category(category))
            .ok_or(StoreError::CategoryNotFound(category))?;
        self.graph
            .upsert_edge(c, cat, EdgeKind::Interest, weight, WeightMode::Replace);
        Ok(())
    }

    pub fn add_similarity(&mut self, a: ProductId, b: ProductId, score: f32) -> Result<()> {
        if a == b {
            return Err(StoreError::SelfLoop);
        }
        if !(score > 0.0 && score <= 1.0) {
            return Err(StoreError::InvalidWeight);
        }
        let na = self.product_node(a)?;
        let nb = self.product_node(b)?;
        self.graph
            .upsert_edge(na, nb, EdgeKind::Similar, score, WeightMode::Replace);
        Ok(())
    }

    pub fn remove_purchase(&mut self, customer: CustomerId, product: ProductId) -> Result<bool> {
        let c = self.customer_node(customer)?;
        let p = self.product_node(product)?;
        Ok(self.graph.remove_edge(c, p, EdgeKind::Purchase))
    }

    pub fn remove_interest(&mut self, customer: CustomerId, category: CategoryId) -> Result<bool> {
        let c = self.customer_node(customer)?;
        let cat = self
            .graph
            .node(&NodeKey::Category(category))
            .ok_or(StoreError::CategoryNotFound(category))?;
        Ok(self.graph.remove_edge(c, cat, EdgeKind::Interest))
    }

    pub fn recommend_for_customer(
        &self,
        customer: CustomerId,
        cfg: &RecommendConfig,
    ) -> Result<Vec<Recommendation>> {
        let start = self.customer_node(customer)?;
        let mut exclude: HashSet<NodeIdx> = HashSet::from([start]);
        exclude.extend(
            self.graph
                .neighbors(start)
                .iter()
                .filter(|e| matches!(e.kind, EdgeKind::Purchase | EdgeKind::Rating))
                .map(|e| e.to),
        );
        let scores = recommender::propagate_scores(&self.graph, start, cfg, &exclude);
        Ok(self.finish(scores, &exclude, cfg))
    }

    pub fn recommend_for_product(
        &self,
        product: ProductId,
        cfg: &RecommendConfig,
    ) -> Result<Vec<Recommendation>> {
        let start = self.product_node(product)?;
        let exclude: HashSet<NodeIdx> = HashSet::from([start]);
        let scores = recommender::propagate_scores(&self.graph, start, cfg, &exclude);
        Ok(self.finish(scores, &exclude, cfg))
    }

    fn finish(
        &self,
        scores: HashMap<NodeIdx, f64>,
        exclude: &HashSet<NodeIdx>,
        cfg: &RecommendConfig,
    ) -> Vec<Recommendation> {
        let ranked = recommender::top_k(&self.graph, scores, cfg.k);
        let mut seen: HashSet<ProductId> = HashSet::with_capacity(cfg.k);
        let mut out = Vec::with_capacity(cfg.k);
        for s in ranked {
            if let NodeKey::Product(pid) = self.graph.key(s.node) {
                if seen.insert(pid) {
                    out.push(Recommendation {
                        product_id: pid,
                        score: s.score,
                        source: RecSource::Graph,
                    });
                }
            }
        }
        if cfg.fill_with_popular && out.len() < cfg.k {
            let mut blocked: HashSet<ProductId> = exclude
                .iter()
                .filter_map(|n| match self.graph.key(*n) {
                    NodeKey::Product(p) => Some(p),
                    _ => None,
                })
                .collect();
            blocked.extend(seen.iter().copied());
            for (pid, _) in self.best_sellers(cfg.k - out.len(), &blocked) {
                out.push(Recommendation {
                    product_id: pid,
                    score: 0.0,
                    source: RecSource::Popular,
                });
            }
        }
        out
    }

    pub fn best_sellers(&self, k: usize, exclude: &HashSet<ProductId>) -> Vec<(ProductId, u64)> {
        if k == 0 {
            return Vec::new();
        }
        let mut heap: BinaryHeap<Reverse<(u64, Reverse<ProductId>)>> =
            BinaryHeap::with_capacity(k + 1);
        for (&pid, &qty) in &self.sales {
            if exclude.contains(&pid) {
                continue;
            }
            heap.push(Reverse((qty, Reverse(pid))));
            if heap.len() > k {
                heap.pop();
            }
        }
        heap.into_sorted_vec()
            .into_iter()
            .map(|Reverse((qty, Reverse(pid)))| (pid, qty))
            .collect()
    }

    pub fn explain(&self, customer: CustomerId, product: ProductId) -> Result<Option<Explanation>> {
        let from = self.customer_node(customer)?;
        let to = self.product_node(product)?;
        let Some((path, strength)) = recommender::best_path(&self.graph, from, to, 3, 32) else {
            return Ok(None);
        };
        let edges = path
            .windows(2)
            .map(|w| {
                self.graph
                    .neighbors(w[0])
                    .iter()
                    .filter(|e| e.to == w[1])
                    .max_by(|a, b| {
                        (a.weight as f64 * recommender::kind_factor(a.kind))
                            .total_cmp(&(b.weight as f64 * recommender::kind_factor(b.kind)))
                    })
                    .map(|e| e.kind)
                    .expect("caminho só usa arestas existentes")
            })
            .collect();
        Ok(Some(Explanation {
            nodes: path.iter().map(|n| self.graph.key(*n)).collect(),
            edges,
            strength,
        }))
    }

    pub fn node_label(&self, key: NodeKey) -> String {
        match key {
            NodeKey::Customer(id) => self
                .customer(id)
                .map(|c| format!("[cliente] {}", c.name))
                .unwrap_or_else(|| format!("[cliente] #{id}")),
            NodeKey::Product(id) => self
                .product(id)
                .map(|p| format!("[produto] {}", p.name))
                .unwrap_or_else(|| format!("[produto] #{id}")),
            NodeKey::Category(id) => self
                .category(id)
                .map(|c| format!("[categoria] {}", c.name))
                .unwrap_or_else(|| format!("[categoria] #{id}")),
        }
    }

    pub fn reach(&self, customer: CustomerId, depth: usize) -> Result<usize> {
        let node = self.customer_node(customer)?;
        Ok(self.graph.dfs_reach(node, depth))
    }

    pub fn stats(&self) -> StoreStats {
        StoreStats {
            products: self.catalog.len(),
            customers: self.customers.len(),
            categories: self.categories.len(),
            nodes: self.graph.node_count(),
            edges: self.graph.edge_count(),
            graph_bytes: self.graph.memory_bytes(),
            matrix_bytes: self.graph.matrix_bytes(),
        }
    }

    fn customer_node(&self, id: CustomerId) -> Result<NodeIdx> {
        self.graph
            .node(&NodeKey::Customer(id))
            .ok_or(StoreError::CustomerNotFound(id))
    }

    fn product_node(&self, id: ProductId) -> Result<NodeIdx> {
        self.graph
            .node(&NodeKey::Product(id))
            .ok_or(StoreError::ProductNotFound(id))
    }
}
