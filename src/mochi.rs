//! Mochi: a fork of Maia with its own look. The colours are Catppuccin's, laid out as
//! Material 3 tonal roles (a crust backdrop, base cards floating on it, accent-tinted
//! containers for what is selected), in any of Catppuccin's 14 accents. The shapes are
//! rounder: pill tabs, pill rows, round icon buttons, large-radius cards and dialogs.
//!
//! Updating does not download Maia's releases (they would drop this look): it pulls the
//! upstream clone, applies this fork's patch again, rebuilds and installs, through
//! `overlay/mochi.sh` beside the source tree.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering::Relaxed};
use std::sync::{Arc, Mutex};

use egui::{Color32, CornerRadius, Painter, Rect, Vec2};

use crate::theme::{Glyph, Palette, ThemeName};
use crate::update::{Release, State};

pub const NAME: &str = "Mochi";
pub const BIN: &str = "mochi";
pub const APP_ID: &str = "io.github.bardiat94.Mochi";
/// Settings, data and cache folders (`~/.config/mochi`, …), apart from Maia's.
pub const DIR: &str = "mochi";

/// Space between the window's edge, the sidebar and the panel cards.
pub const GUTTER: f32 = 8.0;
/// The panel cards' corners.
pub const CARD_R: u8 = 16;
/// Dialogs and menus.
pub const POPUP_R: u8 = 18;
/// Cards inside dialogs, text fields.
pub const INNER_R: u8 = 12;

pub const ACCENTS: [&str; 14] = ["Rosewater", "Flamingo", "Pink", "Mauve", "Red", "Maroon", "Peach", "Yellow", "Green", "Teal", "Sky", "Sapphire", "Blue", "Lavender"];
pub const DEFAULT_ACCENT: &str = "Mauve";

static ACCENT: AtomicU8 = AtomicU8::new(3);

/// Use the accent called `name` (any case); an unknown name keeps the current one.
pub fn set_accent(name: &str) {
    if let Some(i) = ACCENTS.iter().position(|a| a.eq_ignore_ascii_case(name.trim())) {
        ACCENT.store(i as u8, Relaxed);
    }
}

/// Follow the configured accent; true when it changed (the palette is then rebuilt).
pub fn sync_accent(name: &str) -> bool {
    let before = accent_index();
    set_accent(name);
    before != accent_index()
}

pub fn accent_index() -> usize {
    ACCENT.load(Relaxed) as usize
}

/// One Catppuccin flavour: the 14 accents in `ACCENTS` order, then text to crust.
struct Flavor {
    accents: [u32; 14],
    text: u32,
    subtext1: u32,
    subtext0: u32,
    surface2: u32,
    surface1: u32,
    surface0: u32,
    base: u32,
    mantle: u32,
    crust: u32,
    dark: bool,
}

const LATTE: Flavor = Flavor {
    accents: [0xdc8a78, 0xdd7878, 0xea76cb, 0x8839ef, 0xd20f39, 0xe64553, 0xfe640b, 0xdf8e1d, 0x40a02b, 0x179299, 0x04a5e5, 0x209fb5, 0x1e66f5, 0x7287fd],
    text: 0x4c4f69,
    subtext1: 0x5c5f77,
    subtext0: 0x6c6f85,
    surface2: 0xacb0be,
    surface1: 0xbcc0cc,
    surface0: 0xccd0da,
    base: 0xeff1f5,
    mantle: 0xe6e9ef,
    crust: 0xdce0e8,
    dark: false,
};

const FRAPPE: Flavor = Flavor {
    accents: [0xf2d5cf, 0xeebebe, 0xf4b8e4, 0xca9ee6, 0xe78284, 0xea999c, 0xef9f76, 0xe5c890, 0xa6d189, 0x81c8be, 0x99d1db, 0x85c1dc, 0x8caaee, 0xbabbf1],
    text: 0xc6d0f5,
    subtext1: 0xb5bfe2,
    subtext0: 0xa5adce,
    surface2: 0x626880,
    surface1: 0x51576d,
    surface0: 0x414559,
    base: 0x303446,
    mantle: 0x292c3c,
    crust: 0x232634,
    dark: true,
};

