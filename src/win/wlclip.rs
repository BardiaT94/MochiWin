//! Windows: the types the clipboard and drag-and-drop code share with the Wayland client
//! (src/wlclip.rs), and the Windows clipboard and drag out (WinClip).

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

/// The Windows clipboard and drag out, for the window `hwnd`.
pub struct WinClip {
    hwnd: usize,
}

mod sys {
    pub use windows_sys::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
    pub use windows_sys::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData};
    pub use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
    pub use windows_sys::Win32::System::Ole::{CF_DIB, CF_HDROP, CF_UNICODETEXT};
    pub use windows_sys::Win32::UI::Shell::Common::ITEMIDLIST;
    pub use windows_sys::Win32::UI::Shell::{DragQueryFileW, ILCreateFromPathW, ILFindLastID, ILFree, SHCreateDataObject, SHDoDragDrop};
}

fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(std::iter::once(0)).collect()
}

fn format_id(name: &str) -> u32 {
    let w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { sys::RegisterClipboardFormatW(w.as_ptr()) }
}

/// The clipboard, open until dropped. Another app may hold it for a moment, so opening
/// is tried a few times.
struct Open;

impl Open {
    fn new(hwnd: usize) -> std::io::Result<Open> {
        for _ in 0..20 {
            if unsafe { sys::OpenClipboard(hwnd as sys::HWND) } != 0 {
                return Ok(Open);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        Err(std::io::Error::last_os_error())
    }

    fn get(&self, format: u32) -> Option<Vec<u8>> {
        unsafe {
            if format == 0 || sys::IsClipboardFormatAvailable(format) == 0 {
                return None;
            }
            let h = sys::GetClipboardData(format);
            if h.is_null() {
                return None;
            }
            let p = sys::GlobalLock(h as sys::HGLOBAL) as *const u8;
            if p.is_null() {
                return None;
            }
            let v = std::slice::from_raw_parts(p, sys::GlobalSize(h as sys::HGLOBAL)).to_vec();
            sys::GlobalUnlock(h as sys::HGLOBAL);
            Some(v)
        }
    }

    fn put(&self, format: u32, bytes: &[u8]) {
        unsafe {
            let h = sys::GlobalAlloc(sys::GMEM_MOVEABLE, bytes.len().max(1));
            if h.is_null() {
                return;
            }
            let p = sys::GlobalLock(h) as *mut u8;
            if p.is_null() {
                sys::GlobalFree(h);
                return;
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
            sys::GlobalUnlock(h);
            // The clipboard owns the memory once it takes it.
            if sys::SetClipboardData(format, h as sys::HANDLE).is_null() {
                sys::GlobalFree(h);
            }
        }
    }

    fn files(&self) -> Option<Vec<PathBuf>> {
        use std::os::windows::ffi::OsStringExt;
        unsafe {
            if sys::IsClipboardFormatAvailable(sys::CF_HDROP as u32) == 0 {
                return None;
            }
            let h = sys::GetClipboardData(sys::CF_HDROP as u32);
            if h.is_null() {
                return None;
            }
            let n = sys::DragQueryFileW(h, u32::MAX, std::ptr::null_mut(), 0);
            let paths: Vec<PathBuf> = (0..n)
                .filter_map(|i| {
                    let len = sys::DragQueryFileW(h, i, std::ptr::null_mut(), 0);
                    let mut buf = vec![0u16; len as usize + 1];
                    let got = sys::DragQueryFileW(h, i, buf.as_mut_ptr(), buf.len() as u32) as usize;
                    (got > 0).then(|| PathBuf::from(std::ffi::OsString::from_wide(&buf[..got])))
                })
                .collect();
            (!paths.is_empty()).then_some(paths)
        }
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        unsafe { sys::CloseClipboard() };
    }
}

/// UTF-16, ended with a zero, as CF_UNICODETEXT holds it.
fn text_bytes(s: &str) -> Vec<u8> {
    s.replace("\r\n", "\n").replace('\n', "\r\n").encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes).collect()
}

fn text_of(b: &[u8]) -> String {
    let w: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&c| c != 0).collect();
    String::from_utf16_lossy(&w)
}

/// A CF_DIB as a .bmp file: the file header the clipboard leaves out.
fn bmp_of(dib: &[u8]) -> Option<Vec<u8>> {
    let u32_at = |i: usize| dib.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let size = u32_at(0)? as usize;
    let bits = u16::from_le_bytes([*dib.get(14)?, *dib.get(15)?]);
    let compression = u32_at(16)?;
    let used = u32_at(32).unwrap_or(0) as usize;
    let mut table = if bits <= 8 { (if used > 0 { used } else { 1 << bits }) * 4 } else { used * 4 };
    if size == 40 && compression == 3 {
        table += 12;
    }
    let offset = 14 + size + table;
    let mut out = Vec::with_capacity(14 + dib.len());
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((14 + dib.len()) as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(offset as u32).to_le_bytes());
    out.extend_from_slice(dib);
    Some(out)
}

const TEXT: [&str; 3] = ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain"];

impl WinClip {
    pub fn new(hwnd: usize) -> WinClip {
        WinClip { hwnd }
    }

