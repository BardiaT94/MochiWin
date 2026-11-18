//! The `std::os::unix` extension traits the code base calls, under one path.
//!
//! On Unix this is a re-export of the standard library. On Windows the same names exist
//! with stand-in behaviour: Unix modes become read-only flags, owners and inodes read as 0,
//! and process hooks such as `pre_exec` do nothing.

#[cfg(unix)]
pub use std::os::unix::{ffi, fs, process};

#[cfg(windows)]
pub mod fs {
    use std::io;
    use std::path::Path;

    pub trait PermissionsExt {
        fn mode(&self) -> u32;
        fn set_mode(&mut self, mode: u32);
        fn from_mode(mode: u32) -> Self;
    }

    impl PermissionsExt for std::fs::Permissions {
        fn mode(&self) -> u32 {
            if self.readonly() { 0o100444 } else { 0o100644 }
        }
        fn set_mode(&mut self, mode: u32) {
            self.set_readonly(mode & 0o222 == 0);
        }
        fn from_mode(mode: u32) -> Self {
            // Permissions has no public constructor on Windows; read one off the temp dir.
            let mut p = std::fs::metadata(std::env::temp_dir()).map(|m| m.permissions()).expect("temp dir metadata");
            p.set_readonly(mode & 0o222 == 0);
            p
        }
    }

    pub trait MetadataExt {
        fn dev(&self) -> u64;
        fn ino(&self) -> u64;
        fn mode(&self) -> u32;
        fn nlink(&self) -> u64;
        fn uid(&self) -> u32;
        fn gid(&self) -> u32;
        fn rdev(&self) -> u64;
        fn size(&self) -> u64;
        fn atime(&self) -> i64;
        fn atime_nsec(&self) -> i64;
        fn mtime(&self) -> i64;
        fn mtime_nsec(&self) -> i64;
        fn ctime(&self) -> i64;
        fn ctime_nsec(&self) -> i64;
        fn blksize(&self) -> u64;
        fn blocks(&self) -> u64;
    }

    fn secs(t: io::Result<std::time::SystemTime>) -> (i64, i64) {
        match t.ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()) {
            Some(d) => (d.as_secs() as i64, d.subsec_nanos() as i64),
            None => (0, 0),
        }
    }

    impl MetadataExt for std::fs::Metadata {
        fn dev(&self) -> u64 { 0 }
        fn ino(&self) -> u64 { 0 }
        fn mode(&self) -> u32 {
            let kind = if self.is_dir() { 0o040000 } else if self.is_symlink() { 0o120000 } else { 0o100000 };
            let perm = if self.permissions().readonly() { 0o444 } else { 0o644 };
            kind | perm | if self.is_dir() { 0o111 } else { 0 }
        }
        fn nlink(&self) -> u64 { 1 }
        fn uid(&self) -> u32 { 0 }
        fn gid(&self) -> u32 { 0 }
        fn rdev(&self) -> u64 { 0 }
        fn size(&self) -> u64 { self.len() }
        fn atime(&self) -> i64 { secs(self.accessed()).0 }
        fn atime_nsec(&self) -> i64 { secs(self.accessed()).1 }
        fn mtime(&self) -> i64 { secs(self.modified()).0 }
        fn mtime_nsec(&self) -> i64 { secs(self.modified()).1 }
        fn ctime(&self) -> i64 { secs(self.created()).0 }
        fn ctime_nsec(&self) -> i64 { secs(self.created()).1 }
        fn blksize(&self) -> u64 { 4096 }
        fn blocks(&self) -> u64 { self.len().div_ceil(512) }
    }

    pub trait FileTypeExt {
        fn is_block_device(&self) -> bool { false }
        fn is_char_device(&self) -> bool { false }
        fn is_fifo(&self) -> bool { false }
        fn is_socket(&self) -> bool { false }
    }
    impl FileTypeExt for std::fs::FileType {}

    pub trait OpenOptionsExt {
        fn mode(&mut self, mode: u32) -> &mut Self;
        fn custom_flags(&mut self, flags: i32) -> &mut Self;
    }
    impl OpenOptionsExt for std::fs::OpenOptions {
        fn mode(&mut self, _: u32) -> &mut Self { self }
        fn custom_flags(&mut self, _: i32) -> &mut Self { self }
    }

    pub trait DirBuilderExt {
        fn mode(&mut self, mode: u32) -> &mut Self;
    }
    impl DirBuilderExt for std::fs::DirBuilder {
        fn mode(&mut self, _: u32) -> &mut Self { self }
    }

    pub trait FileExt {
        fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize>;
        fn write_at(&self, buf: &[u8], offset: u64) -> io::Result<usize>;
        fn read_exact_at(&self, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
            while !buf.is_empty() {
                match self.read_at(buf, offset)? {
                    0 => return Err(io::ErrorKind::UnexpectedEof.into()),
                    n => {
                        buf = &mut buf[n..];
                        offset += n as u64;
                    }
                }
            }
            Ok(())
        }
        fn write_all_at(&self, mut buf: &[u8], mut offset: u64) -> io::Result<()> {
            while !buf.is_empty() {
                match self.write_at(buf, offset)? {
                    0 => return Err(io::ErrorKind::WriteZero.into()),
                    n => {
                        buf = &buf[n..];
                        offset += n as u64;
                    }
                }
            }
            Ok(())
        }
    }
    impl FileExt for std::fs::File {
        fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
            std::os::windows::fs::FileExt::seek_read(self, buf, offset)
        }
        fn write_at(&self, buf: &[u8], offset: u64) -> io::Result<usize> {
            std::os::windows::fs::FileExt::seek_write(self, buf, offset)
        }
    }

    /// A Unix symlink: a directory link when the target is a directory.
    pub fn symlink(original: impl AsRef<Path>, link: impl AsRef<Path>) -> io::Result<()> {
        let (original, link) = (original.as_ref(), link.as_ref());
        let target = link.parent().map(|d| d.join(original)).unwrap_or_else(|| original.to_path_buf());
        if target.is_dir() {
            std::os::windows::fs::symlink_dir(original, link)
        } else {
            std::os::windows::fs::symlink_file(original, link)
        }
    }

    pub fn chown(_: impl AsRef<Path>, _: Option<u32>, _: Option<u32>) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(windows)]
