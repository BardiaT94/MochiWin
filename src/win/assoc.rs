//! Windows: the apps that open a file type, as Explorer's "Open with" lists them
//! (SHAssocEnumHandlers), and opening files with one of them through the shell, which
//! also starts Store apps.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize};
use windows_sys::Win32::UI::Shell::{ASSOCF_NONE, ASSOCSTR_FRIENDLYAPPNAME, AssocQueryStringW, OAIF_ALLOW_REGISTRATION, OAIF_EXEC, OAIF_REGISTER_EXT, OPENASINFO, SHAssocEnumHandlers, SHOpenWithDialog};

type Obj = *mut core::ffi::c_void;
type Str = *mut u16;

/// An App's id (and its exec) names the handler this way: the prefix, then the name
/// Windows gives it (an exe's path, or a Store app's id).
pub const PREFIX: &str = "assoc:";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The type Windows files a path under: ".pdf"; "." when it has no extension.
fn ext_of(p: &Path) -> String {
    p.extension().map(|e| format!(".{}", e.to_string_lossy().to_lowercase())).unwrap_or_else(|| ".".into())
}

/// The COM entry `i` of an object's table.
unsafe fn slot<F: Copy>(obj: Obj, i: usize) -> F {
    unsafe {
        let vtbl = *(obj as *const *const F);
        *vtbl.add(i)
    }
}

unsafe fn take_str(s: Str) -> Option<String> {
    if s.is_null() {
        return None;
    }
    unsafe {
        let n = (0..).take_while(|&i| *s.add(i) != 0).count();
        let out = String::from_utf16_lossy(std::slice::from_raw_parts(s, n));
        CoTaskMemFree(s as *const _);
        Some(out)
    }
}

/// COM for this thread while it lives (the window's thread already has it).
struct Com(bool);

impl Com {
    fn new() -> Com {
        Com(unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) } >= 0)
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

/// Each handler for `ext` with its name and the name to show; `f` may keep one (it is
/// released after `f` returns). `all` lists every registered app, else the recommended.
fn each(ext: &str, all: bool, mut f: impl FnMut(Obj, &str, &str)) {
    let _com = Com::new();
    let e = wide(ext);
    let mut en: Obj = std::ptr::null_mut();
    if unsafe { SHAssocEnumHandlers(e.as_ptr(), if all { 0 } else { 1 }, &mut en) } < 0 || en.is_null() {
        return;
    }
    unsafe {
        let next: unsafe extern "system" fn(Obj, u32, *mut Obj, *mut u32) -> i32 = slot(en, 3);
        loop {
            let (mut h, mut got): (Obj, u32) = (std::ptr::null_mut(), 0);
            if next(en, 1, &mut h, &mut got) != 0 || got == 0 || h.is_null() {
                break;
            }
            let get_name: unsafe extern "system" fn(Obj, *mut Str) -> i32 = slot(h, 3);
            let get_ui: unsafe extern "system" fn(Obj, *mut Str) -> i32 = slot(h, 4);
            let (mut a, mut b): (Str, Str) = (std::ptr::null_mut(), std::ptr::null_mut());
            let name = if get_name(h, &mut a) >= 0 { take_str(a) } else { None };
            let ui = if get_ui(h, &mut b) >= 0 { take_str(b) } else { None };
            if let Some(name) = name {
                let ui = ui.filter(|u| !u.is_empty()).unwrap_or_else(|| Path::new(&name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| name.clone()));
                f(h, &name, &ui);
            }
            crate::wlclip::release(h);
        }
        crate::wlclip::release(en);
    }
}

/// The name Windows shows for the default app of `ext`.
fn default_name(ext: &str) -> Option<String> {
    let e = wide(ext);
    let verb = wide("open");
    let mut buf = vec![0u16; 512];
    let mut n = buf.len() as u32;
    let r = unsafe { AssocQueryStringW(ASSOCF_NONE, ASSOCSTR_FRIENDLYAPPNAME, e.as_ptr(), verb.as_ptr(), buf.as_mut_ptr(), &mut n) };
    (r >= 0).then(|| String::from_utf16_lossy(&buf[..(n as usize).saturating_sub(1)])).filter(|s| !s.is_empty())
}

type Cache = HashMap<(String, bool), (Instant, Arc<Vec<crate::mime::App>>)>;
static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

/// The apps for files like `path`, the default first. Asked again after a few seconds, so
/// an app installed or a default changed meanwhile shows.
pub fn apps_for(path: &Path, all: bool) -> Arc<Vec<crate::mime::App>> {
    let ext = ext_of(path);
    let key = (ext.clone(), all);
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = g.get_or_insert_with(HashMap::new);
    if let Some((at, v)) = cache.get(&key)
        && at.elapsed() < Duration::from_secs(5)
    {
        return v.clone();
    }
    let mut out: Vec<crate::mime::App> = vec![];
    each(&ext, all, |_, name, ui| {
        if !out.iter().any(|a| a.id[PREFIX.len()..] == *name) {
            let id = format!("{PREFIX}{name}");
            out.push(crate::mime::App { id: id.clone(), name: ui.to_string(), exec: id, mimes: vec![], terminal: false, hidden: false, icon: String::new() });
        }
    });
    if let Some(d) = default_name(&ext)
        && let Some(i) = out.iter().position(|a| a.name.eq_ignore_ascii_case(&d))
    {
        let a = out.remove(i);
        out.insert(0, a);
    }
    let v = Arc::new(out);
    cache.insert(key, (Instant::now(), v.clone()));
    v
}

/// An app listed earlier by `apps_for`, by its id.
pub fn by_id(id: &str) -> Option<crate::mime::App> {
    let g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    g.as_ref()?.values().find_map(|(_, v)| v.iter().find(|a| a.id == id).cloned())
}

/// Open `paths` with the handler `name` (an id without its prefix).
pub fn open_with(name: &str, paths: &[PathBuf]) -> Result<(), String> {
    let first = paths.first().ok_or("no files")?;
    let mut result = Err("That app no longer opens this type of file".to_string());
    let mut done = false;
    // Every registered app, so one picked from either list is found.
    each(&ext_of(first), true, |h, n, _| {
        if done || n != name {
            return;
        }
        done = true;
        let obj = crate::wlclip::data_object(paths);
        if obj.is_null() {
            result = Err("Couldn't hand the files over".into());
            return;
        }
        unsafe {
            let invoke: unsafe extern "system" fn(Obj, Obj) -> i32 = slot(h, 8);
            let r = invoke(h, obj);
            crate::wlclip::release(obj);
            result = if r >= 0 { Ok(()) } else { Err(std::io::Error::from_raw_os_error(r).to_string()) };
        }
    });
    result
}

/// Windows' own "Open with" window for `path`: every app, the Store, and "Always use
/// this app" (Windows lets no other program set a default).
/// It runs on a thread of its own, so Mochi keeps drawing while it is open.
pub fn system_dialog(path: &Path) {
    use std::os::windows::ffi::OsStrExt;
    let p: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let _ = std::thread::Builder::new().name("open-with".into()).spawn(move || {
        let _com = Com::new();
        let info = OPENASINFO { pcszFile: p.as_ptr(), pcszClass: std::ptr::null(), oaifInFlags: OAIF_ALLOW_REGISTRATION | OAIF_REGISTER_EXT | OAIF_EXEC };
        unsafe { SHOpenWithDialog(std::ptr::null_mut(), &info) };
    });
}
