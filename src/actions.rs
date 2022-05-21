use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct CharacterInventoryData {
    pub bag: HashMap<String, Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mailbox: Option<Vec<String>>,
    pub equip: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank: Option<HashMap<i32, Vec<String>>>,
    pub money: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guild: Option<String>,
    pub faction: String,
    pub race: String,
    pub class: String,
    pub gender: i32,
}

#[derive(Debug)]
pub struct InventorySet {
    pub realm_name: String,
    pub character_name: String,
    pub character_inventory_data: CharacterInventoryData,
}
