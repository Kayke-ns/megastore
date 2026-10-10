use crate::models::Product;
use crate::store::MegaStore;

pub const ELETRONICOS: u32 = 1;
pub const LIVROS: u32 = 2;
pub const VESTUARIO: u32 = 3;
pub const CASA: u32 = 4;
pub const ALIMENTOS: u32 = 5;
pub const MOVEIS: u32 = 6;

pub fn demo_store() -> MegaStore {
    let mut s = MegaStore::new();
    for (id, name) in [
        (ELETRONICOS, "Eletrônicos"),
        (LIVROS, "Livros"),
        (VESTUARIO, "Vestuário"),
        (CASA, "Casa e Decoração"),
        (ALIMENTOS, "Alimentos"),
        (MOVEIS, "Móveis"),
    ] {
        s.add_category(id, name).unwrap();
    }

    let products = [
        (101, "Notebook Gamer X15", ELETRONICOS, 649_900),
        (102, "Mouse Sem Fio Pro", ELETRONICOS, 19_990),
        (103, "Teclado Mecânico RGB", ELETRONICOS, 34_990),
        (104, "Headset Bluetooth", ELETRONICOS, 29_900),
        (105, "Monitor 27\" 4K", ELETRONICOS, 219_900),
        (106, "Smartphone Z10", ELETRONICOS, 399_900),
        (201, "Livro: Rust na Prática", LIVROS, 11_990),
        (202, "Livro: Algoritmos e Estruturas", LIVROS, 14_990),
        (203, "Livro: Código Limpo", LIVROS, 9_990),
        (204, "Livro: Ficção Científica Vol. 1", LIVROS, 5_990),
        (301, "Camiseta Dev", VESTUARIO, 5_990),
        (302, "Moletom Canguru", VESTUARIO, 14_990),
        (303, "Tênis de Corrida", VESTUARIO, 39_990),
        (401, "Luminária de Mesa LED", CASA, 12_990),
        (402, "Quadro Decorativo", CASA, 8_990),
        (501, "Café Especial 1kg", ALIMENTOS, 7_990),
        (502, "Chá Verde Orgânico", ALIMENTOS, 2_990),
        (601, "Cadeira Ergonômica", MOVEIS, 149_900),
        (602, "Mesa Regulável", MOVEIS, 189_900),
        (603, "Estante de Livros", MOVEIS, 49_900),
    ];
    for (id, name, cat, price) in products {
        s.add_product(Product::new(id, name, cat, price)).unwrap();
    }

    for (id, name) in [
        (1, "Ana"),
        (2, "Bruno"),
        (3, "Carla"),
        (4, "Diego"),
        (5, "Elisa"),
        (6, "Fábio"),
        (7, "Gabi"),
        (8, "Hugo"),
    ] {
        s.add_customer(id, name).unwrap();
    }

    let purchases: [(u32, &[u32]); 7] = [
        (1, &[101, 102, 201, 501]),
        (2, &[101, 102, 103, 105, 601]),
        (3, &[201, 202, 203, 603, 501]),
        (4, &[101, 103, 104, 602, 601]),
        (5, &[301, 302, 303, 502]),
        (6, &[202, 201, 401, 501, 105]),
        (7, &[106, 104, 302, 402]),
    ];
    for (c, items) in purchases {
        for &p in items {
            s.record_purchase(c, p, 1).unwrap();
        }
    }
    s.record_purchase(3, 501, 2).unwrap();

    for (c, p, stars) in [
        (2, 105, 5),
        (4, 601, 5),
        (3, 202, 5),
        (1, 102, 4),
        (6, 401, 4),
        (4, 104, 2),
    ] {
        s.rate_product(c, p, stars).unwrap();
    }

    s.add_interest(1, ELETRONICOS, 0.8).unwrap();
    s.add_interest(1, LIVROS, 0.5).unwrap();
    s.add_interest(8, MOVEIS, 0.9).unwrap();

    for (a, b, sim) in [
        (101, 105, 0.7),
        (201, 202, 0.9),
        (601, 602, 0.8),
        (103, 102, 0.6),
    ] {
        s.add_similarity(a, b, sim).unwrap();
    }
    s
}
