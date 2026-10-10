use std::fs;
use std::io::{self, BufRead, Write};
use std::time::Instant;

use megastore::benchmark;
use megastore::demo::{self, ELETRONICOS, MOVEIS};
use megastore::{EdgeKind, MegaStore, Product, RecSource, RecommendConfig, Recommendation};

const LINE: &str = "──────────────────────────────────────────────────────────────────────────────";

pub fn print_usage() {
    println!("ConectaStore: recomendação de produtos baseada em grafos\n");
    println!("Uso: megastore [comando]\n");
    println!("  demo                   demonstração completa (padrão)");
    println!("  interativo             cadastro, consulta e recomendação pelo terminal");
    println!("  bench [--full] [--csv ARQUIVO] [--queries N]");
    println!("                         testes de desempenho com volumes crescentes");
    println!("  ajuda                  esta mensagem");
}

fn banner() {
    println!("╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("║   ConectaStore  ·  Sistema de Recomendação de Produtos Baseado em Grafos     ║");
    println!("║   MegaStore  ·  Data Structure Strategy and Implementation  ·  Rust          ║");
    println!("╚══════════════════════════════════════════════════════════════════════════════╝");
}

fn section(title: &str) {
    println!("\n▶ {title}");
    println!("{LINE}");
}

fn category_name(store: &MegaStore, p: &Product) -> String {
    store
        .category(p.category)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| "?".into())
}

fn print_product_table(store: &MegaStore, products: &[&Product]) {
    println!(
        "  {:<5} {:<34} {:<18} {:>14} {:>7}",
        "ID", "Produto", "Categoria", "Preço", "Vendas"
    );
    for p in products {
        println!(
            "  {:<5} {:<34} {:<18} {:>14} {:>7}",
            p.id,
            p.name,
            category_name(store, p),
            p.price_brl(),
            store.sales_of(p.id)
        );
    }
}

fn print_recs(store: &MegaStore, recs: &[Recommendation]) {
    if recs.is_empty() {
        println!("  (nenhuma recomendação)");
        return;
    }
    println!(
        "  {:<3} {:<5} {:<34} {:<18} {:>8}  Origem",
        "#", "ID", "Produto", "Categoria", "Score"
    );
    for (i, r) in recs.iter().enumerate() {
        let p = store
            .product(r.product_id)
            .expect("produto recomendado existe");
        let origin = match r.source {
            RecSource::Graph => "grafo",
            RecSource::Popular => "mais vendidos",
        };
        println!(
            "  {:<3} {:<5} {:<34} {:<18} {:>8.4}  {}",
            i + 1,
            p.id,
            p.name,
            category_name(store, p),
            r.score,
            origin
        );
    }
}

fn print_explanation(store: &MegaStore, customer: u32, product: u32) {
    match store.explain(customer, product) {
        Ok(Some(exp)) => {
            for (i, node) in exp.nodes.iter().enumerate() {
                if i == 0 {
                    println!("  {}", store.node_label(*node));
                } else {
                    println!(
                        "     └─({})─▶ {}",
                        exp.edges[i - 1].label(),
                        store.node_label(*node)
                    );
                }
            }
            println!("  força do caminho: {:.4}", exp.strength);
        }
        Ok(None) => println!("  sem caminho em até 3 saltos"),
        Err(e) => println!("  erro: {e}"),
    }
}

