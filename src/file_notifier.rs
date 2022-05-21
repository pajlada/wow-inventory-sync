use std::path::PathBuf;

use tokio::sync::mpsc;

pub struct FileNotifier {
    tx: mpsc::Sender<PathBuf>,
}

pub fn new(tx: mpsc::Sender<PathBuf>) -> FileNotifier {
    FileNotifier { tx }
}

impl notify::EventHandler for FileNotifier {
    fn handle_event(&mut self, event: notify::Result<notify::Event>) {
        if let Ok(notify::Event {
            kind:
                notify::EventKind::Modify(notify::event::ModifyKind::Name(
                    notify::event::RenameMode::Both,
                )),
            paths,
            attrs: _,
        }) = event
        {
            if let Some(target_path) = paths.get(1) {
                if target_path.ends_with("BagSyncString.lua") {
                    // Wow finished saving the path
                    match self.tx.blocking_send(target_path.clone()) {
                        Err(e) => {
                            println!("Error sending notification to mpsc: {}", e);
                        }
                        _ => {
                            println!("Sent notification about '{:?}", target_path);
                        }
                    }
                    // }
                }
            }
        }
    }
}
