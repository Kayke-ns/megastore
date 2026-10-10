use std::collections::HashSet;

use megastore::datagen::{self, GenConfig};
use megastore::demo::{self, ELETRONICOS, MOVEIS};
use megastore::{EdgeKind, MegaStore, Product, RecSource, RecommendConfig, StoreError};

fn cfg(k: usize) -> RecommendConfig {
    RecommendConfig {
        k,
        ..Default::default()
    }
}

#[test]
fn fluxo_completo_cadastro_compra_recomendacao() {
    let mut s = MegaStore::new();
    s.add_category(1, "Eletrônicos").unwrap();
    s.add_category(2, "Livros").unwrap();
    s.add_product(Product::new(10, "Notebook", 1, 500_000))
        .unwrap();
    s.add_product(Product::new(11, "Mouse", 1, 10_000)).unwrap();
    s.add_product(Product::new(12, "Teclado", 1, 20_000))
        .unwrap();
    s.add_product(Product::new(20, "Livro de Grafos", 2, 9_000))
        .unwrap();
    for (id, n) in [(1, "Ana"), (2, "Bia"), (3, "Caio")] {
        s.add_customer(id, n).unwrap();
    }
    s.record_purchase(1, 10, 1).unwrap();
    s.record_purchase(2, 10, 1).unwrap();
    s.record_purchase(2, 11, 1).unwrap();
    s.record_purchase(3, 10, 1).unwrap();
    s.record_purchase(3, 11, 1).unwrap();
    s.record_purchase(3, 12, 1).unwrap();

    let recs = s.recommend_for_customer(1, &cfg(3)).unwrap();
    assert_eq!(recs[0].product_id, 11, "mouse aparece com 2 coocorrências");
    assert_eq!(recs[1].product_id, 12);
    assert!(
        recs.iter().all(|r| r.product_id != 10),
        "nunca recomenda o que já comprou"
    );
}

#[test]
fn consultas_de_produto() {
    let s = demo::demo_store();
    assert_eq!(s.product(105).unwrap().name, "Monitor 27\" 4K");
    assert!(s.product(9999).is_none());
    assert_eq!(s.search_products("livro").len(), 5);
    let moveis: Vec<u32> = s
        .products_in_category(MOVEIS)
        .iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(moveis, vec![601, 602, 603]);
}

#[test]
fn erros_de_cadastro_e_referencias_invalidas() {
    let mut s = demo::demo_store();
    assert_eq!(
        s.add_product(Product::new(101, "Repetido", ELETRONICOS, 1)),
        Err(StoreError::DuplicateProduct(101))
    );
    assert_eq!(
        s.add_product(Product::new(900, "Sem categoria", 77, 1)),
        Err(StoreError::CategoryNotFound(77))
    );
    assert_eq!(
        s.add_customer(1, "Ana"),
        Err(StoreError::DuplicateCustomer(1))
    );
    assert_eq!(
        s.record_purchase(1, 9999, 1),
        Err(StoreError::ProductNotFound(9999))
    );
    assert_eq!(
        s.record_purchase(404, 101, 1),
        Err(StoreError::CustomerNotFound(404))
    );
    assert_eq!(
        s.record_purchase(1, 101, 0),
        Err(StoreError::InvalidQuantity)
    );
    assert_eq!(s.rate_product(1, 101, 6), Err(StoreError::InvalidRating(6)));
    assert_eq!(s.add_similarity(101, 101, 0.5), Err(StoreError::SelfLoop));
    assert_eq!(s.add_interest(1, 1, 1.5), Err(StoreError::InvalidWeight));
    assert!(s.recommend_for_customer(404, &cfg(5)).is_err());
    assert!(s.recommend_for_product(9999, &cfg(5)).is_err());
}

#[test]
fn recomendacoes_nao_tem_duplicatas_nem_itens_comprados() {
    let s = demo::demo_store();
    for c in 1..=8 {
        let recs = s.recommend_for_customer(c, &cfg(20)).unwrap();
        let ids: HashSet<u32> = recs.iter().map(|r| r.product_id).collect();
        assert_eq!(ids.len(), recs.len(), "cliente {c}: recomendação duplicada");
        let bought: HashSet<u32> = s.purchased_by(c).unwrap().into_iter().collect();
        assert!(
            ids.is_disjoint(&bought),
            "cliente {c}: recomendou item já comprado"
        );
    }
}

#[test]
fn compra_repetida_reforca_aresta_sem_duplicar() {
    let mut s = demo::demo_store();
    let edges = s.graph().edge_count();
    s.record_purchase(1, 101, 1).unwrap();
    s.record_purchase(1, 101, 5).unwrap();
    assert_eq!(s.graph().edge_count(), edges);
    assert_eq!(s.sales_of(101), 3 + 6);
    assert!(s.graph().is_consistent());
}

#[test]
fn pontuacoes_em_ordem_decrescente() {
    let s = demo::demo_store();
    let recs = s.recommend_for_customer(1, &cfg(10)).unwrap();
    let graph_scores: Vec<f64> = recs
        .iter()
        .filter(|r| r.source == RecSource::Graph)
        .map(|r| r.score)
        .collect();
    assert!(graph_scores.windows(2).all(|w| w[0] >= w[1]));
}