const MACCHIATO: Flavor = Flavor {
    accents: [0xf4dbd6, 0xf0c6c6, 0xf5bde6, 0xc6a0f6, 0xed8796, 0xee99a0, 0xf5a97f, 0xeed49f, 0xa6da95, 0x8bd5ca, 0x91d7e3, 0x7dc4e4, 0x8aadf4, 0xb7bdf8],
    text: 0xcad3f5,
    subtext1: 0xb8c0e0,
    subtext0: 0xa5adcb,
    surface2: 0x5b6078,
    surface1: 0x494d64,
    surface0: 0x363a4f,
    base: 0x24273a,
    mantle: 0x1e2030,
    crust: 0x181926,
    dark: true,
};

const MOCHA: Flavor = Flavor {
    accents: [0xf5e0dc, 0xf2cdcd, 0xf5c2e7, 0xcba6f7, 0xf38ba8, 0xeba0ac, 0xfab387, 0xf9e2af, 0xa6e3a1, 0x94e2d5, 0x89dceb, 0x74c7ec, 0x89b4fa, 0xb4befe],
    text: 0xcdd6f4,
    subtext1: 0xbac2de,
    subtext0: 0xa6adc8,
    surface2: 0x585b70,
    surface1: 0x45475a,
    surface0: 0x313244,
    base: 0x1e1e2e,
    mantle: 0x181825,
    crust: 0x11111b,
    dark: true,
};

fn flavor(t: ThemeName) -> Option<&'static Flavor> {
    match t {
        ThemeName::Latte => Some(&LATTE),
        ThemeName::CatppuccinFrappe => Some(&FRAPPE),
        ThemeName::CatppuccinMacchiato => Some(&MACCHIATO),
        ThemeName::Mocha => Some(&MOCHA),
        _ => None,
    }
}

pub fn is_catppuccin(t: ThemeName) -> bool {
    flavor(t).is_some()
}

fn rgb(x: u32) -> Color32 {
    Color32::from_rgb((x >> 16) as u8, (x >> 8) as u8, x as u8)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// The accent `i` of the Catppuccin flavour `t` (for the picker's swatches).
pub fn accent_of(t: ThemeName, i: usize) -> Option<Color32> {
    flavor(t).map(|f| rgb(f.accents[i % 14]))
}

/// Every theme, Catppuccin's flavours first (the picker's order).
pub fn themes_first() -> Vec<ThemeName> {
    let mut v = ThemeName::ALL.to_vec();
    v.sort_by_key(|t| !is_catppuccin(*t));
    v
}

/// Recolour `p` for Mochi: Catppuccin flavours get their tonal roles in the chosen accent;
/// every other theme keeps its colours, with the backdrop behind its cards made one.
pub fn restyle(t: ThemeName, p: &mut Palette) {
    let Some(f) = flavor(t) else {
        p.sidebar = p.bg;
        p.chrome = p.bg;
        return;
    };
    let accent = rgb(f.accents[accent_index()]);
    let (base, surface0) = (rgb(f.base), rgb(f.surface0));
    p.dark = f.dark;
    p.bg = rgb(f.crust);
    p.sidebar = rgb(f.crust);
    p.chrome = rgb(f.crust);
    p.panel = base;
    p.text = rgb(f.text);
    p.text_dim = rgb(f.subtext0);
    p.accent = accent;
    p.danger = rgb(f.accents[4]);
    if f.dark {
        // Surface container high for menus; accent containers at increasing strength.
        p.raised = mix(base, surface0, 0.6);
        p.hover = surface0;
        p.border = mix(surface0, rgb(f.surface1), 0.25);
        p.alt = mix(base, rgb(f.mantle), 0.45);
        p.row_hover = mix(base, accent, 0.09);
        p.selected = mix(base, accent, 0.22);
        p.cursor = mix(base, accent, 0.32);
    } else {
        p.raised = mix(base, Color32::WHITE, 0.55);
        p.hover = mix(base, surface0, 0.6);
        p.border = surface0;
        p.alt = mix(base, rgb(f.mantle), 0.5);
        p.row_hover = mix(base, accent, 0.07);
        p.selected = mix(base, accent, 0.16);
        p.cursor = mix(base, accent, 0.25);
    }
}

/// The terminal pane's 16 colours: Catppuccin's own terminal palette for the flavour (as
/// its kitty theme has them).
pub fn ansi(t: ThemeName) -> Option<[Color32; 16]> {
    let f = flavor(t)?;
    let a = |i: usize| rgb(f.accents[i]);
    // red, green, yellow, blue, pink, teal
    let hues = [a(4), a(8), a(7), a(12), a(2), a(9)];
    let (black, white, bright_black, bright_white) = if f.dark { (f.surface1, f.subtext1, f.surface2, f.subtext0) } else { (f.subtext1, f.surface2, f.subtext0, f.surface1) };
    let mut c = [Color32::BLACK; 16];
    c[0] = rgb(black);
    c[7] = rgb(white);
    c[8] = rgb(bright_black);
    c[15] = rgb(bright_white);
    for (i, h) in hues.into_iter().enumerate() {
        c[1 + i] = h;
        c[9 + i] = h;
    }
    Some(c)
}

/// The user's login shell (what kitty runs), else $SHELL, else sh.
pub fn login_shell() -> String {
    // The simulations pick theirs with $SHELL.
    #[cfg(test)]
    if let Ok(sh) = std::env::var("SHELL") {
        return sh;
    }
    #[cfg(unix)]
    {
        let pw = unsafe { libc::getpwuid(crate::compat::sys::getuid()) };
        let from_db = (!pw.is_null()).then(|| unsafe { std::ffi::CStr::from_ptr((*pw).pw_shell) }.to_string_lossy().into_owned());
        from_db.or_else(|| std::env::var("SHELL").ok()).filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/sh".into())
    }
    // Windows: PowerShell 7 when installed, else the Windows PowerShell every version ships.
    #[cfg(windows)]
    {
        let on_path = std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join("pwsh.exe").is_file()));
        if on_path { "pwsh.exe" } else { "powershell.exe" }.into()
    }
}

