use std::collections::HashMap;

use crate::error::{Result, StoreError};
use crate::models::{CategoryId, Product, ProductId};

#[derive(Debug, Default, Clone)]
pub struct Catalog {
    products: HashMap<ProductId, Product>,
    by_category: HashMap<CategoryId, Vec<ProductId>>,
}

impl Catalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(products: usize, categories: usize) -> Self {
        Self {
            products: HashMap::with_capacity(products),
            by_category: HashMap::with_capacity(categories),
        }
    }

    pub fn insert(&mut self, product: Product) -> Result<()> {
        if self.products.contains_key(&product.id) {
            return Err(StoreError::DuplicateProduct(product.id));
        }
        self.by_category
            .entry(product.category)
            .or_default()
            .push(product.id);
        self.products.insert(product.id, product);
        Ok(())
    }

    pub fn get(&self, id: ProductId) -> Option<&Product> {
        self.products.get(&id)
    }

    pub fn contains(&self, id: ProductId) -> bool {
        self.products.contains_key(&id)
    }

    pub fn len(&self) -> usize {
        self.products.len()
    }

    pub fn is_empty(&self) -> bool {
        self.products.is_empty()
    }

    pub fn in_category(&self, category: CategoryId) -> &[ProductId] {
        self.by_category
            .get(&category)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn search_by_name(&self, term: &str) -> Vec<&Product> {
        let term = term.to_lowercase();
        let mut found: Vec<&Product> = self
            .products
            .values()
            .filter(|p| p.name.to_lowercase().contains(&term))
            .collect();
        found.sort_by_key(|p| p.id);
        found
    }

    pub fn update_price(&mut self, id: ProductId, price_cents: u64) -> Result<()> {
        let product = self
            .products
            .get_mut(&id)
            .ok_or(StoreError::ProductNotFound(id))?;
        product.price_cents = price_cents;
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &Product> {
        self.products.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Catalog {
        let mut c = Catalog::new();
        c.insert(Product::new(1, "Notebook Gamer", 10, 650000))
            .unwrap();
        c.insert(Product::new(2, "Mouse sem fio", 10, 19990))
            .unwrap();
        c.insert(Product::new(3, "Livro de Rust", 20, 11990))
            .unwrap();
        c
    }

    #[test]
    fn cadastra_e_consulta_por_id() {
        let c = sample();
        assert_eq!(c.len(), 3);
        assert_eq!(c.get(2).unwrap().name, "Mouse sem fio");
        assert!(c.get(99).is_none());
    }

    #[test]
    fn rejeita_id_duplicado() {
        let mut c = sample();
        let err = c.insert(Product::new(1, "Outro", 10, 1)).unwrap_err();
        assert_eq!(err, StoreError::DuplicateProduct(1));
        assert_eq!(c.len(), 3);
        assert_eq!(
            c.in_category(10).len(),
            2,
            "índice não pode ganhar duplicata"
        );
    }

    #[test]
    fn indice_por_categoria() {
        let c = sample();
        assert_eq!(c.in_category(10), &[1, 2]);
        assert_eq!(c.in_category(20), &[3]);
        assert!(c.in_category(99).is_empty());
    }

    #[test]
    fn busca_por_nome_ignora_maiusculas() {
        let c = sample();
        let r = c.search_by_name("MOUSE");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, 2);
        assert!(c.search_by_name("geladeira").is_empty());
    }

    #[test]
    fn atualiza_preco() {
        let mut c = sample();
        c.update_price(3, 9990).unwrap();
        assert_eq!(c.get(3).unwrap().price_cents, 9990);
        assert_eq!(c.update_price(42, 1), Err(StoreError::ProductNotFound(42)));
    }
}
