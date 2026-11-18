//! Mochi: pictures in the terminal pane, by kitty's graphics protocol
//! (<https://sw.kovidgoyal.net/kitty/graphics-protocol/>). Enough of it for what shells
//! show at start (fastfetch's logo) and image previews: pixels sent inline or as a file,
//! as RGB, RGBA or PNG, zlib-compressed or not, in chunks; shown at the cursor, kept
//! under an id to show again, deleted. Animation, relative placements and Unicode
//! placeholders are not done.

use std::collections::HashMap;
use std::io::Read;
use std::sync::Arc;

use crate::vt::Screen;

/// Largest picture taken, in pixels a side, and largest payload, in bytes.
const MAX_SIDE: usize = 8192;
const MAX_DATA: usize = 64 << 20;

/// A picture shown on the screen: its pixels and the cells it covers, from its top-left
/// one (lasting row number, as `Screen::line`, and column). `span` is its size in cells,
/// fractional when the picture is drawn at its own size.
#[derive(Clone)]
pub struct Image {
    /// Unique per placement: the key for its texture.
    pub serial: u64,
    /// The program's id for the picture (0 when it gave none).
    pub id: u32,
    pub line: u64,
    pub col: usize,
    pub span: (f32, f32),
    /// Shown on the alternate screen (a full-screen program), not the main one.
    pub alt: bool,
    pub size: [usize; 2],
    pub rgba: Arc<Vec<u8>>,
}

impl Image {
    /// Rows it reaches into (at least one).
    pub fn rows(&self) -> u64 {
        (self.span.1.ceil() as u64).max(1)
    }
}

/// The terminal pane's cell in device pixels, as last drawn: new shells are told it as
/// they start (fastfetch sizes its logo by it before anything is drawn).
static CELL_PX: std::sync::Mutex<(f32, f32)> = std::sync::Mutex::new((0.0, 0.0));

pub fn cell_px() -> (f32, f32) {
    *CELL_PX.lock().unwrap_or_else(|e| e.into_inner())
}

/// Sets the cell size; true when it changed.
pub fn set_cell_px(px: (f32, f32)) -> bool {
    let mut c = CELL_PX.lock().unwrap_or_else(|e| e.into_inner());
    std::mem::replace(&mut *c, px) != px
}

type Pixels = ([usize; 2], Arc<Vec<u8>>);

/// What a screen keeps between commands: a picture still arriving in chunks, and the
/// pictures kept under their ids.
#[derive(Default)]
pub struct Store {
    pending: Option<(Vec<u8>, Vec<u8>)>,
    kept: HashMap<u32, Pixels>,
    serial: u64,
}

/// An APC string's body (`G` then the control data, `;`, the payload in base64).
pub fn command(s: &mut Screen, body: &[u8]) {
    let Some(body) = body.strip_prefix(b"G") else { return };
    let (ctl, data) = match body.iter().position(|&b| b == b';') {
        Some(i) => (&body[..i], &body[i + 1..]),
        None => (body, &b""[..]),
    };
    let more = keys(ctl).get(&b'm').is_some_and(|v| v == "1");
    // A picture in chunks: the first one's keys hold for the whole of it.
    let (ctl, data) = match s.gfx.pending.take() {
        Some((first, mut acc)) => {
            acc.extend_from_slice(data);
            if acc.len() > MAX_DATA * 4 / 3 {
                return;
            }
            if more {
                s.gfx.pending = Some((first, acc));
                return;
            }
            (first, acc)
        }
        None if more => {
            s.gfx.pending = Some((ctl.to_vec(), data.to_vec()));
            return;
        }
        None => (ctl.to_vec(), data.to_vec()),
    };
    run(s, &keys(&ctl), &data);
}

fn keys(ctl: &[u8]) -> HashMap<u8, String> {
    ctl.split(|&b| b == b',')
        .filter_map(|kv| {
            let (k, v) = kv.split_first()?;
            Some((*k, String::from_utf8_lossy(v.strip_prefix(b"=")?).into_owned()))
        })
        .collect()
}