/// Cloud drives mounted with rclone (`overlay/gdrive.sh`), shown in the sidebar's Storage:
/// each `fuse.rclone` mount, a Google Drive one named so. Remembered for `cloud_name`.
pub fn cloud_places() -> Vec<(String, PathBuf)> {
    let v = cloud_places_in(&std::fs::read_to_string(crate::drives::mounts_table()).unwrap_or_default());
    *CLOUDS.lock().unwrap_or_else(|e| e.into_inner()) = v.clone();
    v
}

static CLOUDS: std::sync::Mutex<Vec<(String, PathBuf)>> = std::sync::Mutex::new(Vec::new());

/// The name of the cloud drive mounted at `path`, as the last `cloud_places` found it.
pub fn cloud_name(path: &std::path::Path) -> Option<String> {
    CLOUDS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(_, p)| p == path).map(|(n, _)| n.clone())
}

/// Storage's mounts from a refresh, where one left unmeasured (the quick pass skips network
/// and FUSE mounts, whose free space can take Google many seconds to answer) keeps the size
/// it had, so its meter stays put instead of vanishing every few seconds.
pub fn keep_space(old: &[(PathBuf, String, u64, u64)], new: Vec<(PathBuf, String, u64, u64)>) -> Vec<(PathBuf, String, u64, u64)> {
    new.into_iter()
        .map(|(p, l, t, f)| match old.iter().find(|o| o.0 == p && o.2 > 0) {
            Some(o) if t == 0 => (p, l, o.2, o.3),
            _ => (p, l, t, f),
        })
        .collect()
}

pub fn cloud_places_in(table: &str) -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = vec![];
    for line in table.lines() {
        let mut it = line.split_whitespace();
        let (Some(remote), Some(mp), Some("fuse.rclone")) = (it.next(), it.next(), it.next()) else { continue };
        let path = PathBuf::from(mp.replace("\\040", " "));
        let base = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let google = remote.to_lowercase().starts_with("gdrive") || base.to_lowercase().replace([' ', '-', '_'], "") == "googledrive";
        let name = if google { "Google Drive".to_string() } else { base };
        if !v.iter().any(|(_, p)| *p == path) {
            v.push((name, path));
        }
    }
    v
}

