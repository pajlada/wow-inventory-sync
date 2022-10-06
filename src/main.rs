use clap::{Arg, ArgAction, Command};
use std::fs;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use mlua::Lua;
use notify::Watcher;

use tokio::sync::mpsc;

mod accounts;
mod actions;
mod file_notifier;

static BASE_DB: &str = "BagSyncDB={}";

fn inventory_db_updated(
    accounts: &HashMap<String, accounts::Account>,
    account_name: &str,
) -> Result<()> {
    let current_account = accounts
        .get(account_name)
        .ok_or_else(|| anyhow::anyhow!("Account '{account_name}' not registered",))?;

    let (mut lua, inventory_setters) = current_account.get_inventory_setters()?;

    for (into_account_name, into_account) in accounts {
        if account_name == into_account_name {
            continue;
        }

        into_account.update(None, &inventory_setters)?;
        lua = current_account.update_from(lua, into_account)?;
    }

    Ok(())
}

fn get_account_name_from_path(path: &Path) -> Result<&str> {
    let savedvariables_dir = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Missing parent"))?;
    let account_dir = savedvariables_dir
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Missing parent"))?
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Can't get file name of parent"))?
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("account_dir not valid UTF-8"))?;
    Ok(account_dir)
}

async fn sync_files(
    accounts: HashMap<String, accounts::Account>,
    mut rx: mpsc::Receiver<PathBuf>,
) -> Result<()> {
    loop {
        match rx.recv().await {
            Some(str) => {
                let account_name = get_account_name_from_path(&str)
                    .context("Failed getting account name from path {str}")?;
                inventory_db_updated(&accounts, account_name)
                    .context("Failed handling db update for {account_name}")?;
            }
            None => {
                log::error!("Error reading sync_files signal");
                return Ok(());
            }
        }
    }
}

async fn run(wtf_path: &str, accounts_to_sync: &[&str]) -> Result<()> {
    let (tx, rx) = mpsc::channel(1);

    let mut watcher = notify::recommended_watcher(file_notifier::new(tx))?;

        println!("aaaaa");
    let accounts = accounts::load(wtf_path, accounts_to_sync)?;

        println!("a");
    // Watch all account SavedVariables dir
    for (account_name, account) in &accounts {
        let savedvariables_dir = account.dir.join("SavedVariables");
        watcher.watch(&savedvariables_dir, notify::RecursiveMode::NonRecursive)?;

        // Sync databases on startup
        let mut lua = Lua::new();
        let bagsync_db_path = account.bagsync_db_path();
        let db_contents = fs::read_to_string(&bagsync_db_path)
            .or_else::<std::io::Error, _>(|_| Ok(BASE_DB.to_string()))?;
        lua.load(&db_contents).exec()?;

        for (other_account_name, other_account) in &accounts {
            if account_name == other_account_name {
                continue;
            }
            lua = account.update_from(lua, other_account)?;
        }
    }

    let file_syncer = sync_files(accounts, rx);

    log::info!("Press CTRL+C to exit");

    tokio::select! {
        res = file_syncer => {
            res.context("file_syncer stopped")?;
        }

        _ = tokio::signal::ctrl_c() => {
            log::info!("ctrl_c pressed");
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches =
        Command::new(clap::crate_name!())
            .version(clap::crate_version!())
            .author(clap::crate_authors!())
            .about(clap::crate_description!())
            .arg(
                Arg::new("config")
                    .short('c')
                    .long("config")
                    .value_name("FILE")
                    .help("Path to config file to use")
                    .default_value("config.json"),
            )
            .arg(
                Arg::new("verbose")
                    .short('v')
                    .long("verbose")
                    .action(ArgAction::Count)
                    .help("Sets the level of verbosity"),
            )
            .arg(
                Arg::new("fix")
                    .long("fix")
                    .action(ArgAction::SetTrue)
                    .help("Try to fix the issues found"),
            )
            .arg(
                Arg::new("repo")
                    .long("repo")
                    .action(ArgAction::Append)
                    .help("Target GitHub repository"),
            )
            .arg(
                Arg::new("wtf_path")
                .required(true)
                .help( "Path to the WTF directory (e.g. /home/pajlada/World of Warcraft/_classic_/WTF)"),
                )
            .arg(
                Arg::new("account")
                    .long("account")
                    .action(ArgAction::Append)
                    .help("Name of account to include"),
            )
            .arg(
                Arg::new("organization")
                    .long("organization")
                    .alias("org")
                    .action(ArgAction::Append)
                    .help("Target GitHub organization"),
            )
            .get_matches();

    let log_level = match matches.get_count("verbose") {
        0 => log::LevelFilter::Info,
        1 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };

    println!("log level: {log_level:?}");

    let wtf_path = matches
        .get_one::<String>("wtf_path")
        .ok_or_else(|| anyhow::anyhow!("Missing required wtf_path parameter"))?;
    let accounts: Vec<&str> = matches
        .get_many::<String>("account")
        .unwrap_or_default()
        .map(|v| v.as_str())
        .collect::<Vec<_>>();

    if accounts.len() < 2 {
        return Err(anyhow::anyhow!(
            "Must specify at least two accounts with --account option"
        ));
    }

    env_logger::Builder::new()
        .format_timestamp(None)
        .format_target(false)
        .filter_module(module_path!(), log_level)
        .init();

    run(wtf_path, &accounts).await
}
