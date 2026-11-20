//! Mochi: a folder's own colour and symbol, kept on the folder in the `user.mochi.look`
//! extended attribute ("Peach;rocket_launch"), so it moves and renames with the folder.
//!
//! The art follows the kora icon theme's folder: a darker back, a front that brightens
//! towards its top, and the symbol pressed into the front in a deep tone of the same
//! colour. Each colour is a Catppuccin accent, its Latte shade at the bottom of the front
//! rising to its Mocha shade at the top. A folder with a symbol and no colour keeps the
//! theme's blue.
//!
//! Attributes are read off the UI thread, as tags are: a row asks for its folder's look
//! when drawn, a worker reads it, and the cache keeps what was found.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, mpsc};

use egui::{Color32, ColorImage, CornerRadius, FontId, Painter, Rect, Sense, Stroke, TextureHandle, TextureOptions, Ui, pos2, vec2};

use crate::app::App;
use crate::fsx::Entry;
use crate::popups as pp;

pub use crate::folderglyphs::GROUPS;

const ATTR: &[u8] = b"user.mochi.look\0";

/// (name, Latte, Mocha).
pub const COLOURS: [(&str, [u8; 3], [u8; 3]); 14] = [
    ("Rosewater", [0xdc, 0x8a, 0x78], [0xf5, 0xe0, 0xdc]),
    ("Flamingo", [0xdd, 0x78, 0x78], [0xf2, 0xcd, 0xcd]),
    ("Pink", [0xea, 0x76, 0xcb], [0xf5, 0xc2, 0xe7]),
    ("Mauve", [0x88, 0x39, 0xef], [0xcb, 0xa6, 0xf7]),
    ("Red", [0xd2, 0x0f, 0x39], [0xf3, 0x8b, 0xa8]),
    ("Maroon", [0xe6, 0x45, 0x53], [0xeb, 0xa0, 0xac]),
    ("Peach", [0xfe, 0x64, 0x0b], [0xfa, 0xb3, 0x87]),
    ("Yellow", [0xdf, 0x8e, 0x1d], [0xf9, 0xe2, 0xaf]),
    ("Green", [0x40, 0xa0, 0x2b], [0xa6, 0xe3, 0xa1]),
    ("Teal", [0x17, 0x92, 0x99], [0x94, 0xe2, 0xd5]),
    ("Sky", [0x04, 0xa5, 0xe5], [0x89, 0xdc, 0xeb]),
    ("Sapphire", [0x20, 0x9f, 0xb5], [0x74, 0xc7, 0xec]),
    ("Blue", [0x1e, 0x66, 0xf5], [0x89, 0xb4, 0xfa]),
    ("Lavender", [0x72, 0x87, 0xfd], [0xb4, 0xbe, 0xfe]),
];

/// kora's folder: back, front bottom, front top.
const THEME: [[u8; 3]; 3] = [[0, 131, 213], [16, 117, 246], [18, 197, 255]];

/// kora's folder outline in a 64-unit square: the back, then the front with its tab.
const BACK: &str = "m61.122 15.88c0-2.762-2.239-5-5-5h-48.244c-2.761 0-5 2.238-5 5v32.246c0 2.761 2.239 5 5 5h48.244c2.761 0 5-2.239 5-5v-32.246z";
const FRONT: &str = "m61.122 20.652c0-1.326-0.527-2.598-1.465-3.536-0.938-0.937-2.209-1.464-3.535-1.464h-25.58c-1.232 0-2.42-0.455-3.337-1.277-0.768-0.689-1.713-1.535-2.481-2.224-0.917-0.822-2.105-1.277-3.337-1.277h-13.509c-1.326 0-2.597 0.527-3.535 1.465-0.938 0.937-1.465 2.209-1.465 3.535v32.252c0 2.761 2.239 5 5 5h48.244c2.761 0 5-2.239 5-5v-27.474z";