pub mod ffi {
    use std::ffi::{OsStr, OsString};

    pub trait OsStrExt {
        fn from_bytes(slice: &[u8]) -> &Self;
        fn as_bytes(&self) -> &[u8];
    }
    impl OsStrExt for OsStr {
        fn from_bytes(slice: &[u8]) -> &OsStr {
            match std::str::from_utf8(slice) {
                Ok(s) => OsStr::new(s),
                Err(_) => OsStr::new("\u{FFFD}"),
            }
        }
        fn as_bytes(&self) -> &[u8] {
            self.as_encoded_bytes()
        }
    }

    pub trait OsStringExt {
        fn from_vec(vec: Vec<u8>) -> Self;
        fn into_vec(self) -> Vec<u8>;
    }
    impl OsStringExt for OsString {
        fn from_vec(vec: Vec<u8>) -> OsString {
            String::from_utf8_lossy(&vec).into_owned().into()
        }
        fn into_vec(self) -> Vec<u8> {
            self.into_encoded_bytes()
        }
    }
}

#[cfg(windows)]
pub mod process {
    use std::io;
    use std::process::Command;

    pub trait CommandExt {
        fn process_group(&mut self, pgroup: i32) -> &mut Command;
        /// Runs nothing on Windows: there is no fork to hook into.
        unsafe fn pre_exec<F>(&mut self, f: F) -> &mut Command
        where
            F: FnMut() -> io::Result<()> + Send + Sync + 'static;
        fn exec(&mut self) -> io::Error;
        fn uid(&mut self, id: u32) -> &mut Command;
        fn gid(&mut self, id: u32) -> &mut Command;
        fn arg0<S: AsRef<std::ffi::OsStr>>(&mut self, arg: S) -> &mut Command;
    }
    impl CommandExt for Command {
        fn process_group(&mut self, _: i32) -> &mut Command { self }
        unsafe fn pre_exec<F>(&mut self, _: F) -> &mut Command
        where
            F: FnMut() -> io::Result<()> + Send + Sync + 'static,
        {
            self
        }
        fn exec(&mut self) -> io::Error {
            match self.status() {
                Ok(s) => std::process::exit(s.code().unwrap_or(1)),
                Err(e) => e,
            }
        }
        fn uid(&mut self, _: u32) -> &mut Command { self }
        fn gid(&mut self, _: u32) -> &mut Command { self }
        fn arg0<S: AsRef<std::ffi::OsStr>>(&mut self, _: S) -> &mut Command { self }
    }
}