/// How well a search's words fit `e`, in bands the list is ordered by (the chosen sort
/// orders each band): the whole name, the name's start, anywhere in the name, then a file
/// in a folder whose name holds them; last a close typo of the name (its letters near each
/// other). Letters scattered over a path are no match.
pub fn search_score(q: &crate::fuzzy::Query, e: &crate::fsx::Entry) -> Option<i32> {
    let t = q.text();
    if t.is_empty() {
        return Some(0);
    }
    let name = e.lname();
    // The words were joined without spaces: "file manager" finds "File Manager".
    let squeezed;
    let name = if name.contains(char::is_whitespace) {
        squeezed = name.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        squeezed.as_str()
    } else {
        name
    };
    if name == t {
        return Some(4000);
    }
    if name.starts_with(t) {
        return Some(3000);
    }
    if name.contains(t) {
        return Some(2000);
    }
    if !e.is_dir {
        if let Some(rel) = e.rel() {
            let dirs = rel.rsplit_once('/').map_or("", |(d, _)| d);
            if crate::query::contains_ci(dirs, t) {
                return Some(1000);
            }
        }
    }
    // A typo: the letters in order, spread over at most half as many again. The quick
    // test first, as most names fail it.
    crate::fuzzy::score(q, name)?;
    let (s, pos) = crate::fuzzy::positions(q, name)?;
    let n = t.chars().count();
    let span = pos.last()? - pos.first()? + 1;
    (n >= 3 && span <= n + n / 2).then(|| s.clamp(0, 900))
}

/// Kiro's program: on PATH, or where its installers put it.
#[cfg(windows)]
pub fn kiro() -> Option<PathBuf> {
    // Kiro's installer: per user under %LOCALAPPDATA%\Programs\Kiro, or for all users under
    // Program Files; its `bin\kiro.cmd` on PATH sits beside the app's folder.
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let on_path = std::env::var_os("PATH").into_iter().flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).filter(|d| d.join("kiro.cmd").is_file()).filter_map(|d| d.parent().map(|p| p.join("Kiro.exe")));
    on_path
        .chain(env("LOCALAPPDATA").map(|d| d.join("Programs\\Kiro\\Kiro.exe")))
        .chain(env("ProgramFiles").map(|d| d.join("Kiro\\Kiro.exe")))
        .find(|f| f.is_file())
}

/// Windows: the folder goes to Kiro.exe itself (the `kiro.cmd` shim would flash a console).
#[cfg(windows)]
pub fn open_kiro(dir: &std::path::Path) -> Result<(), String> {
    let exe = kiro().ok_or("Kiro isn't installed (no Kiro.exe in Programs or Program Files)")?;
    let mut c = std::process::Command::new(exe);
    c.arg(dir).current_dir(dir);
    crate::mime::spawn_detached(c)
}

#[cfg(unix)]
pub fn kiro() -> Option<PathBuf> {
    let home = crate::fsx::home();
    let path = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default();
    path.into_iter()
        .chain([home.join(".local/bin"), home.join(".kiro/bin"), PathBuf::from("/opt/kiro/bin"), PathBuf::from("/usr/share/kiro/bin"), PathBuf::from("/opt/Kiro/bin")])
        .map(|d| d.join("kiro"))
        .find(|f| f.is_file())
}

/// Open `dir` in Kiro, as `kiro ./` run there; through the login shell when Kiro is
/// only on the shell's own PATH.
#[cfg(unix)]
pub fn open_kiro(dir: &std::path::Path) -> Result<(), String> {
    let mut c = match kiro() {
        Some(exe) => {
            let mut c = std::process::Command::new(exe);
            c.arg("./");
            c
        }
        None => {
            let mut c = std::process::Command::new(login_shell());
            c.args(["-l", "-c", "command -v kiro >/dev/null && exec kiro ./"]);
            c
        }
    };
    c.current_dir(dir);
    crate::mime::spawn_detached(c)
}

/// Text and marks drawn on a filled accent: Catppuccin's pastels want a dark ink, the
/// deeper Latte accents white.
pub fn on_accent(p: &Palette) -> Color32 {
    let a = p.accent;
    let lum = (0.2126 * a.r() as f32 + 0.7152 * a.g() as f32 + 0.0722 * a.b() as f32) / 255.0;
    if lum > 0.5 { Color32::from_rgb(0x11, 0x11, 0x1b) } else { Color32::WHITE }
}

/// A fully rounded shape filling `r`.
pub fn pill(p: &Painter, r: Rect, fill: Color32) {
    p.rect_filled(r, CornerRadius::same((r.height().min(r.width()) / 2.0).min(255.0) as u8), fill);
}

/// A recessed card inside a panel card (the preview pane): a step towards the backdrop.
pub fn well(p: &Painter, r: Rect, pal: &Palette) {
    p.rect_filled(r, CornerRadius::same(INNER_R), mix(pal.panel, pal.bg, 0.45));
}

/// An icon button's side in this row: 34 px in the toolbars, shrinking to the old 26 in
/// shorter bars.
pub fn button_size(ui: &egui::Ui) -> f32 {
    ui.available_height().clamp(26.0, 34.0)
}