#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct Look {
    /// A name from `COLOURS`, or empty for the theme's own.
    pub colour: String,
    /// An id from `GROUPS`, or empty for none.
    pub symbol: String,
}

impl Look {
    pub fn parse(s: &str) -> Look {
        let (c, s) = s.trim().split_once(';').unwrap_or((s.trim(), ""));
        let colour = COLOURS.iter().find(|x| x.0.eq_ignore_ascii_case(c.trim())).map(|x| x.0.to_string()).unwrap_or_default();
        let symbol = symbol(s.trim()).map(|x| x.0.to_string()).unwrap_or_default();
        Look { colour, symbol }
    }
    pub fn attr(&self) -> String {
        if self.is_plain() { String::new() } else { format!("{};{}", self.colour, self.symbol) }
    }
    pub fn is_plain(&self) -> bool {
        self.colour.is_empty() && self.symbol.is_empty()
    }
    /// "Peach, Rocket", "Peach", "Rocket".
    pub fn describe(&self) -> String {
        let s = symbol(&self.symbol).map(|x| x.1).unwrap_or("");
        match (self.colour.as_str(), s) {
            ("", "") => "the usual look".into(),
            (c, "") => c.into(),
            ("", s) => s.into(),
            (c, s) => format!("{c}, {s}"),
        }
    }
    /// The three tones of its folder: back, front bottom, front top.
    fn tones(&self) -> [[u8; 3]; 3] {
        match COLOURS.iter().find(|x| x.0 == self.colour) {
            Some((_, l, m)) => [mix(*l, [0, 0, 0], 0.18), *l, mix(*l, *m, 0.75)],
            None => THEME,
        }
    }
    /// The deep tone its symbol (and a name on it) is pressed in.
    pub fn ink(&self) -> Color32 {
        let [_, lo, _] = self.tones();
        let [r, g, b] = mix(lo, [0, 0, 0], if self.colour.is_empty() { 0.55 } else { 0.5 });
        Color32::from_rgb(r, g, b)
    }
}

/// A symbol by its id: (id, name, path).
pub fn symbol(id: &str) -> Option<&'static (&'static str, &'static str, &'static str)> {
    if id.is_empty() {
        return None;
    }
    GROUPS.iter().flat_map(|g| g.1.iter()).find(|s| s.0 == id)
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// The folder as an SVG, with its symbol pressed in when `with_symbol`.
fn folder_svg(look: &Look, with_symbol: bool) -> String {
    let [back, lo, hi] = look.tones();
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\"><defs><linearGradient id=\"f\" x1=\"0\" y1=\"1\" x2=\"0\" y2=\"0\"><stop offset=\"0\" stop-color=\"{}\"/><stop offset=\"1\" stop-color=\"{}\"/></linearGradient><linearGradient id=\"s\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\"><stop offset=\"0\" stop-color=\"{}\"/><stop offset=\"1\" stop-color=\"{}\"/></linearGradient></defs><path d=\"{BACK}\" fill=\"{}\"/><path d=\"{FRONT}\" fill=\"url(#f)\"/>",
        hex(lo),
        hex(hi),
        hex(mix(lo, [0, 0, 0], 0.3)),
        hex(mix(lo, [0, 0, 0], if look.colour.is_empty() { 0.55 } else { 0.47 })),
        hex(back),
    );
    if let Some(sym) = symbol(&look.symbol).filter(|_| with_symbol) {
        // 25 units across, a little below the front's middle as kora's own symbols sit.
        s.push_str(&format!("<g transform=\"translate(19.5 22.5) scale({})\"><path d=\"{}\" fill=\"url(#s)\" fill-opacity=\"0.92\"/></g>", 25.0 / 24.0, sym.2));
    }
    s.push_str("</svg>");
    s
}

/// A symbol alone, white (painted tinted).
fn symbol_svg(id: &str) -> Option<String> {
    let sym = symbol(id)?;
    Some(format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><path d=\"{}\" fill=\"#ffffff\"/></svg>", sym.2))
}

