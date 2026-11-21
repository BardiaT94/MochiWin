//! Windows: Mochi as its own `bsdtar` (`mochi.exe --bsdtar ...`), for the archive code,
//! which drives bsdtar's command line (archive.rs, archwrite.rs). Zip is read and written
//! here; any other kind goes to the tar.exe Windows ships (libarchive's bsdtar too), which
//! is left out for zip because it passes names through the system's code page and loses
//! any outside it (Persian, on an English Windows).
//!
//! Understood: -t, -x, -c with v, O, f, C, T, z, J, j, --zstd, --format, --options,
//! --passphrase and `--`, as archive.rs and archwrite.rs use them. A password is read from
//! the input when --passphrase doesn't give one, as bsdtar does. `--bsdcat` decompresses a
//! lone .gz file to the output.

use std::ffi::{OsStr, OsString};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use zip::write::SimpleFileOptions;
use zip::{AesMode, CompressionMethod, DateTime, ZipArchive, ZipWriter};

/// What archive.rs passes when it knows no password (`archive::NO_PASSWORD`).
const NO_PASSWORD: &str = "maia: no password given";

#[derive(Default)]
struct Args {
    mode: Option<char>,
    verbose: bool,
    to_stdout: bool,
    file: Option<PathBuf>,
    dir: Option<PathBuf>,
    from_list: Option<PathBuf>,
    format: Option<String>,
    options: Vec<String>,
    passphrase: Option<String>,
    names: Vec<OsString>,
}

fn parse(raw: &[OsString]) -> Result<Args, String> {
    let mut a = Args::default();
    let mut it = raw.iter();
    let value = |it: &mut std::slice::Iter<OsString>, what: &str| it.next().cloned().ok_or(format!("Option {what} needs a value"));
    while let Some(x) = it.next() {
        let s = x.to_string_lossy();
        match s.as_ref() {
            "--" => {
                a.names.extend(it.by_ref().cloned());
                break;
            }
            "--format" => a.format = Some(value(&mut it, "--format")?.to_string_lossy().into_owned()),
            "--options" => a.options.extend(value(&mut it, "--options")?.to_string_lossy().split(',').map(String::from)),
            "--passphrase" => a.passphrase = Some(value(&mut it, "--passphrase")?.to_string_lossy().into_owned()),
            "--zstd" => {}
            _ if s.starts_with("--") => return Err(format!("Option {s} is not supported")),
            _ if s.starts_with('-') && s.len() > 1 => {
                for c in s.chars().skip(1) {
                    match c {
                        't' | 'x' | 'c' => a.mode = Some(c),
                        'v' => a.verbose = true,
                        'O' => a.to_stdout = true,
                        'f' => a.file = Some(PathBuf::from(value(&mut it, "-f")?)),
                        'C' => a.dir = Some(PathBuf::from(value(&mut it, "-C")?)),
                        'T' => a.from_list = Some(PathBuf::from(value(&mut it, "-T")?)),
                        'z' | 'J' | 'j' => {}
                        _ => return Err(format!("Option -{c} is not supported")),
                    }
                }
            }
            _ => a.names.push(x.clone()),
        }
    }
    Ok(a)
}

/// `mochi.exe --bsdtar <args>`: the exit code.
pub fn bsdtar(raw: Vec<OsString>) -> i32 {
    let a = match parse(&raw) {
        Ok(a) => a,
        Err(e) => return fail(&e),
    };
    let Some(file) = a.file.clone() else { return fail("No archive named (-f)") };
    let zip = match a.mode {
        Some('c') => a.format.as_deref() == Some("zip"),
        _ => is_zip(&file),
    };
    if !zip {
        return windows_tar(&raw);
    }
    let r = match a.mode {
        Some('t') => list(&file, a.verbose),
        Some('x') => extract(&file, &a),
        Some('c') => create(&file, &a),
        _ => Err("Say -t, -x or -c".into()),
    };
    match r {
        Ok(()) => 0,
        Err(e) => fail(&e),
    }
}

fn fail(e: &str) -> i32 {
    let _ = writeln!(std::io::stderr(), "bsdtar: {e}");
    1
}

/// A zip by its first bytes (a zip that starts with something else, as a self-extracting
/// one does, is found by its name).
fn is_zip(p: &Path) -> bool {
    let mut b = [0u8; 4];
    let magic = std::fs::File::open(p).and_then(|mut f| f.read_exact(&mut b)).is_ok() && (b == *b"PK\x03\x04" || b == *b"PK\x05\x06");
    let n = p.to_string_lossy().to_lowercase();
    magic || [".zip", ".jar", ".apk", ".cbz", ".xpi"].iter().any(|e| n.ends_with(e))
}

