use std::hint::black_box;
use std::time::{Duration, Instant};

use crate::datagen::{self, GenConfig, Rng};
use crate::recommender::RecommendConfig;

#[derive(Debug, Clone)]
pub struct Scenario {
    pub label: &'static str,
    pub config: GenConfig,
}

#[derive(Debug, Clone)]
pub struct BenchResult {
    pub label: &'static str,
    pub products: usize,
    pub customers: usize,
    pub nodes: usize,
    pub edges: usize,
    pub build: Duration,
    pub lookup_ns: f64,
    pub cust_avg_us: f64,
    pub cust_p95_us: f64,
    pub cust_max_us: f64,
    pub prod_avg_us: f64,
    pub graph_mb: f64,
    pub matrix_mb: f64,
}

pub fn scenarios(full: bool) -> Vec<Scenario> {
    let mut v = vec![
        Scenario {
            label: "XS",
            config: GenConfig::new(1_000, 500, 20),
        },
        Scenario {
            label: "S",
            config: GenConfig::new(10_000, 5_000, 50),
        },
        Scenario {
            label: "M",
            config: GenConfig::new(100_000, 50_000, 200),
        },
        Scenario {
            label: "L",
            config: GenConfig::new(500_000, 250_000, 500),
        },
    ];
    if full {
        v.push(Scenario {
            label: "XL",
            config: GenConfig::new(1_000_000, 500_000, 1_000),
        });
    }
    v
}

fn micros(d: Duration) -> f64 {
    d.as_secs_f64() * 1e6
}

pub fn run(s: &Scenario, queries: usize) -> BenchResult {
    let t = Instant::now();
    let store = datagen::generate(&s.config);
    let build = t.elapsed();
    let cfg = RecommendConfig::default();
    let mut rng = Rng::new(42);

    let lookups = 200_000;
    let t = Instant::now();
    for _ in 0..lookups {
        let id = rng.below(s.config.products as u64) as u32;
        black_box(store.product(black_box(id)));
    }
    let lookup_ns = t.elapsed().as_secs_f64() * 1e9 / lookups as f64;

    let mut times: Vec<f64> = Vec::with_capacity(queries);
    for _ in 0..queries {
        let c = rng.below(s.config.customers as u64) as u32;
        let t = Instant::now();
        let recs = store
            .recommend_for_customer(c, &cfg)
            .expect("cliente existe");
        times.push(micros(t.elapsed()));
        black_box(recs);
    }
    times.sort_by(f64::total_cmp);
    let cust_avg_us = times.iter().sum::<f64>() / times.len() as f64;
    let cust_p95_us = times[((times.len() as f64 * 0.95) as usize).min(times.len() - 1)];
    let cust_max_us = *times.last().unwrap();

    let t = Instant::now();
    for _ in 0..queries {
        let p = rng.below(s.config.products as u64) as u32;
        black_box(
            store
                .recommend_for_product(p, &cfg)
                .expect("produto existe"),
        );
    }
    let prod_avg_us = micros(t.elapsed()) / queries as f64;

    let st = store.stats();
    BenchResult {
        label: s.label,
        products: st.products,
        customers: st.customers,
        nodes: st.nodes,
        edges: st.edges,
        build,
        lookup_ns,
        cust_avg_us,
        cust_p95_us,
        cust_max_us,
        prod_avg_us,
        graph_mb: st.graph_bytes as f64 / 1_048_576.0,
        matrix_mb: st.matrix_bytes as f64 / 1_048_576.0,
    }
}

pub fn csv_header() -> &'static str {
    "cenario,produtos,clientes,vertices,arestas,construcao_ms,consulta_id_ns,rec_cliente_media_us,rec_cliente_p95_us,rec_cliente_max_us,rec_produto_media_us,grafo_mb,matriz_mb"
}

pub fn to_csv_row(r: &BenchResult) -> String {
    format!(
        "{},{},{},{},{},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{:.2},{:.1}",
        r.label,
        r.products,
        r.customers,
        r.nodes,
        r.edges,
        r.build.as_secs_f64() * 1e3,
        r.lookup_ns,
        r.cust_avg_us,
        r.cust_p95_us,
        r.cust_max_us,
        r.prod_avg_us,
        r.graph_mb,
        r.matrix_mb
    )
}

#[derive(Debug, Clone)]
pub struct FanoutResult {
    pub fanout: usize,
    pub avg_us: f64,
    pub overlap_pct: f64,
}

pub fn fanout_ablation(config: &GenConfig, fanouts: &[usize], queries: usize) -> Vec<FanoutResult> {
    let store = datagen::generate(config);
    let mut rng = Rng::new(7);
    let customers: Vec<u32> = (0..queries)
        .map(|_| rng.below(config.customers as u64) as u32)
        .collect();
    let base_cfg = RecommendConfig {
        max_fanout: usize::MAX,
        ..Default::default()
    };
    let reference: Vec<Vec<u32>> = customers
        .iter()
        .map(|&c| {
            store
                .recommend_for_customer(c, &base_cfg)
                .unwrap()
                .into_iter()
                .map(|r| r.product_id)
                .collect()
        })
        .collect();
    fanouts
        .iter()
        .map(|&fanout| {
            let cfg = RecommendConfig {
                max_fanout: fanout,
                ..Default::default()
            };
            let mut total = 0.0;
            let mut hits = 0usize;
            let mut count = 0usize;
            for (i, &c) in customers.iter().enumerate() {
                let t = Instant::now();
                let recs = store.recommend_for_customer(c, &cfg).unwrap();
                total += micros(t.elapsed());
                count += reference[i].len();
                hits += recs
                    .iter()
                    .filter(|r| reference[i].contains(&r.product_id))
                    .count();
            }
            FanoutResult {
                fanout,
                avg_us: total / queries as f64,
                overlap_pct: 100.0 * hits as f64 / count.max(1) as f64,
            }
        })
        .collect()
}