/// Process and user calls made through libc on Unix.
#[cfg(unix)]
pub mod sys {
    pub use libc::{access, getuid, kill, major, memmem, minor, pid_t, O_NONBLOCK, SIGCONT, SIGHUP, SIGKILL, SIGSTOP, SIGTERM, W_OK};
}

#[cfg(windows)]
#[allow(non_camel_case_types)]
pub mod sys {
    pub use libc::access;

    pub type pid_t = i32;
    pub const W_OK: i32 = 2;
    /// No such open flag on Windows: opening never blocks on a FIFO there.
    pub const O_NONBLOCK: i32 = 0;
    pub const SIGHUP: i32 = 1;
    pub const SIGKILL: i32 = 9;
    pub const SIGTERM: i32 = 15;
    pub const SIGCONT: i32 = 18;
    pub const SIGSTOP: i32 = 19;

    /// Device numbers do not exist on Windows.
    pub const fn major(_dev: u64) -> u32 {
        0
    }
    pub const fn minor(_dev: u64) -> u32 {
        0
    }

    /// C's memmem: the first `needle` in `hay`, or null.
    pub unsafe fn memmem(hay: *const libc::c_void, hay_len: usize, needle: *const libc::c_void, needle_len: usize) -> *mut libc::c_void {
        let (h, n) = unsafe { (std::slice::from_raw_parts(hay.cast::<u8>(), hay_len), std::slice::from_raw_parts(needle.cast::<u8>(), needle_len)) };
        if n.is_empty() {
            return hay.cast_mut();
        }
        if n.len() > h.len() {
            return std::ptr::null_mut();
        }
        let mut i = 0;
        while i + n.len() <= h.len() {
            match h[i..=h.len() - n.len()].iter().position(|&b| b == n[0]) {
                None => break,
                Some(p) => {
                    i += p;
                    if &h[i..i + n.len()] == n {
                        return unsafe { hay.cast::<u8>().add(i) }.cast_mut().cast();
                    }
                    i += 1;
                }
            }
        }
        std::ptr::null_mut()
    }

    /// Every user is uid 0 on Windows: this keeps per-user names (temp dirs, sockets) stable.
    pub unsafe fn getuid() -> u32 {
        0
    }

    /// Ends the process for SIGKILL, SIGTERM and SIGHUP (a negative pid, a Unix process
    /// group, ends that one process). Pausing and resuming (SIGSTOP/SIGCONT) are not
    /// supported and fail; signal 0 only checks that the process exists.
    pub unsafe fn kill(pid: pid_t, sig: i32) -> i32 {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, TerminateProcess};
        let pid = pid.unsigned_abs();
        let access = if sig == 0 { PROCESS_QUERY_LIMITED_INFORMATION } else { PROCESS_TERMINATE };
        if !matches!(sig, 0 | SIGHUP | SIGKILL | SIGTERM) {
            return -1;
        }
        unsafe {
            let h = OpenProcess(access, 0, pid);
            if h.is_null() {
                return -1;
            }
            let ok = sig == 0 || TerminateProcess(h, 1) != 0;
            CloseHandle(h);
            if ok { 0 } else { -1 }
        }
    }
}

/// Local time as the C library gives it on Unix.
#[cfg(unix)]
pub mod time {
    pub use libc::{localtime_r, mktime, strftime, time_t, tm};
}

/// Windows: the same calls in Rust. The CRT's localtime_s does the time zone work;
/// strftime covers the conversions the app uses, with English names.
#[cfg(windows)]
#[allow(non_camel_case_types)]
pub mod time {
    pub type time_t = i64;

    #[derive(Clone, Copy, Default, Debug)]
    pub struct tm {
        pub tm_sec: i32,
        pub tm_min: i32,
        pub tm_hour: i32,
        pub tm_mday: i32,
        pub tm_mon: i32,
        pub tm_year: i32,
        pub tm_wday: i32,
        pub tm_yday: i32,
        pub tm_isdst: i32,
        /// Seconds east of UTC.
        pub tm_gmtoff: i64,
    }