// ------------------------------------------------------------------ textures

#[derive(Clone, PartialEq, Eq, Hash)]
enum Art {
    Folder(Look, bool),
    Symbol(String),
}

fn textures() -> &'static Mutex<HashMap<(Art, u32), Option<TextureHandle>>> {
    static T: OnceLock<Mutex<HashMap<(Art, u32), Option<TextureHandle>>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(HashMap::new()))
}

fn texture(ctx: &egui::Context, art: Art, px: u32) -> Option<TextureHandle> {
    let mut t = textures().lock().unwrap_or_else(|e| e.into_inner());
    t.entry((art.clone(), px))
        .or_insert_with(|| {
            let svg = match &art {
                Art::Folder(l, s) => folder_svg(l, *s),
                Art::Symbol(id) => symbol_svg(id)?,
            };
            let (w, h, rgba) = crate::svg::render(svg.as_bytes(), px)?;
            Some(ctx.load_texture(format!("mochi look {px}"), ColorImage::from_rgba_premultiplied([w as usize, h as usize], &rgba), TextureOptions::LINEAR))
        })
        .clone()
}

fn pixels(p: &Painter, side: f32) -> u32 {
    let raw = (side * p.ctx().pixels_per_point()).round().max(8.0) as u32;
    if raw <= 64 { raw } else { raw.div_ceil(16) * 16 }
}

fn paint_art(p: &Painter, rect: Rect, art: Art, tint: Color32) {
    let side = rect.width().min(rect.height());
    if let Some(t) = texture(p.ctx(), art, pixels(p, side)) {
        let r = Rect::from_center_size(rect.center(), vec2(side, side));
        p.image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
    }
}

/// A folder in this look, its symbol pressed in when `with_symbol`.
pub fn paint_look(p: &Painter, rect: Rect, look: &Look, with_symbol: bool) {
    paint_art(p, rect, Art::Folder(look.clone(), with_symbol), Color32::WHITE);
}

/// A symbol alone in `col`.
pub fn paint_symbol(p: &Painter, rect: Rect, id: &str, col: Color32) {
    paint_art(p, rect, Art::Symbol(id.to_string()), col);
}

/// The look of `e` if it is a folder that has one (read already).
pub fn of(e: &Entry) -> Option<Look> {
    if !e.is_dir {
        return None;
    }
    get(&e.path).filter(|l| !l.is_plain())
}

/// Draw `e` in its own look; false when it has none (the caller draws the usual icon).
pub fn paint(p: &Painter, rect: Rect, e: &Entry) -> bool {
    frame(p.ctx());
    // Windows has no icon theme: every folder is drawn in this art, plain ones in its blue.
    let look = match of(e) {
        Some(l) => l,
        None if cfg!(windows) && e.is_dir => Look::default(),
        None => return false,
    };
    paint_look(p, rect, &look, true);
    if e.is_link {
        crate::theme::paint_link_badge(p, rect);
    }
    true
}

/// The front of a folder icon drawn in `icon` (kora's shape): where a name on it goes.
pub fn face(icon: Rect) -> Rect {
    let s = icon.width().min(icon.height());
    let o = icon.center() - vec2(s, s) / 2.0;
    Rect::from_min_max(o + vec2(s * 0.08, s * 0.33), o + vec2(s * 0.92, s * 0.8))
}