/// Anything not a zip: Windows' own tar.exe, with the same arguments and our input and
/// output, and no console window of its own.
fn windows_tar(raw: &[OsString]) -> i32 {
    use std::os::windows::process::CommandExt;
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| "C:\\Windows".into());
    let tar = root.join("System32").join("tar.exe");
    if !tar.is_file() {
        return fail("Only zip archives can be opened on this Windows (tar.exe is missing)");
    }
    // In a job that ends with this process: Cancel kills this one, and tar.exe with it.
    let _job = kill_with_me();
    match std::process::Command::new(tar).args(raw).creation_flags(0x0800_0000).status() {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => fail(&e.to_string()),
    }
}

/// Put this process in a job whose processes all end when its last handle closes, as it
/// does when this process ends however it ends. Kept open while the handle lives.
fn kill_with_me() -> Option<windows_sys::Win32::Foundation::HANDLE> {
    use windows_sys::Win32::System::JobObjects::{AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return None;
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let set = SetInformationJobObject(job, JobObjectExtendedLimitInformation, (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(), std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32) != 0;
        (set && AssignProcessToJobObject(job, GetCurrentProcess()) != 0).then_some(job)
    }
}

/// The password from the input, read once (bsdtar's way when --passphrase gives none).
fn password(a: &Args, asked: &mut Option<Option<String>>) -> Option<String> {
    if let Some(p) = &a.passphrase {
        return (p != NO_PASSWORD).then(|| p.clone());
    }
    asked
        .get_or_insert_with(|| {
            let mut line = String::new();
            std::io::stdin().lock().read_line(&mut line).ok()?;
            let p = line.trim_end_matches(['\r', '\n']).to_string();
            (!p.is_empty()).then_some(p)
        })
        .clone()
}

fn open(file: &Path) -> Result<ZipArchive<std::io::BufReader<std::fs::File>>, String> {
    let f = std::fs::File::open(file).map_err(|e| format!("{}: {e}", file.display()))?;
    ZipArchive::new(std::io::BufReader::new(f)).map_err(|e| format!("{}: {e}", file.display()))
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// `bsdtar -tv` lines: "-rw-r--r--  0 0 0 1234 Mar 05 14:02 docs/notes.txt", the time
/// as the hour in this year and the year before it (archive::parse_list reads it so).
fn list(file: &Path, verbose: bool) -> Result<(), String> {
    let mut z = open(file)?;
    let this_year = now_year();
    let out = std::io::stdout();
    let mut out = std::io::BufWriter::new(out.lock());
    for i in 0..z.len() {
        let f = z.by_index_raw(i).map_err(|e| e.to_string())?;
        let name = escaped(&f.name().map_err(|e| e.to_string())?);
        if !verbose {
            let _ = writeln!(out, "{name}");
            continue;
        }
        let perms = if f.is_dir() {
            "drwxr-xr-x"
        } else if f.is_symlink() {
            "lrwxrwxrwx"
        } else {
            "-rw-r--r--"
        };
        let (mon, day, when) = match f.last_modified() {
            Some(t) if (1..=12).contains(&t.month()) => {
                let when = if i32::from(t.year()) == this_year { format!("{:02}:{:02}", t.hour(), t.minute()) } else { t.year().to_string() };
                (MONTHS[t.month() as usize - 1], t.day(), when)
            }
            _ => ("Jan", 1, "1980".to_string()),
        };
        let _ = writeln!(out, "{perms}  0 0      0 {:>10} {mon} {day:>2} {when:>5} {name}", f.size());
    }
    out.flush().map_err(|e| e.to_string())
}

/// A name as bsdtar prints it: a backslash doubled, a control character as \ooo.
fn escaped(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 || c == '\x7f' => o.push_str(&format!("\\{:03o}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn now_year() -> i32 {
    let t = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let mut tm = crate::compat::time::tm::default();
    unsafe { crate::compat::time::localtime_r(&t, &mut tm) };
    tm.tm_year + 1900
}

/// A member's name compared as stored, without "./" and a closing slash.
fn plain(s: &str) -> &str {
    s.trim_start_matches("./").trim_end_matches('/')
}

/// Extract everything, or the members named (folders with all inside them), into -C, or
/// with -O one member's bytes to the output. -v says "x name" as each one starts.
fn extract(file: &Path, a: &Args) -> Result<(), String> {
    let mut z = open(file)?;
    let dir = a.dir.clone().unwrap_or_else(|| PathBuf::from("."));
    let wanted: Vec<String> = a.names.iter().map(|n| plain(&n.to_string_lossy().replace('\\', "/")).to_string()).collect();
    let mut asked = None;
    let mut failed: Option<String> = None;
    let mut found = vec![false; wanted.len()];
    for i in 0..z.len() {
        let (name, dir_entry, encrypted, link, when, safe) = {
            let f = z.by_index_raw(i).map_err(|e| e.to_string())?;
            let name = f.name().map_err(|e| e.to_string())?.into_owned();
            (name, f.is_dir(), f.encrypted(), f.is_symlink(), f.last_modified(), f.enclosed_name())
        };
        let n = plain(&name);
        if !wanted.is_empty() {
            let hit = wanted.iter().position(|w| n == w || n.starts_with(&format!("{w}/")));
            match hit {
                Some(k) => found[k] = true,
                None => continue,
            }
        }
        // A name that would land outside the folder ("../x", "C:\x") is left out.
        let Some(rel) = safe else { continue };
        if a.verbose {
            let _ = write!(std::io::stderr(), "x {n}");
        }
        let r = (|| -> Result<(), String> {
            if dir_entry {
                if !a.to_stdout {
                    std::fs::create_dir_all(dir.join(&rel)).map_err(|e| e.to_string())?;
                }
                return Ok(());
            }
            let pw = if encrypted { Some(password(a, &mut asked).ok_or("Passphrase required for this entry")?) } else { None };
            let mut f = match &pw {
                Some(p) => z.by_index_decrypt(i, p.as_bytes()).map_err(|e| match e {
                    zip::result::ZipError::InvalidPassword => "Incorrect passphrase".to_string(),
                    e => e.to_string(),
                })?,
                None => z.by_index(i).map_err(|e| e.to_string())?,
            };
            if a.to_stdout {
                let out = std::io::stdout();
                let mut out = out.lock();
                std::io::copy(&mut f, &mut out).map_err(|e| if pw.is_some() { "Incorrect passphrase".to_string() } else { e.to_string() })?;
                return out.flush().map_err(|e| e.to_string());
            }
            // Windows makes no links without admin rights: a link's target is left out.
            if link {
                return Ok(());
            }
            let dest = dir.join(&rel);
            if let Some(p) = dest.parent() {
                std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut out = std::io::BufWriter::new(std::fs::File::create(&dest).map_err(|e| format!("{}: {e}", dest.display()))?);
            // AES checks its password only at the end of the data: a wrong one fails here.
            std::io::copy(&mut f, &mut out).map_err(|e| if pw.is_some() { "Incorrect passphrase".to_string() } else { e.to_string() })?;
            let file = out.into_inner().map_err(|e| e.to_string())?;
            if let Some(t) = when.and_then(system_time) {
                let _ = file.set_modified(t);
            }
            Ok(())
        })();
        match r {
            Ok(()) => {
                if a.verbose {
                    let _ = writeln!(std::io::stderr());
                }
            }
            Err(e) => {
                // bsdtar's way: "x name: why", and on to the next one.
                if a.verbose {
                    let _ = writeln!(std::io::stderr(), ": {e}");
                } else {
                    let _ = writeln!(std::io::stderr(), "bsdtar: {n}: {e}");
                }
                failed.get_or_insert(e);
            }
        }
    }
    if let Some(k) = found.iter().position(|f| !f) {
        failed.get_or_insert(format!("{}: Not found in archive", wanted[k]));
    }
    match failed {
        Some(e) => Err(format!("Error exit delayed from previous errors ({e})")),
        None => Ok(()),
    }
}

/// A zip time (local, as zip keeps it) as a moment.
fn system_time(t: DateTime) -> Option<SystemTime> {
    let mut tm = crate::compat::time::tm { tm_year: i32::from(t.year()) - 1900, tm_mon: i32::from(t.month()) - 1, tm_mday: i32::from(t.day()), tm_hour: i32::from(t.hour()), tm_min: i32::from(t.minute()), tm_sec: i32::from(t.second()), tm_isdst: -1, ..Default::default() };
    let s = unsafe { crate::compat::time::mktime(&mut tm) };
    (s >= 0).then(|| SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(s as u64))
}

/// A moment as a zip time, in local time (zips before 1980 can't say).
fn zip_time(t: SystemTime) -> DateTime {
    let s = t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let mut tm = crate::compat::time::tm::default();
    unsafe { crate::compat::time::localtime_r(&s, &mut tm) };
    DateTime::from_date_and_time((tm.tm_year + 1900).clamp(1980, 2107) as u16, (tm.tm_mon + 1) as u8, tm.tm_mday as u8, tm.tm_hour as u8, tm.tm_min as u8, tm.tm_sec as u8).unwrap_or_default()
}

/// Pack the names (inside -C) into a zip, folders with all inside them. -v says
/// "a name" as each one starts. `zip:compression-level=N` and
/// `zip:encryption=aes256|zipcrypt` from --options (both encryptions are written as
/// AES-256, which every current tool opens).
fn create(file: &Path, a: &Args) -> Result<(), String> {
    let base = a.dir.clone().unwrap_or_else(|| PathBuf::from("."));
    let mut level: Option<i64> = None;
    let mut encrypt = false;
    for o in &a.options {
        let o = o.trim().trim_start_matches("zip:");
        if let Some(n) = o.strip_prefix("compression-level=") {
            level = n.parse().ok();
        } else if o.starts_with("encryption") {
            encrypt = true;
        }
    }
    let pw = if encrypt {
        let mut asked = None;
        Some(password(a, &mut asked).ok_or("Passphrase required")?)
    } else {
        None
    };
    // -T: names from a file (archwrite passes /dev/null for an empty archive).
    let mut names = a.names.clone();
    if let Some(l) = &a.from_list
        && l != Path::new("/dev/null")
    {
        names.extend(std::fs::read_to_string(l).unwrap_or_default().lines().filter(|l| !l.is_empty()).map(OsString::from));
    }
    let out = std::fs::File::create(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut w = ZipWriter::new(std::io::BufWriter::new(out));
    let opts = |t: SystemTime, size: u64| {
        let o = SimpleFileOptions::default()
            .compression_method(if level == Some(0) { CompressionMethod::Stored } else { CompressionMethod::Deflated })
            .compression_level(level.filter(|&n| n > 0))
            .last_modified_time(zip_time(t))
            .large_file(size >= 0xFFFF_FFFF);
        match &pw {
            Some(p) => o.with_aes_encryption(AesMode::Aes256, p),
            None => o,
        }
    };
    for n in &names {
        add(&mut w, &base, Path::new(n), &opts, a.verbose)?;
    }
    w.finish().map_err(|e| e.to_string())?.flush().map_err(|e| e.to_string())
}

fn add<'k, W: Write + std::io::Seek>(w: &mut ZipWriter<W>, base: &Path, rel: &Path, opts: &dyn Fn(SystemTime, u64) -> zip::write::FileOptions<'k, 'static, ()>, verbose: bool) -> Result<(), String> {
    let full = base.join(rel);
    let m = std::fs::metadata(&full).map_err(|e| format!("{}: {e}", rel.display()))?;
    let name: String = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
    if verbose {
        let _ = writeln!(std::io::stderr(), "a {name}");
    }
    let t = m.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    if m.is_dir() {
        w.add_directory(format!("{name}/"), opts(t, 0)).map_err(|e| e.to_string())?;
        let mut kids: Vec<OsString> = std::fs::read_dir(&full).map_err(|e| format!("{}: {e}", rel.display()))?.flatten().map(|e| e.file_name()).collect();
        kids.sort();
        for k in kids {
            add(w, base, &rel.join(OsStr::new(&k)), opts, verbose)?;
        }
        return Ok(());
    }
    w.start_file(name, opts(t, m.len())).map_err(|e| e.to_string())?;
    let mut f = std::fs::File::open(&full).map_err(|e| format!("{}: {e}", rel.display()))?;
    std::io::copy(&mut f, w).map_err(|e| format!("{}: {e}", rel.display()))?;
    Ok(())
}

/// `mochi.exe --bsdcat -- file.gz`: the one file inside, to the output.
pub fn bsdcat(raw: Vec<OsString>) -> i32 {
    let Some(p) = raw.iter().rev().find(|a| a.as_os_str() != "--") else { return fail("No file named") };
    if !p.to_string_lossy().to_lowercase().ends_with(".gz") {
        return fail("Only .gz files can be opened on Windows");
    }
    let r = std::fs::File::open(p).and_then(|f| {
        let mut d = flate2::read::MultiGzDecoder::new(std::io::BufReader::new(f));
        let out = std::io::stdout();
        let mut out = out.lock();
        std::io::copy(&mut d, &mut out)?;
        out.flush()
    });
    match r {
        Ok(()) => 0,
        Err(e) => fail(&e.to_string()),
    }
}
