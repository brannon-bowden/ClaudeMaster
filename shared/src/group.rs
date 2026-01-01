use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default)]
    pub order: u32,
}

impl Group {
    pub fn new(name: String, parent_id: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            parent_id,
            collapsed: false,
            order: 0,
        }
    }
}
