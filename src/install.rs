//! Desktop integration: install to ~/.local, launcher entry, icon, default file manager.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Mochi: a soft folder in Catppuccin's pastels on a Mocha tile.
pub const ICON_SVG: &str = include_str!("../assets/mochi.svg");

fn local() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| crate::fsx::home().join(".local/share"))
}

fn bin_path() -> PathBuf {
    dirs::executable_dir().unwrap_or_else(|| crate::fsx::home().join(".local/bin")).join(crate::mochi::BIN)
}

fn desktop_path() -> PathBuf {
    local().join(format!("applications/{}.desktop", crate::mochi::APP_ID))
}

fn hicolor() -> PathBuf {
    local().join("icons/hicolor")
}

pub fn icon_in(hicolor: &Path) -> PathBuf {
    hicolor.join(format!("scalable/apps/{}.svg", crate::mochi::APP_ID))
}

/// Mochi: a PNG of the same name (Maia's kind of icon) would compete with the SVG.
pub fn old_icon_in(hicolor: &Path) -> PathBuf {
    hicolor.join(format!("256x256/apps/{}.png", crate::mochi::APP_ID))
}

/// Puts this build's icon under `hicolor` and takes away the old SVG. Writes nothing
/// when the icon there is already this one; returns whether anything changed.
pub fn place_icon(hicolor: &Path) -> std::io::Result<bool> {
    let old = std::fs::remove_file(old_icon_in(hicolor)).is_ok();
    let p = icon_in(hicolor);
    if std::fs::read(&p).is_ok_and(|b| b == ICON_SVG.as_bytes()) {
        return Ok(old);
    }
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = p.with_extension("svg.new");
    std::fs::write(&tmp, ICON_SVG)?;
    std::fs::rename(&tmp, &p)?;
    // The icon theme spec's signal to rescan: an icon cache older than the theme
    // folder is not trusted.
    let _ = std::fs::File::open(hicolor).and_then(|d| d.set_modified(std::time::SystemTime::now()));
    Ok(true)
}

/// An installed copy replaced in place (build-dist.sh, an update of the bare binary or
/// AppImage) brings the launcher's icon up to date with itself.
pub fn refresh_icon() {
    if installed_here() {
        let _ = place_icon(&hicolor());
    }
}

/// 0 not looked at yet, 1 not installed, 2 this program is the installed one.
static INSTALLED: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// This program is the installed copy, with its launcher entry: Install has nothing to
/// do (looked at once; an install marks it).
pub fn installed_here() -> bool {
    use std::sync::atomic::Ordering::Relaxed;
    if INSTALLED.load(Relaxed) == 0 {
        let same = std::env::current_exe().ok().and_then(|e| std::fs::canonicalize(e).ok()).is_some_and(|e| std::fs::canonicalize(bin_path()).ok() == Some(e));
        INSTALLED.store(if same && desktop_path().is_file() { 2 } else { 1 }, Relaxed);
    }
    INSTALLED.load(Relaxed) == 2
}

/// As if this program were (or weren't) the installed copy.
#[cfg(test)]
pub fn set_installed(yes: bool) {
    INSTALLED.store(if yes { 2 } else { 1 }, std::sync::atomic::Ordering::Relaxed);
}

pub fn install() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let bin = bin_path();
    std::fs::create_dir_all(bin.parent().unwrap()).map_err(|e| e.to_string())?;
    if exe != bin {
        // Copy to a temp name then rename, so a running copy isn't clobbered mid-write.
        let tmp = bin.with_extension("new");
        std::fs::copy(&exe, &tmp).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &bin).map_err(|e| e.to_string())?;
    }
    place_icon(&hicolor()).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(desktop_path().parent().unwrap()).map_err(|e| e.to_string())?;
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName={name}\nGenericName=File Manager\nComment=Maia's file manager, redesigned in Catppuccin\nExec={} %U\nIcon={id}\nTerminal=false\nCategories=System;FileTools;FileManager;\nMimeType=inode/directory;\nKeywords=files;folders;explorer;browser;mochi;\nStartupWMClass={id}\nActions=new-window;\n\n[Desktop Action new-window]\nName=New Window\nExec={} --window \"{}\"\n",
        bin.display(),
        bin.display(),
        crate::fsx::home().display(),
        name = crate::mochi::NAME,
        id = crate::mochi::APP_ID,
    );
    std::fs::write(desktop_path(), desktop).map_err(|e| e.to_string())?;
    drop_old_id();
    let _ = Command::new("update-desktop-database").arg(local().join("applications")).status();
    // Running from elsewhere, this copy is still not the installed one; from there, it is.
    INSTALLED.store(if exe == bin { 2 } else { 1 }, std::sync::atomic::Ordering::Relaxed);
    let on_path = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| Some(d.as_path()) == bin.parent())).unwrap_or(false);
    Ok(if on_path {
        format!("Installed to {}", bin.display())
    } else {
        format!("Installed to {} (add {} to your PATH)", bin.display(), bin.parent().unwrap().display())
    })
}