    /// Days since 1970-01-01 of a proleptic Gregorian date (month 1..=12).
    fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    fn civil_from_days(z: i64) -> (i64, i64, i64) {
        let z = z + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        (yoe + era * 400 + i64::from(m <= 2), m, d)
    }

    /// The broken-down fields of `t` shifted by `off` seconds.
    fn fields(t: i64, off: i64) -> tm {
        let s = t + off;
        let (days, sod) = (s.div_euclid(86_400), s.rem_euclid(86_400));
        let (y, m, d) = civil_from_days(days);
        tm {
            tm_sec: (sod % 60) as i32,
            tm_min: (sod / 60 % 60) as i32,
            tm_hour: (sod / 3600) as i32,
            tm_mday: d as i32,
            tm_mon: (m - 1) as i32,
            tm_year: (y - 1900) as i32,
            tm_wday: (days + 4).rem_euclid(7) as i32,
            tm_yday: (days - days_from_civil(y, 1, 1)) as i32,
            tm_isdst: 0,
            tm_gmtoff: off,
        }
    }

    /// Seconds east of UTC at instant `t`, from the CRT's local time.
    fn offset(t: i64) -> i64 {
        let mut c: libc::tm = unsafe { std::mem::zeroed() };
        if t < 0 || unsafe { libc::localtime_s(&mut c, &t) } != 0 {
            return 0;
        }
        let local = days_from_civil(c.tm_year as i64 + 1900, c.tm_mon as i64 + 1, c.tm_mday as i64) * 86_400 + c.tm_hour as i64 * 3600 + c.tm_min as i64 * 60 + c.tm_sec as i64;
        local - t
    }

    pub unsafe fn localtime_r(t: *const time_t, out: *mut tm) -> *mut tm {
        let t = unsafe { *t };
        let mut r = fields(t, offset(t));
        let mut c: libc::tm = unsafe { std::mem::zeroed() };
        if t >= 0 && unsafe { libc::localtime_s(&mut c, &t) } == 0 {
            r.tm_isdst = c.tm_isdst;
        }
        unsafe { *out = r };
        out
    }

    /// Local fields to an instant; normalises `tm` the way C's mktime does.
    pub unsafe fn mktime(tm: *mut tm) -> time_t {
        let f = unsafe { &mut *tm };
        let (y, m) = (f.tm_year as i64 + 1900 + (f.tm_mon as i64).div_euclid(12), (f.tm_mon as i64).rem_euclid(12) + 1);
        let as_utc = days_from_civil(y, m, 1) * 86_400 + (f.tm_mday as i64 - 1) * 86_400 + f.tm_hour as i64 * 3600 + f.tm_min as i64 * 60 + f.tm_sec as i64;
        let mut t = as_utc - offset(as_utc);
        t = as_utc - offset(t);
        unsafe { localtime_r(&t, tm) };
        t
    }

    const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

    /// `%a %A %b %B %d %e %H %I %j %m %M %p %S %y %Y %z %Z %%`, and `%-d` style
    /// unpadded numbers.
    pub fn format(f: &str, t: &tm) -> String {
        let mut out = String::new();
        let mut it = f.chars();
        while let Some(c) = it.next() {
            if c != '%' {
                out.push(c);
                continue;
            }
            let mut spec = it.next();
            let pad = spec != Some('-');
            if !pad {
                spec = it.next();
            }
            let num = |out: &mut String, v: i32, w: usize| {
                if pad { out.push_str(&format!("{v:0w$}")) } else { out.push_str(&v.to_string()) }
            };
            let wday = DAYS[t.tm_wday.rem_euclid(7) as usize];
            let month = MONTHS[t.tm_mon.rem_euclid(12) as usize];
            match spec {
                Some('a') => out.push_str(&wday[..3]),
                Some('A') => out.push_str(wday),
                Some('b' | 'h') => out.push_str(&month[..3]),
                Some('B') => out.push_str(month),
                Some('d') => num(&mut out, t.tm_mday, 2),
                Some('e') => out.push_str(&format!("{:>2}", t.tm_mday)),
                Some('H') => num(&mut out, t.tm_hour, 2),
                Some('I') => num(&mut out, (t.tm_hour + 11) % 12 + 1, 2),
                Some('j') => num(&mut out, t.tm_yday + 1, 3),
                Some('m') => num(&mut out, t.tm_mon + 1, 2),
                Some('M') => num(&mut out, t.tm_min, 2),
                Some('p') => out.push_str(if t.tm_hour < 12 { "AM" } else { "PM" }),
                Some('S') => num(&mut out, t.tm_sec, 2),
                Some('y') => num(&mut out, (t.tm_year + 1900) % 100, 2),
                Some('Y') => out.push_str(&(t.tm_year + 1900).to_string()),
                Some('z') => {
                    let o = t.tm_gmtoff / 60;
                    out.push_str(&format!("{}{:02}{:02}", if o < 0 { '-' } else { '+' }, o.abs() / 60, o.abs() % 60));
                }
                Some('Z') => {}
                Some('%') => out.push('%'),
                Some(o) => {
                    out.push('%');
                    out.push(o);
                }
                None => out.push('%'),
            }
        }
        out
    }