/// Mochi test: a folder with its name on its front. The look's colour (or the theme's
/// folder), its symbol small in the front's corner, the name pressed in its ink.
/// Returns whether the name was cut short.
pub fn paint_named(ui: &Ui, icon: Rect, e: &Entry, name: &str, font_size: f32, faded: bool) -> bool {
    let p = ui.painter();
    frame(p.ctx());
    let look = of(e);
    match &look {
        Some(l) => paint_look(p, icon, l, false),
        None if cfg!(windows) => paint_look(p, icon, &Look::default(), false),
        None => {
            if !crate::icons::entry(p, icon, e) {
                crate::theme::paint_icon(p, icon, crate::theme::FileCat::Folder, "", e.is_link);
            }
        }
    }
    let ink = look.as_ref().map_or(Color32::WHITE, |l| l.ink());
    let ink = if faded { ink.gamma_multiply(0.45) } else { ink };
    let f = face(icon);
    let s = icon.width();
    let mut room = f;
    if let Some(l) = look.as_ref().filter(|l| !l.symbol.is_empty()) {
        let k = (s * 0.16).max(10.0);
        let sr = Rect::from_min_size(pos2(f.right() - k - s * 0.01, f.bottom() - k + s * 0.01), vec2(k, k));
        paint_symbol(p, sr, &l.symbol, ink.gamma_multiply(0.75));
        room = Rect::from_min_max(f.min, pos2(f.max.x, f.max.y - k * 0.6));
    }
    let size = (s * 0.125).clamp(10.0, font_size + 1.0);
    let rows = ((room.height() / (size * 1.25)).floor() as usize).clamp(1, 3);
    let mut job = egui::text::LayoutJob::single_section(
        crate::mochi::visual(name).into_owned(),
        egui::TextFormat { font_id: FontId::proportional(size), color: ink, line_height: Some(size * 1.2), ..Default::default() },
    );
    job.wrap = egui::text::TextWrapping { max_width: room.width() - 6.0, max_rows: rows, break_anywhere: false, overflow_character: Some('…') };
    job.halign = egui::Align::Center;
    let g = ui.fonts_mut(|fo| fo.layout_job(job));
    let y = room.center().y - g.size().y / 2.0;
    let cut = g.elided;
    p.galley(pos2(room.center().x, y), g, ink);
    cut
}

// ------------------------------------------------------------------ on disk

#[cfg(unix)]
fn cpath(p: &Path) -> Option<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(p.as_os_str().as_bytes()).ok()
}

/// Windows: the look is kept in the folder's NTFS stream `mochi.look` ("Folder:mochi.look"),
/// which moves and renames with it as the attribute does on Linux.
#[cfg(windows)]
fn stream(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(":mochi.look");
    PathBuf::from(s)
}

#[cfg(windows)]
pub fn read(p: &Path) -> Look {
    std::fs::read_to_string(stream(p)).map(|s| Look::parse(&s)).unwrap_or_default()
}

#[cfg(windows)]
pub fn write(p: &Path, look: &Look) -> Result<(), String> {
    let v = look.attr();
    let r = if v.is_empty() {
        match std::fs::remove_file(stream(p)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            r => r,
        }
    } else {
        std::fs::write(stream(p), v.as_bytes())
    };
    match r {
        Ok(()) => {
            set_cached(p, look.clone());
            Ok(())
        }
        // FAT and exFAT drives have no streams: the name with a colon is refused.
        Err(e) if e.raw_os_error() == Some(123) => Err("this drive can't keep a folder look".into()),
        Err(e) => Err(e.to_string()),
    }
}

/// A folder's look as it is on disk.
#[cfg(unix)]
pub fn read(p: &Path) -> Look {
    let Some(c) = cpath(p) else { return Look::default() };
    let mut buf = [0u8; 256];
    let n = unsafe { libc::getxattr(c.as_ptr(), ATTR.as_ptr().cast(), buf.as_mut_ptr().cast(), buf.len()) };
    if n <= 0 {
        return Look::default();
    }
    Look::parse(&String::from_utf8_lossy(&buf[..n as usize]))
}

