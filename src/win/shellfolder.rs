//! Mochi as the default file manager on Windows: folders and drives opened from the
//! desktop or another app, and Win+E, come to Mochi. Per-user keys under
//! HKCU\Software\Classes only, so no admin rights; turning it off deletes them and
//! Explorer is back.
//!
//! A verb of Mochi's own ("mochi") becomes the default for Directory and Drive, so the
//! shell's own "open" and "explore" stay Explorer's (Explorer's "Open in new window" still
//! works). Calls that name a verb, `SHOpenFolderAndSelectItems` ("Show in folder" in
//! browsers) and `explorer.exe <folder>` still reach Explorer: they skip these keys.

use std::path::{Path, PathBuf};
use windows_sys::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteKeyValueW, RegDeleteTreeW, RegGetValueW, RegSetValueExW};

const VERB: &str = "mochi";
/// The File Explorer launcher behind Win+E and the taskbar's Explorer shortcut.
const LAUNCHER: &str = r"Software\Classes\CLSID\{52205fd8-5dfb-447d-801a-d0b52f2e83e1}";
const KINDS: [&str; 2] = ["Directory", "Drive"];

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A string value (`None`: the key's default value), or None when it isn't there.
fn get(key: &str, name: Option<&str>) -> Option<String> {
    let k = w(key);
    let n = name.map(w);
    let np = n.as_ref().map_or(std::ptr::null(), |n| n.as_ptr());
    let mut len = 0u32;
    if unsafe { RegGetValueW(HKEY_CURRENT_USER, k.as_ptr(), np, RRF_RT_REG_SZ, std::ptr::null_mut(), std::ptr::null_mut(), &mut len) } != 0 {
        return None;
    }
    let mut buf = vec![0u16; (len as usize).div_ceil(2) + 1];
    let mut len = (buf.len() * 2) as u32;
    if unsafe { RegGetValueW(HKEY_CURRENT_USER, k.as_ptr(), np, RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut len) } != 0 {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end]))
}

/// Writes a string value, making the key if needed.
fn set(key: &str, name: Option<&str>, value: &str) -> Result<(), String> {
    let k = w(key);
    let mut h: HKEY = std::ptr::null_mut();
    let r = unsafe { RegCreateKeyExW(HKEY_CURRENT_USER, k.as_ptr(), 0, std::ptr::null(), REG_OPTION_NON_VOLATILE, KEY_READ | KEY_WRITE, std::ptr::null(), &mut h, std::ptr::null_mut()) };
    if r != 0 {
        return Err(format!("can't write HKCU\\{key} (error {r})"));
    }
    let n = name.map(w);
    let v = w(value);
    let r = unsafe { RegSetValueExW(h, n.as_ref().map_or(std::ptr::null(), |n| n.as_ptr()), 0, REG_SZ, v.as_ptr().cast(), (v.len() * 2) as u32) };
    unsafe { RegCloseKey(h) };
    if r != 0 { Err(format!("can't write HKCU\\{key} (error {r})")) } else { Ok(()) }
}

fn delete_tree(key: &str) {
    unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, w(key).as_ptr()) };
}

/// Deletes the key itself too, which RegDeleteTreeW leaves.
fn delete_key(key: &str) {
    delete_tree(key);
    unsafe { windows_sys::Win32::System::Registry::RegDeleteKeyW(HKEY_CURRENT_USER, w(key).as_ptr()) };
}

fn delete_default_value(key: &str) {
    unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, w(key).as_ptr(), std::ptr::null()) };
}

fn shell_key(kind: &str) -> String {
    format!(r"Software\Classes\{kind}\shell")
}

/// The main mochi.exe, also when this is the copy in `software-gl\`.
fn exe() -> PathBuf {
    let e = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("mochi.exe"));
    match e.parent() {
        Some(d) if d.file_name().is_some_and(|n| n.eq_ignore_ascii_case("software-gl")) => d.parent().map(|p| p.join(e.file_name().unwrap_or_default())).unwrap_or(e),
        _ => e,
    }
}

/// Windows rereads file associations.
fn tell_shell() {
    use windows_sys::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED as i32, SHCNF_IDLIST, std::ptr::null(), std::ptr::null()) };
}

/// Where another program's default verb for folders is kept while Mochi's is on.
fn saved_verb_file(kind: &str) -> PathBuf {
    crate::config::data_dir().join(format!("previous-{}-verb", kind.to_lowercase()))
}

pub fn is_default() -> bool {
    get(&shell_key("Directory"), None).is_some_and(|v| v.eq_ignore_ascii_case(VERB))
        && get(&format!(r"{}\{VERB}\command", shell_key("Directory")), None).is_some_and(|c| Path::new(c.split('"').nth(1).unwrap_or("")).is_file())
}

pub fn set_default() -> Result<String, String> {
    let exe = exe();
    let e = exe.to_string_lossy();
    for kind in KINDS {
        let shell = shell_key(kind);
        // Another program's default verb (another file manager) comes back when this is off.
        if let Some(prev) = get(&shell, None).filter(|v| !v.is_empty() && !v.eq_ignore_ascii_case(VERB)) {
            let _ = std::fs::create_dir_all(crate::config::data_dir());
            let _ = std::fs::write(saved_verb_file(kind), prev);
        }
        let verb = format!(r"{shell}\{VERB}");
        set(&verb, None, "Open in Mochi")?;
        set(&verb, Some("Icon"), &format!("{e},0"))?;
        // A drive's %1 is "C:\", whose backslash would swallow a closing quote.
        let arg = if kind == "Drive" { "%1" } else { "\"%1\"" };
        set(&format!(r"{verb}\command"), None, &format!("\"{e}\" {arg}"))?;
        set(&shell, None, VERB)?;
    }
    // Win+E. An empty DelegateExecute hides Explorer's own handler, so the command runs.
    let cmd = format!(r"{LAUNCHER}\shell\opennewwindow\command");
    set(&cmd, None, &format!("\"{e}\""))?;
    set(&cmd, Some("DelegateExecute"), "")?;
    tell_shell();
    Ok("Mochi now opens folders, drives and Win+E".into())
}

pub fn unset_default() -> Result<String, String> {
    for kind in KINDS {
        let shell = shell_key(kind);
        delete_key(&format!(r"{shell}\{VERB}"));
        let saved = saved_verb_file(kind);
        match std::fs::read_to_string(&saved).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
            Some(prev) => set(&shell, None, &prev)?,
            None => {
                if get(&shell, None).is_some_and(|v| v.eq_ignore_ascii_case(VERB)) {
                    delete_default_value(&shell);
                }
            }
        }
        let _ = std::fs::remove_file(saved);
    }
    // Only Mochi's own Win+E command goes; one another program set stays.
    let cmd = format!(r"{LAUNCHER}\shell\opennewwindow\command");
    if get(&cmd, None).is_some_and(|c| c.to_lowercase().contains("mochi")) {
        delete_key(&format!(r"{LAUNCHER}\shell\opennewwindow"));
        // The parents only if nothing else is in them (RegDeleteKeyW fails then).
        for k in [format!(r"{LAUNCHER}\shell"), LAUNCHER.to_string()] {
            unsafe { windows_sys::Win32::System::Registry::RegDeleteKeyW(HKEY_CURRENT_USER, w(&k).as_ptr()) };
        }
    }
    tell_shell();
    Ok("Explorer opens folders again".into())
}