fn run(s: &mut Screen, k: &HashMap<u8, String>, data: &[u8]) {
    let num = |key: u8| k.get(&key).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    let action = k.get(&b'a').map_or("t", String::as_str);
    let (id, quiet) = (num(b'i'), num(b'q'));
    let mut reply = |s: &mut Screen, msg: &str| {
        if id != 0 && quiet < 2 && !(quiet == 1 && msg == "OK") {
            s.replies.extend_from_slice(format!("\x1b_Gi={id};{msg}\x1b\\").as_bytes());
        }
    };
    match action {
        "d" => {
            match k.get(&b'd').map_or("a", String::as_str) {
                "i" | "I" => s.images.retain(|im| im.id != id),
                _ => s.images.clear(),
            }
            if k.get(&b'd').is_some_and(|d| d == "I") {
                s.gfx.kept.remove(&id);
            }
        }
        "p" => match s.gfx.kept.get(&id).cloned() {
            Some(px) => place(s, k, id, px),
            None => reply(s, "ENOENT:no such image"),
        },
        "q" | "t" | "T" => match decode(k, data) {
            Ok(px) => {
                if action != "q" && id != 0 {
                    // A handful kept: a program that sends many keeps none of them long.
                    if s.gfx.kept.len() >= 32 {
                        s.gfx.kept.clear();
                    }
                    s.gfx.kept.insert(id, px.clone());
                }
                if action == "T" {
                    place(s, k, id, px);
                }
                reply(s, "OK");
            }
            Err(e) => reply(s, e),
        },
        _ => {}
    }
}

/// Show `px` at the cursor and move the cursor past it, as kitty does: to the column
/// after it, on its last row.
fn place(s: &mut Screen, k: &HashMap<u8, String>, id: u32, (size, rgba): Pixels) {
    let num = |key: u8| k.get(&key).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) as f32;
    let (cw, ch) = s.cell_px;
    let (cw, ch) = if cw > 0.0 && ch > 0.0 { (cw, ch) } else { (8.0, 16.0) };
    let (c, r) = (num(b'c'), num(b'r'));
    let mut span = (size[0] as f32 / cw, size[1] as f32 / ch);
    match (c > 0.0, r > 0.0) {
        (true, true) => span = (c, r),
        (true, false) => span = (c, c * span.1 / span.0.max(0.01)),
        (false, true) => span = (r * span.0 / span.1.max(0.01), r),
        _ => {}
    }
    // Pixels a hair over a whole cell are rounding, not another cell.
    let snap = |v: f32| if (v - v.round()).abs() < 0.05 { v.round() } else { v };
    span = (snap(span.0).max(0.1), snap(span.1).max(0.1));
    s.gfx.serial += 1;
    let (col, _) = s.cursor();
    let im = Image { serial: s.gfx.serial, id, line: s.cursor_line(), col, span, alt: s.alt(), size, rgba };
    let rows = im.rows() as usize;
    s.images.push(im);
    if s.images.len() > 64 {
        s.images.remove(0);
    }
    if num(b'C') != 1.0 {
        s.move_past(span.0.ceil() as usize, rows);
    }
}

