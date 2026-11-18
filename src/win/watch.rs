//! Windows: live folder updates are not wired up yet (ReadDirectoryChangesW comes in a
//! later stage). Folders refresh when reopened or reloaded.

use std::collections::HashSet;
use std::path::PathBuf;

pub struct Watch {
    pub changed_files: Vec<PathBuf>,
}

impl Watch {
    pub fn new(_ctx: egui::Context) -> Watch {
        Watch { changed_files: vec![] }
    }

    pub fn sync(&mut self, _want: &HashSet<PathBuf>) {}

    pub fn poll(&mut self) -> Vec<PathBuf> {
        vec![]
    }

    pub fn has_pending(&self) -> bool {
        false
    }
}