#[test]
fn cold_start_usa_interesse_e_mais_vendidos() {
    let mut s = demo::demo_store();
    let hugo = s.recommend_for_customer(8, &cfg(3)).unwrap();
    assert!(hugo
        .iter()
        .all(|r| s.product(r.product_id).unwrap().category == MOVEIS));
    assert!(hugo.iter().all(|r| r.source == RecSource::Graph));

    s.add_customer(50, "Novo").unwrap();
    let novo = s.recommend_for_customer(50, &cfg(3)).unwrap();
    assert_eq!(novo.len(), 3);
    assert!(novo.iter().all(|r| r.source == RecSource::Popular));
    assert_eq!(novo[0].product_id, 501, "café é o mais vendido da base");

    let cfg_sem = RecommendConfig {
        fill_with_popular: false,
        ..cfg(3)
    };
    assert!(s.recommend_for_customer(50, &cfg_sem).unwrap().is_empty());
}

#[test]
fn recomendacao_por_produto() {
    let s = demo::demo_store();
    let recs = s.recommend_for_product(101, &cfg(5)).unwrap();
    let ids: Vec<u32> = recs.iter().map(|r| r.product_id).collect();
    assert!(!ids.contains(&101));
    assert!(ids.contains(&102));
    assert!(ids.contains(&105));
}

#[test]
fn devolucao_remove_conexao_e_libera_recomendacao() {
    let mut s = demo::demo_store();
    let before = s.recommend_for_customer(1, &cfg(20)).unwrap();
    assert!(before.iter().all(|r| r.product_id != 501));
    assert!(s.remove_purchase(1, 501).unwrap());
    assert!(!s.remove_purchase(1, 501).unwrap());
    assert!(!s.purchased_by(1).unwrap().contains(&501));
    let after = s.recommend_for_customer(1, &cfg(20)).unwrap();
    assert!(
        after.iter().any(|r| r.product_id == 501),
        "café volta a ser candidato"
    );
    assert!(s.graph().is_consistent());
}

#[test]
fn interesse_muda_o_ranking() {
    let mut s = demo::demo_store();
    s.add_customer(60, "Teste").unwrap();
    s.add_interest(60, MOVEIS, 1.0).unwrap();
    let a = s.recommend_for_customer(60, &cfg(1)).unwrap();
    assert_eq!(s.product(a[0].product_id).unwrap().category, MOVEIS);
    assert!(s.remove_interest(60, MOVEIS).unwrap());
    s.add_interest(60, ELETRONICOS, 1.0).unwrap();
    let b = s.recommend_for_customer(60, &cfg(1)).unwrap();
    assert_eq!(s.product(b[0].product_id).unwrap().category, ELETRONICOS);
}

#[test]
fn explicacao_comeca_no_cliente_e_termina_no_produto() {
    let s = demo::demo_store();
    let top = s.recommend_for_customer(1, &cfg(1)).unwrap()[0].product_id;
    let exp = s.explain(1, top).unwrap().expect("há caminho");
    assert_eq!(exp.nodes.first(), Some(&megastore::NodeKey::Customer(1)));
    assert_eq!(exp.nodes.last(), Some(&megastore::NodeKey::Product(top)));
    assert_eq!(exp.edges.len(), exp.nodes.len() - 1);
    assert!(exp.strength > 0.0);
}

#[test]
fn estatisticas_e_tipos_de_aresta() {
    let s = demo::demo_store();
    let st = s.stats();
    assert_eq!((st.products, st.customers, st.categories), (20, 8, 6));
    assert_eq!(st.nodes, 34);
    let kinds: std::collections::HashMap<EdgeKind, usize> =
        s.graph().edge_counts_by_kind().into_iter().collect();
    assert_eq!(kinds[&EdgeKind::Purchase], 32);
    assert_eq!(kinds[&EdgeKind::BelongsTo], 20);
    assert_eq!(kinds[&EdgeKind::Similar], 4);
    assert_eq!(kinds[&EdgeKind::Interest], 3);
    assert_eq!(kinds[&EdgeKind::Rating], 6);
    assert!(st.matrix_bytes > 0);
}

#[test]
fn resultado_e_deterministico() {
    let a = demo::demo_store()
        .recommend_for_customer(2, &cfg(8))
        .unwrap();
    let b = demo::demo_store()
        .recommend_for_customer(2, &cfg(8))
        .unwrap();
    assert_eq!(a, b);
}

#[test]
fn volume_sintetico_mantem_garantias() {
    let gen = GenConfig::new(20_000, 8_000, 40);
    let s = datagen::generate(&gen);
    assert!(s.graph().is_consistent());
    for c in (0..8_000).step_by(397) {
        let recs = s.recommend_for_customer(c, &cfg(10)).unwrap();
        assert_eq!(recs.len(), 10);
        let ids: HashSet<u32> = recs.iter().map(|r| r.product_id).collect();
        assert_eq!(ids.len(), 10);
        let bought: HashSet<u32> = s.purchased_by(c).unwrap().into_iter().collect();
        assert!(ids.is_disjoint(&bought));
    }
}