/// Give `p` this look (the usual look removes the attribute).
#[cfg(unix)]
pub fn write(p: &Path, look: &Look) -> Result<(), String> {
    let c = cpath(p).ok_or("bad path")?;
    let v = look.attr();
    let r = if v.is_empty() {
        let r = unsafe { libc::removexattr(c.as_ptr(), ATTR.as_ptr().cast()) };
        if r != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ENODATA) { 0 } else { r }
    } else {
        unsafe { libc::setxattr(c.as_ptr(), ATTR.as_ptr().cast(), v.as_ptr().cast(), v.len(), 0) }
    };
    if r == 0 {
        set_cached(p, look.clone());
        Ok(())
    } else {
        let e = std::io::Error::last_os_error();
        Err(if e.raw_os_error() == Some(libc::ENOTSUP) { "this drive can't keep a folder look".into() } else { e.to_string() })
    }
}

// ------------------------------------------------------------------ cache

#[derive(Default)]
struct Cache {
    looks: HashMap<PathBuf, Look>,
    /// Read and found plain (hashes, for big folders).
    bare: HashSet<u64>,
    pending: HashSet<u64>,
    tx: Option<mpsc::Sender<PathBuf>>,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
static REPAINT: OnceLock<egui::Context> = OnceLock::new();

fn key(p: &Path) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    p.hash(&mut h);
    h.finish()
}

/// What is known of `p`'s look; None until it is read (it is asked for then).
pub fn get(p: &Path) -> Option<Look> {
    let k = key(p);
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let c = g.get_or_insert_with(Cache::default);
    if let Some(l) = c.looks.get(p) {
        return Some(l.clone());
    }
    if c.bare.contains(&k) {
        return Some(Look::default());
    }
    if !c.pending.insert(k) {
        return None;
    }
    let tx = c.tx.get_or_insert_with(|| {
        let (tx, rx) = mpsc::channel::<PathBuf>();
        std::thread::Builder::new()
            .name("folder looks".into())
            .spawn(move || {
                while let Ok(p) = rx.recv() {
                    let l = read(&p);
                    set_cached(&p, l);
                    if let Some(ctx) = REPAINT.get() {
                        ctx.request_repaint();
                    }
                }
            })
            .ok();
        tx
    });
    let _ = tx.send(p.to_path_buf());
    None
}

fn set_cached(p: &Path, look: Look) {
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let c = g.get_or_insert_with(Cache::default);
    let k = key(p);
    c.pending.remove(&k);
    if look.is_plain() {
        c.looks.remove(p);
        c.bare.insert(k);
    } else {
        c.bare.remove(&k);
        c.looks.insert(p.to_path_buf(), look);
    }
}

/// Note the context, for repaints when a look is read.
pub fn frame(ctx: &egui::Context) {
    let _ = REPAINT.set(ctx.clone());
}

// ------------------------------------------------------------------ undo

/// A folder's look before and after a change, as stored.
pub type Change = (PathBuf, String, String);

/// Put each folder back to its look before (`undo`) or after the change. Returns the
/// changes that applied, and how many failed.
pub fn restore(ch: &[Change], undo: bool) -> (Vec<Change>, usize) {
    let mut done = vec![];
    let mut failed = 0;
    for (p, before, after) in ch {
        let to = if undo { before } else { after };
        if std::fs::symlink_metadata(p).is_err() {
            continue;
        }
        match write(p, &Look::parse(to)) {
            Ok(()) => done.push((p.clone(), before.clone(), after.clone())),
            Err(_) => failed += 1,
        }
    }
    (done, failed)
}

// ------------------------------------------------------------------ the dialog

pub struct State {
    pub paths: Vec<PathBuf>,
    pub pick: Look,
    /// Words to find a symbol by.
    pub query: String,
}

/// The dialog for these folders, on the look the first one has.
pub fn open(paths: Vec<PathBuf>) -> crate::popups::Popup {
    let pick = paths.first().map(|p| read(p)).unwrap_or_default();
    crate::popups::Popup::FolderLook(State { paths, pick, query: String::new() })
}