    /// C's strftime over `format`: writes at most `max` bytes, 0 when it does not fit.
    pub unsafe fn strftime(buf: *mut libc::c_char, max: usize, f: *const libc::c_char, t: *const tm) -> usize {
        let f = unsafe { std::ffi::CStr::from_ptr(f) }.to_string_lossy();
        let s = format(&f, unsafe { &*t });
        if s.len() >= max {
            return 0;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(s.as_ptr(), buf.cast(), s.len());
            *buf.add(s.len()) = 0;
        }
        s.len()
    }
}

/// Unix domain sockets (the D-Bus connection).
#[cfg(unix)]
pub mod net {
    pub use std::os::unix::net::UnixStream;
}

/// Windows: there is no session bus to connect to, so a connection never opens.
#[cfg(windows)]
pub mod net {
    use std::io;
    use std::path::Path;
    use std::time::Duration;

    pub enum UnixStream {}

    impl UnixStream {
        pub fn connect(_path: impl AsRef<Path>) -> io::Result<UnixStream> {
            Err(io::ErrorKind::Unsupported.into())
        }
        pub fn try_clone(&self) -> io::Result<UnixStream> {
            match *self {}
        }
        pub fn set_read_timeout(&self, _t: Option<Duration>) -> io::Result<()> {
            match *self {}
        }
        pub fn set_write_timeout(&self, _t: Option<Duration>) -> io::Result<()> {
            match *self {}
        }
        pub fn set_nonblocking(&self, _on: bool) -> io::Result<()> {
            match *self {}
        }
        pub fn shutdown(&self, _how: std::net::Shutdown) -> io::Result<()> {
            match *self {}
        }
    }

    impl io::Read for UnixStream {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            match *self {}
        }
    }

    impl io::Write for UnixStream {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            match *self {}
        }
        fn flush(&mut self) -> io::Result<()> {
            match *self {}
        }
    }

    impl io::Read for &UnixStream {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            match **self {}
        }
    }

    impl io::Write for &UnixStream {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            match **self {}
        }
        fn flush(&mut self) -> io::Result<()> {
            match **self {}
        }
    }
}

/// Windows calls with no Unix counterpart in this module's other sections.
#[cfg(windows)]
pub mod win {
    use std::os::windows::ffi::OsStrExt as _;
    use std::path::Path;

    /// A path as a nul-ended UTF-16 string.
    pub fn wide(p: &Path) -> Vec<u16> {
        p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
    }

    /// The drive letter `d` (b'C') is a network share.
    pub fn drive_is_remote(d: u8) -> bool {
        use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
        const DRIVE_REMOTE: u32 = 4;
        let root = [d as u16, b':' as u16, b'\\' as u16, 0];
        unsafe { GetDriveTypeW(root.as_ptr()) == DRIVE_REMOTE }
    }

    /// (total, free to this user) bytes on the drive holding `p`.
    pub fn disk_space(p: &Path) -> Option<(u64, u64)> {
        use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
        let (mut free, mut total) = (0u64, 0u64);
        let ok = unsafe { GetDiskFreeSpaceExW(wide(p).as_ptr(), &mut free, &mut total, std::ptr::null_mut()) };
        (ok != 0).then_some((total, free))
    }