    /// Text items go to the clipboard as text; the rest have no Windows form.
    pub fn store(&self, items: Vec<Item>) {
        let Some((_, b)) = items.iter().find(|(m, _)| TEXT.contains(&m.as_str())) else { return };
        let Ok(c) = Open::new(self.hwnd) else { return };
        unsafe { sys::EmptyClipboard() };
        c.put(sys::CF_UNICODETEXT as u32, &text_bytes(&String::from_utf8_lossy(b)));
    }

    /// The first of `prefer` the clipboard holds: text, or a picture.
    pub fn load(&self, prefer: &[&str]) -> std::io::Result<Item> {
        let c = Open::new(0)?;
        for m in prefer {
            let bytes = match *m {
                "text/plain;charset=utf-8" | "UTF8_STRING" | "text/plain" => c.get(sys::CF_UNICODETEXT as u32).map(|b| text_of(&b).into_bytes()),
                "image/png" => c.get(format_id("PNG")),
                "image/jpeg" => c.get(format_id("JFIF")),
                "image/gif" => c.get(format_id("GIF")),
                "image/bmp" => c.get(sys::CF_DIB as u32).and_then(|d| bmp_of(&d)),
                _ => None,
            };
            if let Some(b) = bytes {
                return Ok((m.to_string(), b));
            }
        }
        Err(std::io::ErrorKind::NotFound.into())
    }

    /// Files as Explorer copies them (CF_HDROP), with "Preferred DropEffect" saying
    /// whether they were cut.
    pub fn set_files(&self, paths: &[PathBuf], cut: bool) {
        // DROPFILES: offset of the list, a point, "not in client area", wide names.
        let mut b = Vec::new();
        b.extend_from_slice(&20u32.to_le_bytes());
        b.extend_from_slice(&[0; 12]);
        b.extend_from_slice(&1u32.to_le_bytes());
        for p in paths {
            b.extend(wide(p.as_os_str()).into_iter().flat_map(u16::to_le_bytes));
        }
        b.extend_from_slice(&[0, 0]);
        let Ok(c) = Open::new(self.hwnd) else { return };
        unsafe { sys::EmptyClipboard() };
        c.put(sys::CF_HDROP as u32, &b);
        // DROPEFFECT_MOVE for a cut, COPY | LINK as Explorer sets for a copy.
        c.put(format_id("Preferred DropEffect"), &(if cut { 2u32 } else { 5u32 }).to_le_bytes());
    }

    /// Files on the clipboard, and whether they were cut.
    pub fn files(&self) -> Option<(Vec<PathBuf>, bool)> {
        let c = Open::new(0).ok()?;
        let paths = c.files()?;
        let cut = c.get(format_id("Preferred DropEffect")).and_then(|b| b.get(..4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))).is_some_and(|e| e & 2 != 0 && e & 1 == 0);
        Some((paths, cut))
    }

    /// Carry files out of the window: the shell's own drag, as from Explorer, with its
    /// picture. Returns when they are dropped (the system runs the drag meanwhile).
    pub fn start_drag(&self, paths: Vec<PathBuf>) {
        // IID_IDataObject
        const IID: windows_sys::core::GUID = windows_sys::core::GUID::from_u128(0x0000010e_0000_0000_c000_000000000046);
        unsafe {
            let pidls: Vec<*mut sys::ITEMIDLIST> = paths.iter().map(|p| sys::ILCreateFromPathW(wide(p.as_os_str()).as_ptr())).filter(|p| !p.is_null()).collect();
            if pidls.is_empty() {
                return;
            }
            // Items in one folder go as children of it (the usual case); others as
            // children of the desktop, whose ID list is empty.
            let parent = paths[0].parent();
            let shared = parent.is_some() && paths.iter().all(|p| p.parent() == parent);
            let folder = if shared { sys::ILCreateFromPathW(wide(parent.unwrap_or(&paths[0]).as_os_str()).as_ptr()) } else { std::ptr::null_mut() };
            let desktop = [0u16; 1];
            let (root, items): (*const sys::ITEMIDLIST, Vec<*const sys::ITEMIDLIST>) = if !folder.is_null() {
                (folder, pidls.iter().map(|&p| sys::ILFindLastID(p) as *const _).collect())
            } else {
                (desktop.as_ptr() as *const _, pidls.iter().map(|&p| p as *const _).collect())
            };
            let mut obj: *mut core::ffi::c_void = std::ptr::null_mut();
            let hr = sys::SHCreateDataObject(root, items.len() as u32, items.as_ptr(), std::ptr::null_mut(), &IID, &mut obj);
            if hr >= 0 && !obj.is_null() {
                let mut effect = 0;
                // Copy, move or link: the target picks, as with a drag from Explorer.
                sys::SHDoDragDrop(self.hwnd as sys::HWND, obj, std::ptr::null_mut(), 1 | 2 | 4, &mut effect);
                // IUnknown::Release, the third entry of its table.
                let vtbl = *(obj as *const *const unsafe extern "system" fn(*mut core::ffi::c_void) -> u32);
                (*vtbl.add(2))(obj);
            }
            if !folder.is_null() {
                sys::ILFree(folder);
            }
            for p in pidls {
                sys::ILFree(p);
            }
        }
    }
}
