//! Windows: following a growing file is not wired up yet; the viewer shows the file as
//! it was when opened.

use std::path::PathBuf;

pub const TAIL: u64 = 1 << 20;

pub struct Tail;

impl Tail {
    pub fn start(_path: PathBuf, _from: Option<u64>, _ctx: egui::Context) -> Option<Tail> {
        None
    }

    pub fn take(&self) -> Option<(bool, Vec<u8>)> {
        None
    }
}
