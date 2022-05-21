use anyhow::{Context, Result};
use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::actions;

#[derive(Debug)]
pub struct Realm {
    pub characters: Vec<String>,
}

#[derive(Debug)]
pub struct Account {
    pub name: String,
    pub realms: HashMap<String, Realm>,
    pub dir: PathBuf,
}

impl Account {
    pub fn bagsync_db_path(&self) -> PathBuf {
        self.dir.join("SavedVariables").join("BagSyncString.lua")
    }

    // Update this accounts inventory database with changes from inventory_setters
    pub fn update(
        &self,
        olua: Option<Lua>,
        inventory_setters: &[actions::InventorySet],
    ) -> Result<Lua> {
        let bagsync_db_path = self.bagsync_db_path();
        // Read full db from self.dir/SavedVariables/BagSyncString.lua, set from inventory_setters, save
        // in place
        let lua: Lua = match olua {
            Some(l) => l,
            None => {
                let l = Lua::new();
                let db_contents = fs::read_to_string(&bagsync_db_path)
                    .context(format!("Reading db for {}", self.name))?;
                l.load(&db_contents).exec()?;
                l
            }
        };
        {
            let mut bagsync_db: HashMap<String, Value> = (&lua.globals()).get("BagSyncDB")?;

            for inventory_set in inventory_setters {
                let inventory_data_value: Value =
                    lua.to_value(&inventory_set.character_inventory_data)?;
                bagsync_db
                    .entry(inventory_set.realm_name.to_string())
                    .and_modify(|realm_value| {
                        let realm_table: Table = lua.unpack(realm_value.clone()).unwrap();
                        realm_table
                            .set(
                                inventory_set.character_name.to_string(),
                                inventory_data_value,
                            )
                            .unwrap();
                    });
            }
            let bagsync_db_string = serde_lua_table::to_string_pretty(&bagsync_db)?;
            let mut data: String = "BagSyncDB = ".to_string();
            data.push_str(&bagsync_db_string);
            fs::write(&bagsync_db_path, data)
                .context(format!("Failed writing to {}", bagsync_db_path.display()))?;
        }

        Ok(lua)
    }

    pub fn update_from(&self, lua: Lua, other_account: &Account) -> Result<Lua> {
        // Read character inventories from other_account and update lua
        log::info!("update_from {} -> {}", self.name, other_account.name,);

        let (_, other_inventory_setters) = other_account.get_inventory_setters()?;

        if other_inventory_setters.is_empty() {
            return Ok(lua);
        }

        self.update(Some(lua), &other_inventory_setters)
    }

    pub fn get_inventory_setters(&self) -> Result<(Lua, Vec<actions::InventorySet>)> {
        let mut inventory_setters: Vec<actions::InventorySet> = Vec::new();

        let lua = Lua::new();

        let bagsync_db_path = self.bagsync_db_path();
        let db_contents = fs::read_to_string(&bagsync_db_path)
            .or_else::<std::io::Error, _>(|_| Ok(crate::BASE_DB.to_string()))?;
        lua.load(&db_contents).exec()?;
        let db: HashMap<String, Value> = lua.globals().get("BagSyncDB")?;

        for (realm_name, value) in db {
            if !realm_name.ends_with('§') {
                // NOTE: This limits us to only update inventory data on realms that existed on startup
                match self.realms.get(&realm_name) {
                    Some(realm) => {
                        let table: Table = lua.unpack(value)?;

                        for pair in table.pairs::<String, Value>() {
                            let (character_name, character_inventory) = pair?;
                            if !realm.characters.contains(&character_name) {
                                continue;
                            }

                            log::debug!("{:?} - {} - {}", self, realm_name, character_name);

                            inventory_setters.push(actions::InventorySet {
                                realm_name: realm_name.clone(),
                                character_name: character_name.clone(),
                                character_inventory_data: lua.from_value(character_inventory)?,
                            });
                        }
                    }
                    None => {
                        continue;
                    }
                }
            }
        }

        Ok((lua, inventory_setters))
    }
}

pub fn load(wtf_path: &str, account_names: &[&str]) -> Result<HashMap<String, Account>> {
    let mut accounts: HashMap<String, Account> = HashMap::new();
    let wtf_path = Path::new(wtf_path);
    for account_name in account_names {
        let account_dir = wtf_path.join(account_name);
        let acc = accounts.entry(account_name.to_string()).or_insert(Account {
            name: account_name.to_string(),
            realms: HashMap::new(),
            dir: account_dir.clone(),
        });
        for entry in fs::read_dir(account_dir)? {
            let entry = entry?;
            if !entry.path().is_dir() {
                continue;
            }
            if entry.file_name() == "SavedVariables" {
                continue;
            }

            let realm_name_os = entry.file_name();
            let realm_name = realm_name_os
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("bad unicode name"))?;
            for entry in fs::read_dir(entry.path())? {
                let realm = acc.realms.entry(realm_name.to_string()).or_insert(Realm {
                    characters: Vec::new(),
                });

                let entry = entry?;
                let character_name = entry.file_name();
                realm.characters.push(
                    character_name
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("bad unicode character name"))?
                        .to_string(),
                );
            }
        }
    }

    Ok(accounts)
}