fn what(paths: &[PathBuf]) -> String {
    match paths {
        [one] => one.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        v => format!("{} folders", v.len()),
    }
}

/// Apply the look to every folder, as one change Undo takes back.
fn apply(app: &mut App, paths: &[PathBuf], look: &Look) {
    let mut ch: Vec<Change> = vec![];
    let mut err = None;
    for p in paths {
        let before = read(p);
        if before == *look {
            continue;
        }
        match write(p, look) {
            Ok(()) => ch.push((p.clone(), before.attr(), look.attr())),
            Err(e) => {
                err.get_or_insert(e);
            }
        }
    }
    if let Some(e) = err {
        app.toast(format!("Couldn't change the look: {e}"));
    }
    if !ch.is_empty() {
        let label = if look.is_plain() { format!("Reset the look of {}", what(paths)) } else { format!("Restyle {} ({})", what(paths), look.describe()) };
        app.undo.push(crate::app::UndoEntry { label, kind: crate::app::UndoKind::Looked(ch) });
    }
}

pub fn popup(app: &mut App, ctx: &egui::Context, st: &mut State) -> bool {
    frame(ctx);
    let pal = app.pal;
    let k = pp::take_keys(ctx);
    if k.esc {
        return false;
    }
    let mut ok = k.enter;
    let mut cancel = false;
    let mut reset = false;
    let name = what(&st.paths);
    let swatch = 26.0;
    let cell = 40.0;
    let (_, _, rect) = pp::centered_footer(
        ctx,
        "folder look",
        600.0,
        &pal,
        |ui| {
            pp::dialog_title(ui, "Folder look", Some(&name), &pal);
            ui.add_space(10.0);
            // The preview, beside the colours.
            ui.horizontal(|ui| {
                let (pr, _) = ui.allocate_exact_size(vec2(112.0, 112.0), Sense::hover());
                crate::mochi::well(ui.painter(), pr, &pal);
                paint_look(ui.painter(), pr.shrink(14.0), &st.pick, true);
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Colour").color(pal.text_dim));
                    ui.add_space(4.0);
                    let per_row = 8;
                    let all: Vec<Option<usize>> = std::iter::once(None).chain((0..COLOURS.len()).map(Some)).collect();
                    for row in all.chunks(per_row) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            for c in row {
                                let (r, resp) = ui.allocate_exact_size(vec2(swatch, swatch), Sense::click());
                                let (fill, tip) = match c {
                                    Some(i) => (COLOURS[*i].1, COLOURS[*i].0),
                                    None => (THEME[1], if cfg!(windows) { "Mochi's blue" } else { "The icon theme's own" }),
                                };
                                let on = match c {
                                    Some(i) => st.pick.colour == COLOURS[*i].0,
                                    None => st.pick.colour.is_empty(),
                                };
                                let p = ui.painter();
                                let rad = swatch / 2.0 - 3.0;
                                if on {
                                    p.circle_stroke(r.center(), swatch / 2.0 - 0.5, Stroke::new(2.0, pal.accent));
                                } else if resp.hovered() {
                                    p.circle_stroke(r.center(), swatch / 2.0 - 0.5, Stroke::new(1.0, pal.text_dim));
                                }
                                let [a, b, cc] = fill;
                                p.circle_filled(r.center(), rad, Color32::from_rgb(a, b, cc));
                                if c.is_none() {
                                    // The theme's own: a slash through it.
                                    let d = rad * 0.6;
                                    p.line_segment([r.center() + vec2(-d, d), r.center() + vec2(d, -d)], Stroke::new(1.6, Color32::WHITE));
                                }
                                if resp.on_hover_text(tip).clicked() {
                                    st.pick.colour = c.map(|i| COLOURS[i].0.to_string()).unwrap_or_default();
                                }
                            }
                        });
                        ui.add_space(4.0);
                    }
                });
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Symbol").color(pal.text_dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(egui::TextEdit::singleline(&mut st.query).hint_text("Search symbols").desired_width(220.0));
                });
            });
            ui.add_space(4.0);
            let cols = ((ui.available_width() + 4.0) / (cell + 4.0)).floor().max(1.0) as usize;
            // A search shows the matches in one run (a group's name finds its symbols too).
            let q = st.query.trim().to_lowercase();
            let found: Vec<(&str, &str, &str)> = if q.is_empty() {
                vec![]
            } else {
                let q = q.as_str();
                GROUPS.iter().flat_map(|g| g.1.iter().filter(move |s| s.1.to_lowercase().contains(q) || s.0.replace('_', " ").contains(q) || g.0.to_lowercase().contains(q))).copied().collect()
            };
            let none: &[(&str, &str, &str)] = &[("", "None", "")];
            let groups: Vec<(&str, &[(&str, &str, &str)])> = if q.is_empty() { std::iter::once(("", none)).chain(GROUPS.iter().copied()).collect() } else { vec![("", none), ("", &found[..])] };
            if !q.is_empty() && found.is_empty() {
                ui.label(egui::RichText::new("No symbol by that name").small().color(pal.text_dim));
            }
            for (gname, syms) in groups {
                if !gname.is_empty() {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(gname).small().color(pal.text_dim));
                }
                for row in syms.chunks(cols) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for (id, label, _) in row {
                            let (r, resp) = ui.allocate_exact_size(vec2(cell, cell), Sense::click());
                            let on = st.pick.symbol == *id;
                            let p = ui.painter();
                            if on {
                                p.rect_filled(r, CornerRadius::same(10), crate::kit::lift(&pal));
                                p.rect_stroke(r, CornerRadius::same(10), Stroke::new(1.5, pal.accent), egui::StrokeKind::Inside);
                            } else if resp.hovered() {
                                p.rect_filled(r, CornerRadius::same(10), crate::kit::touch(&pal));
                            }
                            let col = if on { pal.accent } else if resp.hovered() { pal.text } else { pal.text_dim };
                            if id.is_empty() {
                                let c = r.center();
                                p.circle_stroke(c, 8.0, Stroke::new(1.5, col));
                                p.line_segment([c + vec2(-5.6, 5.6), c + vec2(5.6, -5.6)], Stroke::new(1.5, col));
                            } else {
                                paint_symbol(p, r.shrink(10.0), id, col);
                            }
                            if resp.on_hover_text(*label).clicked() {
                                st.pick.symbol = id.to_string();
                            }
                        }
                    });
                }
            }
        },
        |ui| {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if pp::quiet_button(ui, "Reset", &pal).clicked() {
                    reset = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if pp::primary_button(ui, "Apply", true, &pal).clicked() {
                        ok = true;
                    }
                    if pp::plain_button(ui, "Cancel", &pal).clicked() {
                        cancel = true;
                    }
                });
            });
        },
    );
    if reset {
        st.pick = Look::default();
    }
    if ok {
        let (paths, look) = (st.paths.clone(), st.pick.clone());
        apply(app, &paths, &look);
        return false;
    }
    !cancel && !pp::clicked_outside(ctx, rect)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_round_trip() {
        let l = Look::parse("peach;rocket_launch");
        assert_eq!(l, Look { colour: "Peach".into(), symbol: "rocket_launch".into() });
        assert_eq!(Look::parse(&l.attr()), l);
        assert!(Look::parse("nonsense;nothing").is_plain());
        assert_eq!(Look::parse(";git").describe(), "Git");
        assert_eq!(Look::default().attr(), "");
    }

    #[test]
    fn every_folder_renders() {
        for g in GROUPS {
            for s in g.1 {
                let l = Look { colour: "Teal".into(), symbol: s.0.into() };
                assert!(crate::svg::render(folder_svg(&l, true).as_bytes(), 48).is_some(), "{}", s.0);
            }
        }
    }
}
