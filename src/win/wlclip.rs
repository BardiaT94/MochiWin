//! Windows: the types the clipboard and drag-and-drop code share with the Wayland client
//! (src/wlclip.rs). No Windows clipboard backend yet; that comes in a later stage.

use std::path::PathBuf;
use std::sync::Mutex;

/// (mime type, bytes)
pub type Item = (String, Vec<u8>);

/// Drag and drop as the app sees it. Positions are surface-local logical pixels.
#[derive(Clone, Debug, PartialEq)]
pub enum Dnd {
    Over { x: f32, y: f32, paths: Vec<PathBuf>, own: bool },
    Left,
    Dropped { x: f32, y: f32, paths: Vec<PathBuf>, own: bool },
}

static EVENTS: Mutex<Vec<Dnd>> = Mutex::new(Vec::new());
static WAKER: Mutex<Option<Box<dyn Fn() + Send>>> = Mutex::new(None);

pub fn set_waker(f: impl Fn() + Send + 'static) {
    *WAKER.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(f));
}

pub fn take_events() -> Vec<Dnd> {
    std::mem::take(&mut *EVENTS.lock().unwrap_or_else(|e| e.into_inner()))
}

pub fn emit(ev: Dnd) {
    EVENTS.lock().unwrap_or_else(|e| e.into_inner()).push(ev);
    if let Some(w) = WAKER.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        w();
    }
}

/// The picture under the pointer while files are dragged out: premultiplied ARGB8888
/// (little-endian BGRA) at `scale` pixels per logical pixel.
pub struct Picture {
    pub w: usize,
    pub h: usize,
    pub scale: i32,
    pub px: Vec<u8>,
}
