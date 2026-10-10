use crate::models::Product;
use crate::store::MegaStore;

#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn below(&mut self, n: u64) -> u64 {
        assert!(n > 0);
        self.next_u64() % n
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenConfig {
    pub products: u32,
    pub customers: u32,
    pub categories: u32,
    pub avg_purchases: u32,
    pub seed: u64,
}

impl GenConfig {
    pub fn new(products: u32, customers: u32, categories: u32) -> Self {
        Self {
            products,
            customers,
            categories,
            avg_purchases: 8,
            seed: 2026,
        }
    }
}

fn pick_in_category(rng: &mut Rng, category: u32, cfg: &GenConfig) -> u32 {
    let per_cat = (cfg.products / cfg.categories).max(1);
    let u = rng.next_f64();
    let j = ((u * u) * per_cat as f64) as u32;
    j.min(per_cat - 1) * cfg.categories + category
}

pub fn generate(cfg: &GenConfig) -> MegaStore {
    assert!(cfg.categories > 0 && cfg.products >= cfg.categories);
    let mut rng = Rng::new(cfg.seed);
    let mut store = MegaStore::with_capacity(
        cfg.products as usize,
        cfg.customers as usize,
        cfg.categories as usize,
    );

    for c in 0..cfg.categories {
        store
            .add_category(c, format!("Categoria {c}"))
            .expect("id único");
    }
    for p in 0..cfg.products {
        let price = 990 + rng.below(500_000);
        store
            .add_product(Product::new(
                p,
                format!("Produto {p}"),
                p % cfg.categories,
                price,
            ))
            .expect("id único");
    }

    let ncat = cfg.categories as u64;
    for c in 0..cfg.customers {
        store
            .add_customer(c, format!("Cliente {c}"))
            .expect("id único");
        let fav1 = rng.below(ncat) as u32;
        let fav2 = rng.below(ncat) as u32;
        store.add_interest(c, fav1, 0.8).unwrap();
        if fav2 != fav1 {
            store.add_interest(c, fav2, 0.4).unwrap();
        }
        let n = 1 + rng.below(2 * cfg.avg_purchases.max(1) as u64);
        for _ in 0..n {
            let r = rng.next_f64();
            let cat = if r < 0.6 {
                fav1
            } else if r < 0.85 {
                fav2
            } else {
                rng.below(ncat) as u32
            };
            let p = pick_in_category(&mut rng, cat, cfg);
            let qty = 1 + rng.below(3) as u32;
            store.record_purchase(c, p, qty).unwrap();
            if rng.next_f64() < 0.3 {
                let stars = [5u8, 5, 4, 4, 3, 2, 1][rng.below(7) as usize];
                store.rate_product(c, p, stars).unwrap();
            }
        }
    }

    for _ in 0..cfg.products / 10 {
        let a = rng.below(cfg.products as u64) as u32;
        let b = pick_in_category(&mut rng, a % cfg.categories, cfg);
        if a != b {
            let score = 0.5 + 0.5 * rng.next_f64() as f32;
            store.add_similarity(a, b, score.min(1.0)).unwrap();
        }
    }
    store
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_e_reprodutivel() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        let x = Rng::new(1).next_f64();
        assert!((0.0..1.0).contains(&x));
    }

    #[test]
    fn gera_volume_pedido() {
        let cfg = GenConfig::new(500, 200, 10);
        let store = generate(&cfg);
        let s = store.stats();
        assert_eq!(s.products, 500);
        assert_eq!(s.customers, 200);
        assert_eq!(s.categories, 10);
        assert_eq!(s.nodes, 710);
        assert!(s.edges > 500 + 200);
        assert!(store.graph().is_consistent());
    }
}