fn decode(k: &HashMap<u8, String>, data: &[u8]) -> Result<Pixels, &'static str> {
    let num = |key: u8| k.get(&key).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
    let raw = base64(data).ok_or("EINVAL:bad base64")?;
    let bytes = match k.get(&b't').map_or("d", String::as_str) {
        "d" => raw,
        // A file the program wrote; a temporary one (t) is removed once read.
        m @ ("f" | "t") => {
            let path = std::path::PathBuf::from(String::from_utf8(raw).map_err(|_| "EINVAL:bad path")?);
            let mut b = vec![];
            std::fs::File::open(&path).and_then(|f| f.take(MAX_DATA as u64).read_to_end(&mut b)).map_err(|_| "EBADF:cannot read file")?;
            if m == "t" && path.to_string_lossy().contains("tty-graphics-protocol") {
                let _ = std::fs::remove_file(&path);
            }
            b
        }
        _ => return Err("EINVAL:unsupported transmission"),
    };
    let bytes = if k.get(&b'o').is_some_and(|o| o == "z") {
        let mut out = vec![];
        flate2::read::ZlibDecoder::new(&bytes[..]).take(MAX_DATA as u64).read_to_end(&mut out).map_err(|_| "EINVAL:bad zlib data")?;
        out
    } else {
        bytes
    };
    let (w, h) = (num(b's'), num(b'v'));
    let px = match k.get(&b'f').map_or("32", String::as_str) {
        "100" => {
            let img = crate::imgdec::decode_bytes(&bytes).ok_or("EINVAL:bad PNG")?;
            ([img.w as usize, img.h as usize], img.px)
        }
        f @ ("24" | "32") => {
            let n = if f == "24" { 3 } else { 4 };
            if w == 0 || h == 0 || bytes.len() < w * h * n {
                return Err("EINVAL:size does not match the data");
            }
            let px = if n == 4 { bytes[..w * h * 4].to_vec() } else { bytes[..w * h * 3].chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect() };
            ([w, h], px)
        }
        _ => return Err("EINVAL:unsupported format"),
    };
    if px.0[0] == 0 || px.0[1] == 0 || px.0[0] > MAX_SIDE || px.0[1] > MAX_SIDE {
        return Err("EINVAL:bad size");
    }
    Ok((px.0, Arc::new(px.1)))
}

/// Standard base64, padding optional, whitespace skipped.
pub fn base64(s: &[u8]) -> Option<Vec<u8>> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for &c in s.iter().filter(|c| !c.is_ascii_whitespace()) {
        if c == b'=' {
            break;
        }
        acc = acc << 6 | val(c)? as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// A run of ordinary output, or a finished APC string's body.
pub enum Seg<'a> {
    Text(&'a [u8]),
    Apc(&'a [u8]),
}

/// Splits a program's output into what the parser takes and kitty's APC strings
/// (`ESC _ … ESC \`), which `vte` drops. Carries a string, or an escape that may start
/// one, from one read to the next.
#[derive(Default)]
pub struct Apc {
    buf: Option<Vec<u8>>,
    esc: bool,
}

impl Apc {
    /// Calls `f` with each run of ordinary output and each finished string, in order.
    pub fn split(&mut self, bytes: &[u8], mut f: impl FnMut(Seg)) {
        let mut start = 0;
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if let Some(buf) = &mut self.buf {
                if self.esc {
                    self.esc = false;
                    if b == b'\\' {
                        let body = self.buf.take().unwrap_or_default();
                        f(Seg::Apc(&body));
                        i += 1;
                        start = i;
                        continue;
                    }
                    buf.push(0x1b);
                }
                if b == 0x1b {
                    self.esc = true;
                } else if buf.len() < MAX_DATA * 2 {
                    buf.push(b);
                }
                i += 1;
                start = i;
                continue;
            }
            if self.esc {
                // An escape ended the last read.
                self.esc = false;
                if b == b'_' {
                    self.buf = Some(vec![]);
                    i += 1;
                    start = i;
                    continue;
                }
                f(Seg::Text(b"\x1b"));
            }
            if b == 0x1b {
                match bytes.get(i + 1) {
                    Some(b'_') => {
                        f(Seg::Text(&bytes[start..i]));
                        self.buf = Some(vec![]);
                        i += 2;
                        start = i;
                        continue;
                    }
                    None => {
                        f(Seg::Text(&bytes[start..i]));
                        self.esc = true;
                        i += 1;
                        start = i;
                        continue;
                    }
                    _ => {}
                }
            }
            i += 1;
        }
        if self.buf.is_none() && start < bytes.len() {
            f(Seg::Text(&bytes[start..]));
        }
    }
}