/// The glyph inside a button of side `s`.
pub fn icon_size(s: f32) -> f32 {
    (s * 0.53).clamp(15.0, 18.0)
}

/// The corner radius that makes `r` a pill.
pub fn pill_r(r: Rect) -> CornerRadius {
    CornerRadius::same((r.height().min(r.width()) / 2.0).min(255.0) as u8)
}

/// The commands the palette offers: Maia's, less the recent-files ones Mochi left out
/// (and bookmarks, but on Windows).
pub static COMMANDS: std::sync::LazyLock<Vec<crate::commands::Cmd>> = std::sync::LazyLock::new(|| {
    use crate::commands::Cmd;
    crate::commands::ALL.iter().copied().filter(|c| !matches!(c, Cmd::Timeline | Cmd::ToggleRecents | Cmd::ToggleRecentsOnTop | Cmd::ToggleReadme) && (cfg!(windows) || *c != Cmd::ToggleBookmark)).collect()
});

// ------------------------------------------------------------------ updating

/// Where `mochi.sh` and the patch live: set by the build script, else beside the tree.
fn overlay() -> PathBuf {
    match option_env!("MOCHI_OVERLAY") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../overlay"),
    }
}

/// Run `mochi.sh <what>`: Ok(stdout) or Err(the last lines it complained with).
fn script(what: &str) -> Result<String, String> {
    let sh = overlay().join("mochi.sh");
    if !sh.is_file() {
        return Err(format!("{} is missing; update from a terminal", sh.display()));
    }
    let out = std::process::Command::new("sh").arg(&sh).arg(what).stdin(std::process::Stdio::null()).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = err.lines().filter(|l| !l.trim().is_empty()).collect();
        Err(tail[tail.len().saturating_sub(3)..].join("\n"))
    }
}

/// `mochi.sh check` prints how many upstream commits are new, then the version they
/// bring, then their subjects.
fn parse_check(out: &str) -> State {
    let mut lines = out.lines();
    let n: usize = lines.next().and_then(|l| l.trim().parse().ok()).unwrap_or(0);
    if n == 0 {
        return State::Latest;
    }
    let version = lines.next().unwrap_or("").trim().to_string();
    let notes = lines.collect::<Vec<_>>().join("\n");
    let label = format!("Maia {version} (+{n} commit{})", if n == 1 { "" } else { "s" });
    State::Available(Release { version: label, notes, bin: String::new(), sum: String::new(), assets: vec![] })
}

/// Fetch the upstream clone and say whether it moved, on a thread.
pub fn check(state: Arc<Mutex<State>>, ctx: egui::Context) {
    *state.lock().unwrap_or_else(|e| e.into_inner()) = State::Checking;
    std::thread::spawn(move || {
        let next = match script("check") {
            Ok(out) => parse_check(&out),
            Err(e) => State::Failed(e),
        };
        *state.lock().unwrap_or_else(|e| e.into_inner()) = next;
        ctx.request_repaint();
    });
}

/// Pull, patch, build and install, on a thread; Ready once the new program is in place.
pub fn install(state: Arc<Mutex<State>>, version: String, ctx: egui::Context) {
    *state.lock().unwrap_or_else(|e| e.into_inner()) = State::Installing(version.clone());
    std::thread::spawn(move || {
        let next = match script("update") {
            Ok(_) => State::Ready(version),
            Err(e) => State::Failed(e),
        };
        *state.lock().unwrap_or_else(|e| e.into_inner()) = next;
        ctx.request_repaint();
    });
}

