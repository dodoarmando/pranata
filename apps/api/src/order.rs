//! Manual orders: staff put a list in a new order by naming every item once.

use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// A new manual order: every item of the list exactly once, first to last.
#[derive(Deserialize, ToSchema)]
pub struct Order {
    pub(crate) ids: Vec<Uuid>,
}

impl Order {
    /// Whether `ids` holds exactly the items of `old`, each once.
    pub(crate) fn matches(&self, old: &[Uuid]) -> bool {
        let (mut old, mut new) = (old.to_vec(), self.ids.clone());
        old.sort_unstable();
        new.sort_unstable();
        old == new
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_order_names_every_item_exactly_once() {
        let (a, b) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let order = |ids: &[Uuid]| Order { ids: ids.to_vec() };
        assert!(order(&[b, a]).matches(&[a, b]));
        assert!(!order(&[a]).matches(&[a, b]));
        assert!(!order(&[a, a]).matches(&[a, b]));
        assert!(!order(&[a, b, b]).matches(&[a, b]));
    }
}
