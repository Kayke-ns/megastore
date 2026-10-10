pub type ProductId = u32;
pub type CustomerId = u32;
pub type CategoryId = u32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    pub id: ProductId,
    pub name: String,
    pub category: CategoryId,
    pub price_cents: u64,
}

impl Product {
    pub fn new(
        id: ProductId,
        name: impl Into<String>,
        category: CategoryId,
        price_cents: u64,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            category,
            price_cents,
        }
    }

    pub fn price_brl(&self) -> String {
        format_brl(self.price_cents)
    }
}

pub fn format_brl(cents: u64) -> String {
    let reais = (cents / 100).to_string();
    let mut grouped = String::with_capacity(reais.len() + reais.len() / 3);
    for (i, ch) in reais.chars().enumerate() {
        if i > 0 && (reais.len() - i).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(ch);
    }
    format!("R$ {},{:02}", grouped, cents % 100)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Customer {
    pub id: CustomerId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub id: CategoryId,
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formata_precos_em_reais() {
        assert_eq!(format_brl(5), "R$ 0,05");
        assert_eq!(format_brl(19990), "R$ 199,90");
        assert_eq!(format_brl(649900), "R$ 6.499,00");
        assert_eq!(format_brl(123456789), "R$ 1.234.567,89");
    }

    #[test]
    fn produto_expoe_preco_formatado() {
        let p = Product::new(1, "Mouse", 1, 19990);
        assert_eq!(p.price_brl(), "R$ 199,90");
    }
}