fn names_of(store: &MegaStore, ids: &[u32]) -> String {
    ids.iter()
        .filter_map(|id| store.product(*id).map(|p| p.name.clone()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn human_bytes(b: f64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut v = b;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.1} {}", UNITS[i])
}

pub fn run_demo() {
    banner();
    let t = Instant::now();
    let mut store = demo::demo_store();
    let build_us = t.elapsed().as_secs_f64() * 1e6;
    let cfg = RecommendConfig::default();

    section("1. Catálogo de produtos  ·  HashMap<ProductId, Product>");
    let mut all: Vec<&Product> = store.catalog().iter().collect();
    all.sort_by_key(|p| p.id);
    print_product_table(&store, &all);

    section("2. Grafo de relacionamentos  ·  lista de adjacência ponderada");
    let st = store.stats();
    println!(
        "  Vértices: {}  ({} produtos, {} clientes, {} categorias)",
        st.nodes, st.products, st.customers, st.categories
    );
    println!("  Arestas:  {}", st.edges);
    for (kind, n) in store.graph().edge_counts_by_kind() {
        println!("    · {:<10} {:>3}", kind.label(), n);
    }
    println!(
        "  Memória:  lista de adjacência ≈ {}  |  matriz densa seria {}",
        human_bytes(st.graph_bytes as f64),
        human_bytes(st.matrix_bytes as f64)
    );
    println!("  Construído em {build_us:.0} µs");

    section("3. Consultas");
    let t = Instant::now();
    let p = store.product(105).unwrap();
    let ns = t.elapsed().as_nanos();
    println!(
        "  produto(105)        → {} · {} · {}  ({ns} ns)",
        p.name,
        category_name(&store, p),
        p.price_brl()
    );
    let found = store.search_products("livro");
    println!("  buscar(\"livro\")     → {} resultados", found.len());
    print_product_table(&store, &found);
    let moveis = store.products_in_category(MOVEIS);
    println!(
        "  categoria(Móveis)   → {}",
        moveis
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    section("4. Recomendações para a cliente Ana (id 1)");
    println!(
        "  Já comprou: {}",
        names_of(&store, &store.purchased_by(1).unwrap())
    );
    let t = Instant::now();
    let recs = store.recommend_for_customer(1, &cfg).unwrap();
    let us = t.elapsed().as_secs_f64() * 1e6;
    print_recs(&store, &recs);
    println!(
        "  BFS de {} níveis · fan-out {} · calculado em {us:.1} µs",
        cfg.max_depth, cfg.max_fanout
    );

    section("5. Por que recomendamos isso?  ·  DFS com poda (melhor caminho)");
    for r in recs.iter().take(2) {
        print_explanation(&store, 1, r.product_id);
        println!();
    }

    section("6. Quem comprou \"Notebook Gamer X15\" (101) também se interessa por");
    print_recs(&store, &store.recommend_for_product(101, &cfg).unwrap());

    section("7. Cold start: cliente Hugo (id 8), sem compras, interesse em Móveis");
    print_recs(&store, &store.recommend_for_customer(8, &cfg).unwrap());
    let mut fresh = store.clone();
    fresh.add_customer(99, "Visitante").unwrap();
    println!("\n  Visitante novo, sem nenhuma conexão (só mais vendidos):");
    print_recs(&fresh, &fresh.recommend_for_customer(99, &cfg).unwrap());

    section("8. Prevenção de duplicatas");
    let edges_before = store.graph().edge_count();
    store.record_purchase(1, 101, 1).unwrap();
    store.record_purchase(1, 101, 3).unwrap();
    let edges_after = store.graph().edge_count();
    println!("  Ana comprou o Notebook mais 2 vezes → arestas: {edges_before} → {edges_after} (peso acumulado, sem aresta nova)");
    let recs = store
        .recommend_for_customer(1, &RecommendConfig { k: 15, ..cfg })
        .unwrap();
    let mut ids: Vec<u32> = recs.iter().map(|r| r.product_id).collect();
    let total = ids.len();
    ids.sort_unstable();
    ids.dedup();
    let bought = store.purchased_by(1).unwrap();
    let overlap = recs
        .iter()
        .filter(|r| bought.contains(&r.product_id))
        .count();
    println!(
        "  Top-15 para Ana: {total} itens, {} distintos, {overlap} já comprados",
        ids.len()
    );
    match store.add_customer(1, "Ana de novo") {
        Err(e) => println!("  Cadastro repetido rejeitado: {e}"),
        Ok(()) => println!("  (inesperado) cadastro duplicado aceito"),
    }

    section("9. Gerenciamento de conexões");
    store.remove_purchase(1, 501).unwrap();
    println!("  Ana devolveu o \"Café Especial 1kg\" (aresta de compra removida)");
    store.add_interest(1, ELETRONICOS, 1.0).unwrap();
    println!("  Interesse de Ana em Eletrônicos atualizado para 1.0");
    print_recs(&store, &store.recommend_for_customer(1, &cfg).unwrap());
    println!(
        "  DFS: vértices alcançáveis a partir de Ana → 1 salto: {} · 2 saltos: {} · 3 saltos: {}",
        store.reach(1, 1).unwrap(),
        store.reach(1, 2).unwrap(),
        store.reach(1, 3).unwrap()
    );
    println!(
        "  Grafo consistente após as alterações: {}",
        store.graph().is_consistent()
    );
    println!("\n{LINE}\nDemonstração concluída. Rode `cargo run --release -- bench` para os testes de volume.");
}

fn repl_help() {
    println!("Comandos:");
    println!("  produtos                         lista o catálogo");
    println!("  produto <id>                     consulta um produto");
    println!("  buscar <termo>                   busca por nome");
    println!("  cadastrar <id> <cat> <preço> <nome...>   cadastra produto (preço em reais)");
    println!("  cliente <id> <nome...>           cadastra cliente");
    println!("  comprar <cliente> <produto> [qtd]");
    println!("  avaliar <cliente> <produto> <1-5>");
    println!("  interesse <cliente> <categoria> <0-1>");
    println!("  similar <produto> <produto> <0-1>");
    println!("  devolver <cliente> <produto>     remove a compra");
    println!("  recomendar <cliente> [k]");
    println!("  relacionados <produto> [k]");
    println!("  explicar <cliente> <produto>");
    println!("  stats | ajuda | sair");
}

fn parse<T: std::str::FromStr>(s: Option<&&str>) -> Option<T> {
    s.and_then(|v| v.parse().ok())
}

pub fn run_repl() {
    banner();
    println!("Modo interativo. Base de demonstração carregada (digite `ajuda`).\n");
    let mut store = demo::demo_store();
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    loop {
        print!("megastore> ");
        io::stdout().flush().ok();
        let Some(Ok(line)) = lines.next() else { break };
        let parts: Vec<&str> = line.split_whitespace().collect();
        let Some(cmd) = parts.first() else { continue };
        if !io_is_terminal() {
            println!("{line}");
        }
        let cfg = RecommendConfig::default();
        match *cmd {
            "sair" | "exit" => break,
            "ajuda" | "help" => repl_help(),
            "produtos" => {
                let mut all: Vec<&Product> = store.catalog().iter().collect();
                all.sort_by_key(|p| p.id);
                print_product_table(&store, &all);
            }
            "produto" => match parse::<u32>(parts.get(1)).and_then(|id| store.product(id)) {
                Some(p) => print_product_table(&store, &[p]),
                None => println!("produto não encontrado"),
            },
            "buscar" => {
                let term = parts[1..].join(" ");
                let found = store.search_products(&term);
                println!("{} resultado(s)", found.len());
                print_product_table(&store, &found);
            }
            "cadastrar" => {
                let id = parse::<u32>(parts.get(1));
                let cat = parse::<u32>(parts.get(2));
                let price = parts
                    .get(3)
                    .and_then(|v| v.replace(',', ".").parse::<f64>().ok());
                let name = parts.get(4..).map(|n| n.join(" ")).unwrap_or_default();
                match (id, cat, price) {
                    (Some(id), Some(cat), Some(price)) if !name.is_empty() => {
                        let cents = (price * 100.0).round() as u64;
                        match store.add_product(Product::new(id, name, cat, cents)) {
                            Ok(()) => println!("produto {id} cadastrado"),
                            Err(e) => println!("erro: {e}"),
                        }
                    }
                    _ => println!("uso: cadastrar <id> <categoria> <preço> <nome...>"),
                }
            }
            "cliente" => match (parse::<u32>(parts.get(1)), parts.get(2..)) {
                (Some(id), Some(n)) if !n.is_empty() => match store.add_customer(id, n.join(" ")) {
                    Ok(()) => println!("cliente {id} cadastrado"),
                    Err(e) => println!("erro: {e}"),
                },
                _ => println!("uso: cliente <id> <nome...>"),
            },
            "comprar" => match (parse::<u32>(parts.get(1)), parse::<u32>(parts.get(2))) {
                (Some(c), Some(p)) => {
                    let qty = parse::<u32>(parts.get(3)).unwrap_or(1);
                    match store.record_purchase(c, p, qty) {
                        Ok(()) => println!("compra registrada"),
                        Err(e) => println!("erro: {e}"),
                    }
                }
                _ => println!("uso: comprar <cliente> <produto> [qtd]"),
            },
            "avaliar" => match (
                parse::<u32>(parts.get(1)),
                parse::<u32>(parts.get(2)),
                parse::<u8>(parts.get(3)),
            ) {
                (Some(c), Some(p), Some(n)) => match store.rate_product(c, p, n) {
                    Ok(()) => println!("avaliação registrada"),
                    Err(e) => println!("erro: {e}"),
                },
                _ => println!("uso: avaliar <cliente> <produto> <1-5>"),
            },
            "interesse" => match (
                parse::<u32>(parts.get(1)),
                parse::<u32>(parts.get(2)),
                parse::<f32>(parts.get(3)),
            ) {
                (Some(c), Some(cat), Some(w)) => match store.add_interest(c, cat, w) {
                    Ok(()) => println!("interesse registrado"),
                    Err(e) => println!("erro: {e}"),
                },
                _ => println!("uso: interesse <cliente> <categoria> <0-1>"),
            },
            "similar" => match (
                parse::<u32>(parts.get(1)),
                parse::<u32>(parts.get(2)),
                parse::<f32>(parts.get(3)),
            ) {
                (Some(a), Some(b), Some(w)) => match store.add_similarity(a, b, w) {
                    Ok(()) => println!("similaridade registrada"),
                    Err(e) => println!("erro: {e}"),
                },
                _ => println!("uso: similar <produto> <produto> <0-1>"),
            },
            "devolver" => match (parse::<u32>(parts.get(1)), parse::<u32>(parts.get(2))) {
                (Some(c), Some(p)) => match store.remove_purchase(c, p) {
                    Ok(true) => println!("compra removida"),
                    Ok(false) => println!("não havia essa compra"),
                    Err(e) => println!("erro: {e}"),
                },
                _ => println!("uso: devolver <cliente> <produto>"),
            },
            "recomendar" => match parse::<u32>(parts.get(1)) {
                Some(c) => {
                    let k = parse::<usize>(parts.get(2)).unwrap_or(5);
                    let t = Instant::now();
                    match store.recommend_for_customer(c, &RecommendConfig { k, ..cfg }) {
                        Ok(recs) => {
                            let us = t.elapsed().as_secs_f64() * 1e6;
                            if let Some(cust) = store.customer(c) {
                                println!("Recomendações para {} ({us:.1} µs):", cust.name);
                            }
                            print_recs(&store, &recs);
                        }
                        Err(e) => println!("erro: {e}"),
                    }
                }
                None => println!("uso: recomendar <cliente> [k]"),
            },
            "relacionados" => match parse::<u32>(parts.get(1)) {
                Some(p) => {
                    let k = parse::<usize>(parts.get(2)).unwrap_or(5);
                    match store.recommend_for_product(p, &RecommendConfig { k, ..cfg }) {
                        Ok(recs) => print_recs(&store, &recs),
                        Err(e) => println!("erro: {e}"),
                    }
                }
                None => println!("uso: relacionados <produto> [k]"),
            },
            "explicar" => match (parse::<u32>(parts.get(1)), parse::<u32>(parts.get(2))) {
                (Some(c), Some(p)) => print_explanation(&store, c, p),
                _ => println!("uso: explicar <cliente> <produto>"),
            },
            "stats" => {
                let s = store.stats();
                println!(
                    "produtos {} · clientes {} · categorias {} · vértices {} · arestas {}",
                    s.products, s.customers, s.categories, s.nodes, s.edges
                );
                for (kind, n) in store.graph().edge_counts_by_kind() {
                    if kind != EdgeKind::BelongsTo || n > 0 {
                        println!("  {:<10} {n}", kind.label());
                    }
                }
            }
            other => println!("comando desconhecido: {other} (digite `ajuda`)"),
        }
    }
    println!("até logo!");
}

fn io_is_terminal() -> bool {
    use std::io::IsTerminal;
    io::stdin().is_terminal()
}

pub fn run_bench(args: &[String]) {
    let full = args.iter().any(|a| a == "--full");
    let csv_path = args
        .iter()
        .position(|a| a == "--csv")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let queries = args
        .iter()
        .position(|a| a == "--queries")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(500usize);

    banner();
    if cfg!(debug_assertions) {
        println!(
            "⚠ Compilado em modo debug. Use `cargo run --release -- bench` para medições reais."
        );
    }
    section(&format!(
        "Testes de desempenho  ·  {queries} recomendações por cenário"
    ));
    let mut results = Vec::new();
    for s in benchmark::scenarios(full) {
        print!(
            "  [{}] gerando {} produtos, {} clientes, {} categorias... ",
            s.label, s.config.products, s.config.customers, s.config.categories
        );
        io::stdout().flush().ok();
        let r = benchmark::run(&s, queries);
        println!("ok ({:.2} s)", r.build.as_secs_f64());
        results.push(r);
    }

    section("Resultados");
    println!(
        "  {:<4} {:>10} {:>11} {:>9} {:>9} {:>10} {:>10} {:>10} {:>10}",
        "",
        "vértices",
        "arestas",
        "constr.",
        "busca id",
        "rec. méd",
        "rec. p95",
        "rel. prod",
        "memória"
    );
    for r in &results {
        println!(
            "  {:<4} {:>10} {:>11} {:>8.2}s {:>7.0}ns {:>8.1}µs {:>8.1}µs {:>8.1}µs {:>10}",
            r.label,
            r.nodes,
            r.edges,
            r.build.as_secs_f64(),
            r.lookup_ns,
            r.cust_avg_us,
            r.cust_p95_us,
            r.prod_avg_us,
            human_bytes(r.graph_mb * 1_048_576.0)
        );
    }

    section("Lista de adjacência  vs  matriz de adjacência (f32, V²)");
    println!(
        "  {:<4} {:>12} {:>16} {:>14}",
        "", "lista", "matriz", "razão"
    );
    for r in &results {
        println!(
            "  {:<4} {:>12} {:>16} {:>13.0}x",
            r.label,
            human_bytes(r.graph_mb * 1_048_576.0),
            human_bytes(r.matrix_mb * 1_048_576.0),
            r.matrix_mb / r.graph_mb
        );
    }

    let ablation_cfg = benchmark::scenarios(false)[2].config;
    section(&format!(
        "Efeito do corte de fan-out  ·  cenário M ({} produtos)",
        ablation_cfg.products
    ));
    println!(
        "  {:>10} {:>14} {:>22}",
        "fan-out", "rec. média", "top-5 igual ao exato"
    );
    let fanouts = [16, 64, 256, 1024, usize::MAX];
    for r in benchmark::fanout_ablation(&ablation_cfg, &fanouts, 200) {
        let label = if r.fanout == usize::MAX {
            "ilimitado".to_string()
        } else {
            r.fanout.to_string()
        };
        println!(
            "  {:>10} {:>12.1}µs {:>21.1}%",
            label, r.avg_us, r.overlap_pct
        );
    }

    if let Some(path) = csv_path {
        let mut out = String::from(benchmark::csv_header());
        out.push('\n');
        for r in &results {
            out.push_str(&benchmark::to_csv_row(r));
            out.push('\n');
        }
        match fs::write(&path, out) {
            Ok(()) => println!("\n  CSV salvo em {path}"),
            Err(e) => println!("\n  não foi possível salvar o CSV: {e}"),
        }
    }
}
