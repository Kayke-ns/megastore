use std::fmt;

use crate::models::{CategoryId, CustomerId, ProductId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    DuplicateProduct(ProductId),
    DuplicateCustomer(CustomerId),
    DuplicateCategory(CategoryId),
    ProductNotFound(ProductId),
    CustomerNotFound(CustomerId),
    CategoryNotFound(CategoryId),
    InvalidRating(u8),
    InvalidQuantity,
    InvalidWeight,
    SelfLoop,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::DuplicateProduct(id) => write!(f, "produto {id} já cadastrado"),
            StoreError::DuplicateCustomer(id) => write!(f, "cliente {id} já cadastrado"),
            StoreError::DuplicateCategory(id) => write!(f, "categoria {id} já cadastrada"),
            StoreError::ProductNotFound(id) => write!(f, "produto {id} não encontrado"),
            StoreError::CustomerNotFound(id) => write!(f, "cliente {id} não encontrado"),
            StoreError::CategoryNotFound(id) => write!(f, "categoria {id} não encontrada"),
            StoreError::InvalidRating(n) => write!(f, "avaliação {n} inválida (use 1 a 5)"),
            StoreError::InvalidQuantity => write!(f, "quantidade deve ser maior que zero"),
            StoreError::InvalidWeight => write!(f, "peso deve estar no intervalo (0, 1]"),
            StoreError::SelfLoop => write!(f, "um produto não pode ser similar a ele mesmo"),
        }
    }
}

impl std::error::Error for StoreError {}

pub type Result<T> = std::result::Result<T, StoreError>;