/// `mochi --update`, waited for.
pub fn update_now() -> Result<String, String> {
    match parse_check(&script("check")?) {
        State::Available(r) => {
            script("update")?;
            Ok(format!("Updated to {}; start Mochi again to use it", r.version))
        }
        _ => Ok("Mochi is up to date with Maia".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_output() {
        assert_eq!(parse_check("0\n"), State::Latest);
        match parse_check("2\n0.4.1\nFix a\nFix b\n") {
            State::Available(r) => {
                assert_eq!(r.version, "Maia 0.4.1 (+2 commits)");
                assert_eq!(r.notes, "Fix a\nFix b");
            }
            s => panic!("{s:?}"),
        }
    }

    #[test]
    fn rclone_mounts() {
        let t = "gdrive: /home/b/GoogleDrive fuse.rclone rw,nosuid 0 0\nwork: /home/b/My\\040Box fuse.rclone rw 0 0\n/dev/sda1 / ext4 rw 0 0\n";
        let v = cloud_places_in(t);
        assert_eq!(v, vec![("Google Drive".to_string(), PathBuf::from("/home/b/GoogleDrive")), ("My Box".to_string(), PathBuf::from("/home/b/My Box"))]);
    }

    #[test]
    fn unmeasured_mount_keeps_its_space() {
        let d = PathBuf::from("/home/b/GoogleDrive");
        let old = vec![(d.clone(), "Google Drive".to_string(), 100, 40)];
        let quick = vec![(d.clone(), "Google Drive".to_string(), 0, 0), (PathBuf::from("/usb"), "USB".to_string(), 0, 0)];
        assert_eq!(keep_space(&old, quick), vec![(d.clone(), "Google Drive".to_string(), 100, 40), (PathBuf::from("/usb"), "USB".to_string(), 0, 0)]);
        let full = vec![(d.clone(), "Google Drive".to_string(), 100, 30)];
        assert_eq!(keep_space(&old, full)[0].3, 30);
    }

    #[test]
    fn accents() {
        set_accent("blue");
        assert_eq!(accent_index(), 12);
        set_accent("nope");
        assert_eq!(accent_index(), 12);
        set_accent(DEFAULT_ACCENT);
    }
}

/// Each place's own Catppuccin hue (an index into `ACCENTS`), so the sidebar reads at a glance.
fn place_hue(g: Glyph) -> Option<usize> {
    Some(match g {
        Glyph::Monitor => 12, // Blue
        Glyph::Home => 6,     // Peach
        Glyph::Desktop => 3,  // Mauve
        Glyph::Down => 8,     // Green
        Glyph::Docs => 11,    // Sapphire
        Glyph::Music => 2,    // Pink
        Glyph::Pics => 7,     // Yellow
        Glyph::Video => 5,    // Maroon
        Glyph::Cloud => 10,   // Sky
        Glyph::Trash => 4,    // Red
        Glyph::Drive => 13,   // Lavender
        _ => return None,
    })
}

/// A place's glyph in its hue on a soft tile of the same hue; other themes borrow Mocha's
/// or Latte's hues. Glyphs without a hue are drawn plain in `plain`.
pub fn place_icon(p: &Painter, ic: Rect, g: Glyph, t: ThemeName, pal: &Palette, plain: Color32) {
    let Some(h) = place_hue(g) else {
        crate::theme::paint_glyph(p, ic, g, plain);
        return;
    };
    let f = flavor(t).unwrap_or(if pal.dark { &MOCHA } else { &LATTE });
    let hue = rgb(f.accents[h]);
    let tile = Rect::from_center_size(ic.center(), Vec2::splat(ic.width() + 7.0));
    p.rect_filled(tile, CornerRadius::same(6), mix(pal.sidebar, hue, if pal.dark { 0.18 } else { 0.2 }));
    crate::theme::paint_glyph(p, ic.shrink(1.0), g, hue);
}

/// Digi Hamishe's regular face for Persian text, else any font fontconfig has for Persian.
/// Windows: Digi Hamishe when installed; else none, and Segoe UI's own Persian is used.
#[cfg(windows)]
pub fn persian_font() -> Option<PathBuf> {
    crate::compat::win::font_file("Digi Hamishe")
}

#[cfg(unix)]
pub fn persian_font() -> Option<PathBuf> {
    let list = |pat: &str| {
        let out = std::process::Command::new("fc-list").args(["--format=%{file}\n", pat]).output().ok()?;
        String::from_utf8_lossy(&out.stdout).lines().next().map(PathBuf::from)
    };
    list("Digi Hamishe:style=Regular").or_else(|| list(":lang=fa:style=Regular"))
}

/// `text` as it should appear on screen: egui neither joins Arabic-script letters nor lays
/// right-to-left runs out from the right, so Persian names get their joined letter forms
/// and visual order here. Any other text comes back as it is.
pub fn visual(text: &str) -> std::borrow::Cow<'_, str> {
    let rtl = |c: char| matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF);
    if !text.chars().any(rtl) {
        return text.into();
    }
    let shaped = ar_reshaper::reshape_line(text);
    let bidi = unicode_bidi::BidiInfo::new(&shaped, None);
    let out: String = bidi.paragraphs.iter().map(|p| bidi.reorder_line(p, p.range.clone()).into_owned()).collect();
    out.into()
}