    /// A file's identity: its volume's serial number and its index there. None when it
    /// can't be opened.
    pub fn file_id(p: &Path) -> Option<(u64, u64)> {
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileInformationByHandle, OPEN_EXISTING};
        // No access asked for: only its attributes are read. Backup semantics open folders too.
        let h = unsafe { CreateFileW(wide(p).as_ptr(), 0, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, std::ptr::null(), OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS, std::ptr::null_mut()) };
        if h == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        let ok = unsafe { GetFileInformationByHandle(h, &mut info) } != 0;
        unsafe { CloseHandle(h) };
        ok.then(|| (info.dwVolumeSerialNumber as u64, (info.nFileIndexHigh as u64) << 32 | info.nFileIndexLow as u64))
    }

    /// The volume's own name ("Data"), empty when it has none; nothing for an empty drive.
    pub fn volume_name(root: &Path) -> Option<String> {
        use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;
        let mut buf = [0u16; 261];
        let ok = unsafe { GetVolumeInformationW(wide(root).as_ptr(), buf.as_mut_ptr(), buf.len() as u32, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), 0) };
        let n = buf.iter().position(|&c| c == 0).unwrap_or(0);
        (ok != 0).then(|| String::from_utf16_lossy(&buf[..n]))
    }

    /// Drive letters in use and their roots (`C:\\`).
    pub fn drive_roots() -> Vec<(char, std::path::PathBuf)> {
        let mask = unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() };
        (0..26u8).filter(|i| mask & (1 << i) != 0).map(|i| ((b'A' + i) as char, std::path::PathBuf::from(format!("{}:\\", (b'A' + i) as char)))).collect()
    }

    /// Open `what` (a file, folder or URL) the way Explorer would: its type's default app.
    pub fn shell_open(what: &std::ffi::OsStr, dir: Option<&Path>) -> Result<(), String> {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        const SW_SHOWNORMAL: i32 = 1;
        let w: Vec<u16> = what.encode_wide().chain(std::iter::once(0)).collect();
        let d = dir.map(wide);
        let verb: Vec<u16> = "open\0".encode_utf16().collect();
        let r = unsafe { ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), w.as_ptr(), std::ptr::null(), d.as_ref().map_or(std::ptr::null(), |d| d.as_ptr()), SW_SHOWNORMAL) };
        // Values above 32 mean it started.
        if r as isize > 32 { Ok(()) } else { Err(std::io::Error::last_os_error().to_string()) }
    }

    /// Mesa's software OpenGL ships in `software-gl\` beside the app, with its own copy
    /// of the app: Windows loads opengl32.dll from the program's folder first, and once
    /// a process has loaded the system's it cannot switch. Starts that copy with the same
    /// arguments and exits; returns when there is none, or this already is it.
    pub fn relaunch_software_gl() {
        const ENV: &str = "MOCHI_SOFTWARE_GL";
        if std::env::var_os(ENV).is_some() {
            return;
        }
        let Some(exe) = std::env::current_exe().ok() else { return };
        let Some(alt) = exe.parent().map(|d| d.join("software-gl").join(exe.file_name().unwrap_or_default())) else { return };
        if !alt.is_file() {
            return;
        }
        let started = std::process::Command::new(&alt).args(std::env::args_os().skip(1)).env(ENV, "1").env("GALLIUM_DRIVER", "llvmpipe").spawn();
        if started.is_ok() {
            std::process::exit(0);
        }
    }

    /// A blocking error box, for failures before there is a window to show them in.
    pub fn message_box(title: &str, body: &str) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
        let w = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        let (t, b) = (w(title), w(body));
        unsafe { MessageBoxW(std::ptr::null_mut(), b.as_ptr(), t.as_ptr(), MB_OK | MB_ICONERROR) };
    }

    /// The font file for a family name, from the fonts Windows lists in the registry (the
    /// machine's, then this user's). Generic names map to Windows' own: `sans-serif` and
    /// `system-ui` to Segoe UI, `monospace` to Cascadia Mono or Consolas. A fontconfig-style
    /// `Family:weight=semibold` asks for the family's semibold face.
    pub fn font_file(family: &str) -> Option<std::path::PathBuf> {
        let (fam, semibold) = match family.split_once(':') {
            Some((f, style)) => (f.trim(), style.contains("semibold")),
            None => (family.trim(), false),
        };
        let wanted: Vec<String> = match fam.to_ascii_lowercase().as_str() {
            "sans-serif" | "system-ui" | "sans" => vec!["Segoe UI".into()],
            "monospace" | "mono" => vec!["Cascadia Mono".into(), "Consolas".into()],
            _ => vec![fam.to_string()],
        };
        let fonts = registry_fonts();
        let dir = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("Fonts");
        for w in wanted {
            let names: Vec<String> = if semibold { vec![format!("{w} Semibold")] } else { vec![w.clone(), format!("{w} Regular")] };
            for n in names {
                let hit = fonts.iter().find(|(faces, _)| faces.iter().any(|f| f.eq_ignore_ascii_case(&n)));
                if let Some((_, file)) = hit {
                    let p = std::path::PathBuf::from(file);
                    return Some(if p.is_absolute() { p } else { dir.join(p) });
                }
            }
        }
        None
    }

    /// Settings > Accessibility > Text size, as a factor (1.0 to 2.25).
    pub fn text_scale() -> f32 {
        use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
        let key: Vec<u16> = "Software\\Microsoft\\Accessibility\0".encode_utf16().collect();
        let val: Vec<u16> = "TextScaleFactor\0".encode_utf16().collect();
        let (mut v, mut n) = (100u32, 4u32);
        let ok = unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), val.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut v as *mut u32).cast(), &mut n) } == 0;
        if ok { (v.clamp(100, 225) as f32) / 100.0 } else { 1.0 }
    }

    /// The registry's font list: the faces each value names ("Segoe UI (TrueType)",
    /// "MS Gothic & MS UI Gothic (TrueType)") and its file.
    fn registry_fonts() -> Vec<(Vec<String>, String)> {
        use windows_sys::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ, RegCloseKey, RegEnumValueW, RegOpenKeyExW};
        let sub: Vec<u16> = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts\0".encode_utf16().collect();
        let mut out = vec![];
        for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            let mut key: HKEY = std::ptr::null_mut();
            if unsafe { RegOpenKeyExW(root, sub.as_ptr(), 0, KEY_READ, &mut key) } != 0 {
                continue;
            }
            for i in 0.. {
                let (mut name, mut data) = (vec![0u16; 512], vec![0u8; 2048]);
                let (mut nlen, mut dlen, mut ty) = (name.len() as u32, data.len() as u32, 0u32);
                let r = unsafe { RegEnumValueW(key, i, name.as_mut_ptr(), &mut nlen, std::ptr::null_mut(), &mut ty, data.as_mut_ptr(), &mut dlen) };
                if r != 0 {
                    break;
                }
                if ty != REG_SZ {
                    continue;
                }
                let name = String::from_utf16_lossy(&name[..nlen as usize]);
                let wide: Vec<u16> = data[..dlen as usize].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&c| c != 0).collect();
                let file = String::from_utf16_lossy(&wide);
                let faces = name.rsplit_once(" (").map_or(name.as_str(), |(f, _)| f).split(" & ").map(|f| f.trim().to_string()).collect();
                out.push((faces, file));
            }
            unsafe { RegCloseKey(key) };
        }
        out
    }

    /// Fills `b` from the system's secure random source.
    pub fn random(b: &mut [u8]) -> bool {
        use windows_sys::Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom};
        unsafe { BCryptGenRandom(std::ptr::null_mut(), b.as_mut_ptr(), b.len() as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) == 0 }
    }

    /// Rename only when nothing is at `to`: MoveFileExW without REPLACE_EXISTING checks
    /// and moves in one step (across drives it copies, then deletes).
    pub fn rename_new(from: &Path, to: &Path) -> std::io::Result<()> {
        use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_COPY_ALLOWED, MoveFileExW};
        if unsafe { MoveFileExW(wide(from).as_ptr(), wide(to).as_ptr(), MOVEFILE_COPY_ALLOWED) } != 0 {
            return Ok(());
        }
        let e = std::io::Error::last_os_error();
        // ERROR_FILE_EXISTS, ERROR_ALREADY_EXISTS
        if matches!(e.raw_os_error(), Some(80 | 183)) {
            return Err(std::io::ErrorKind::AlreadyExists.into());
        }
        Err(e)
    }
}