pub const DESKTOP_ID: &str = "io.github.bardiat94.Mochi.desktop";

/// Mochi's id before it matched the GitHub account.
const OLD_ID: &str = "io.github.bardia.Mochi";

/// An install under the old id gives way: its launcher entry and icon go, and if it
/// opened folders, the new entry does.
fn drop_old_id() {
    let _ = std::fs::remove_file(local().join(format!("applications/{OLD_ID}.desktop")));
    let _ = std::fs::remove_file(hicolor().join(format!("scalable/apps/{OLD_ID}.svg")));
    #[cfg(not(test))]
    if current_default().as_deref() == Some(&format!("{OLD_ID}.desktop")) {
        let _ = crate::mime::set_default(DESKTOP_ID, "inode/directory");
    }
}

/// The desktop's file manager: the first `inode/directory` default in the user's
/// mimeapps.list, where `xdg-mime default` writes it.
pub fn current_default() -> Option<String> {
    default_in(&std::fs::read_to_string(dirs::config_dir()?.join("mimeapps.list")).ok()?)
}

/// The `inode/directory` default in a mimeapps.list text.
pub fn default_in(text: &str) -> Option<String> {
    let mut in_defaults = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_defaults = line == "[Default Applications]";
        } else if in_defaults {
            if let Some(v) = line.strip_prefix("inode/directory=") {
                return v.split(';').map(str::trim).find(|s| !s.is_empty()).map(String::from);
            }
        }
    }
    None
}

pub fn is_default() -> bool {
    current_default().as_deref() == Some(DESKTOP_ID)
}

pub fn set_default() -> Result<String, String> {
    #[cfg(not(test))]
    if !desktop_path().exists() {
        install()?;
    }
    // Remember the previous handler so turning this off (or uninstalling) restores it.
    if let Some(prev) = current_default().filter(|p| p != DESKTOP_ID) {
        let _ = std::fs::create_dir_all(crate::config::data_dir());
        let _ = std::fs::write(crate::config::data_dir().join("previous-file-manager"), prev);
    }
    crate::mime::set_default(DESKTOP_ID, "inode/directory")?;
    // "Show in folder" from browsers and other apps comes here too.
    #[cfg(not(test))]
    let _ = crate::fm1::set_service(true, &crate::fm1::exe());
    Ok("Mochi is now the default file manager".into())
}

/// Give folders back to the file manager used before Maia, else to another one that
/// is installed.
pub fn unset_default() -> Result<String, String> {
    let prev = std::fs::read_to_string(crate::config::data_dir().join("previous-file-manager")).ok().map(|p| p.trim().to_string()).filter(|p| !p.is_empty() && p != DESKTOP_ID);
    let other = prev.or_else(|| crate::mime::apps().for_mime("inode/directory").into_iter().map(|a| a.id.clone()).find(|id| id != DESKTOP_ID));
    match other {
        Some(id) => {
            crate::mime::set_default(&id, "inode/directory")?;
            #[cfg(not(test))]
            let _ = crate::fm1::set_service(false, &crate::fm1::exe());
            let name = crate::mime::apps().by_id(&id).map(|a| a.name.clone()).unwrap_or(id);
            Ok(format!("{name} opens folders again"))
        }
        None => Err("No other file manager is installed".into()),
    }
}

pub fn uninstall() -> Result<String, String> {
    let prev = std::fs::read_to_string(crate::config::data_dir().join("previous-file-manager")).ok();
    if let Some(p) = prev {
        let _ = crate::mime::set_default(p.trim(), "inode/directory");
    }
    #[cfg(not(test))]
    let _ = crate::fm1::set_service(false, &crate::fm1::exe());
    // Open and Save dialogs go back to the desktop's, and the backend's files go.
    #[cfg(not(test))]
    {
        let pl = crate::portal::Places::user();
        if crate::portal::is_on(&pl) || pl.portal_file().exists() || pl.service_file().exists() {
            let _ = crate::portal::set(false);
        }
    }
    for p in [desktop_path(), icon_in(&hicolor()), old_icon_in(&hicolor())] {
        let _ = std::fs::remove_file(p);
    }
    let bin = bin_path();
    if bin.exists() {
        std::fs::remove_file(&bin).map_err(|e| e.to_string())?;
    }
    let _ = Command::new("update-desktop-database").arg(local().join("applications")).status();
    Ok("Mochi uninstalled (settings were kept)".into())
}

/// The window icon: the launcher icon at 64 px, for X11 window lists and docks
/// (Wayland ones find the launcher's icon through the app id).
pub fn window_icon() -> (Vec<u8>, u32, u32) {
    let Some((w, h, mut px)) = crate::svg::render(ICON_SVG.as_bytes(), 64) else { return (vec![0; 4], 1, 1) };
    // Un-premultiply: the renderer's pixels are premultiplied, the window icon's are not.
    for p in px.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    (px, w, h)
}
