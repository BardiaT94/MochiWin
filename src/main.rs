//! Maia: a fast, keyboard-first file manager for Linux.
// Windows: a windowed program, with no console window of its own.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod admin;
mod anim;
mod app;
mod appearance;
mod calendar;
mod clipfile;
mod checksum;
mod codesearch;
mod archive;
mod archwrite;
mod blame;
mod books;
mod avatars;
mod batchname;
mod bigfile;
mod findbar;
mod quickedit;
mod buildclean;
mod update;
mod versions;
mod viewer;
mod wallpaper;
mod clipboard;
mod compat;
mod colours;
mod columns;
mod commands;
mod compare;
mod contentsearch;
mod datatree;
mod envfile;
mod errands;
mod exif;
mod facts;
mod extpic;
mod config;
mod copyto;
mod convert;
mod devtools;
mod dither;
mod glow;
mod drives;
mod dupes;
mod editor;
mod ctxmenu;
mod filediff;
mod fontview;
mod formatdrive;
mod freeup;
mod frost;
mod fsx;
mod fuzzy;
mod gitchanges;
mod gitcompare;
mod gitfind;
mod github;
mod gitlog;
mod gitstatus;
mod glance;
mod glide;
mod gitsync;
mod gitwork;
mod holders;
mod icons;
mod imgdec;
mod inspector;
mod install;
mod jump;
mod kit;
mod linecount;
mod listing;
mod markdown;
mod media;
mod memory;
mod mime;
mod musictags;
mod namefind;
mod needs;
mod network;
mod office;
mod ollama;
mod ops;
mod origin;
mod panel;
mod pastes;
mod peek;
mod perms;
mod phone;
mod picfix;
mod picker;
mod popups;
mod portal;
mod project;
mod projects;
mod fm1;
mod quickopen;
// Mochi: the README card is not drawn.
#[allow(dead_code)]
mod readme;
mod redo;
mod undofile;
mod undohist;
mod grew;
mod hints;
mod marks;
mod mochi;
mod graphics;
mod qr;
mod query;
mod ribbon;
mod rename;
mod runner;
mod selmatch;
mod scrub;
mod semantic;
mod sendto;
mod share;
mod shelf;
mod sidebar;
mod sidediff;
mod smart;
mod signals;
mod sqlite;
mod single;
mod spaceuse;
mod speedline;
mod stack;
mod solar_map;
mod symbols;
mod svg;
mod syntax;
mod tags;
mod textsel;
mod tasks;
mod terminal;
mod termlink;
mod stats;
mod tab;
mod table;
mod texmath;
#[cfg_attr(windows, path = "win/tail.rs")]
mod tail;
#[cfg(any(test, feature = "testkit"))]
mod testkit;
#[cfg(test)]
mod sim;
mod timefmt;
mod trashcan;
mod theme;
mod tidy;
mod transfers;
mod tour;
mod thumbcache;
mod thumbs;
mod ui;
mod vt;
mod wash;
#[cfg_attr(windows, path = "win/watch.rs")]
mod watch;
#[cfg_attr(not(target_os = "linux"), path = "win/wlclip.rs")]
mod wlclip;
#[cfg(target_os = "linux")]
mod xclip;

#[cfg(test)]
mod tests;

use std::path::PathBuf;



const HELP: &str = "Mochi: Maia's file manager, redesigned in Catppuccin

Usage: mochi [OPTIONS] [PATH...]

Options:
  --install        Install to ~/.local/bin and add a launcher entry
  --set-default    Make Mochi the default file manager (inode/directory)
  --set-picker     Open and save files for other apps (the desktop portal's dialog)
  --unset-picker   Give the Open and Save dialog back to the desktop
  --uninstall      Remove the launcher entry and installed binary
  --update         Pull Maia's newest commits, re-apply Mochi's changes and reinstall
  --window PATH    Open PATH in a new window
  -h, --help       Show this help
  -V, --version    Show the version
";

/// Windows: `--help` and errors print to the terminal it was started from, if any, and a
/// crash says what happened in a message box (there is no console to see it in).
#[cfg(windows)]
fn windows_startup() {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        signals::log(&format!("panic: {msg}"));
        compat::win::message_box("Mochi stopped", &msg);
    }));
}

fn main() {
    #[cfg(windows)]
    windows_startup();
    memory::tune();
    let mut paths: Vec<PathBuf> = vec![];
    let mut new_window = false;
    // Paths need not be UTF-8 (std::env::args would stop Maia at the first that isn't),
    // and everything after `--` is a path, whatever it starts with.
    let mut args = std::env::args_os().skip(1);
    let mut options = true;
    while let Some(raw) = args.next() {
        let a = if options { raw.to_str().unwrap_or("") } else { "" };
        match a {
            "--" if options => options = false,
            "-h" | "--help" => {
                print!("{HELP}");
                return;
            }
            "-V" | "--version" => {
                println!("mochi {} (on Maia {})", env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_VERSION"));
                return;
            }
            "--update" => {
                match mochi::update_now() {
                    Ok(m) => println!("{m}"),
                    Err(e) => {
                        eprintln!("mochi: {e}");
                        std::process::exit(1);
                    }
                }
                return;
            }
            "--install" | "--set-default" | "--uninstall" => {
                let r = match a {
                    "--install" => install::install(),
                    "--set-default" => install::set_default(),
                    _ => install::uninstall(),
                };
                match r {
                    Ok(m) => println!("{m}"),
                    Err(e) => {
                        eprintln!("mochi: {e}");
                        std::process::exit(1);
                    }
                }
                return;
            }
            "--set-picker" | "--unset-picker" => {
                match portal::set(a == "--set-picker") {
                    Ok(m) => println!("{m}"),
                    Err(e) => {
                        eprintln!("maia: {e}");
                        std::process::exit(1);
                    }
                }
                return;
            }
            // The Open and Save dialog's backend, started by the session bus.
            "--portal" => {
                if !portal::run_backend() {
                    eprintln!("mochi: no session bus, or the portal backend is already running");
                    std::process::exit(1);
                }
                return;
            }
            // "Show in folder" for other apps, started by the session bus: no window
            // until a call needs one.
            "--fm1" => {
                let sock = single::socket_path();
                let address = std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap_or_default();
                if !fm1::headless(&address, &sock, |r| fm1::open_window(r, &sock)) {
                    eprintln!("maia: no session bus, or a window already answers for the file manager");
                    std::process::exit(1);
                }
                return;
            }
            // One Open or Save dialog: the request on standard input, the answer on output.
            "--pick" => {
                pick();
                return;
            }
            "--which" => {
                // Diagnostics: how each file would be opened.
                for p in args.by_ref() {
                    let (label, mime, app, argv) = mime::describe_open(std::path::Path::new(&p));
                    println!("{}\n  type: {label} ({mime})\n  app:  {app}\n  cmd:  {}", p.to_string_lossy(), argv.join(" "));
                }
                return;
            }
            "--window" => {
                new_window = true;
                if let Some(p) = args.next() {
                    paths.push(PathBuf::from(p));
                }
            }
            _ => {
                // Accept plain paths and file:// URIs (from %U).
                let p = raw.to_str().and_then(clipboard::uri_to_path).unwrap_or_else(|| PathBuf::from(&raw));
                let p = if p.is_absolute() { p } else { std::env::current_dir().unwrap_or_default().join(p) };
                paths.push(p);
            }
        }
    }
    // A launch that isn't asking for its own window hands its folders to the open one.
    // Mochi: a launch without folders (the app launcher) always opens a window of its
    // own; only folders sent by other apps may join an open window.
    let listener = if new_window || !config::load_config().single_instance {
        None
    } else {
        let sock = single::socket_path();
        if !paths.is_empty() && single::forward_to(&sock, &paths) {
            return;
        }
        single::claim_at(&sock).map(|l| (l, sock))
    };
    std::thread::spawn(|| {
        install::refresh_icon();
        portal::refresh();
    });
    runner::run(
        runner::Options { title: mochi::NAME, app_id: if cfg!(feature = "testkit") { "io.github.yappologistic.Maia.test" } else { mochi::APP_ID }, size: [1240.0, 780.0], min_size: [520.0, 340.0], icon: install::window_icon(), picker: None },
        paths,
        listener,
    );
}

/// Windows: the Open and Save dialog for other apps is the desktop portal's, which
/// Windows does not have.
#[cfg(windows)]
fn pick() {}

/// A picker window for the request on standard input; its answer goes to standard
/// output once it closes.
#[cfg(unix)]
fn pick() {
    use std::io::Read as _;
    let mut req = vec![];
    let _ = std::io::stdin().take(1 << 20).read_to_end(&mut req);
    // The answer goes out on a copy of the pipe that programs started from the window
    // (an app opened from it, a thumbnailer) do not inherit: the backend reads until
    // every writer has gone, and one still running would hold the answer back.
    let answer = unsafe { libc::fcntl(1, libc::F_DUPFD_CLOEXEC, 3) };
    if answer < 0 {
        return;
    }
    unsafe {
        let null = libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC);
        if null >= 0 {
            libc::dup2(null, 1);
            libc::close(null);
        }
    }
    picker::answer_to(answer);
    let p = picker::Picker::new(portal::Pick::decode(&req));
    picker::begin();
    picker::warm(&p.pick);
    let start = p.start();
    // A class of its own, for window rules: a compositor can float and size dialogs.
    let title: &'static str = Box::leak(p.title().into_boxed_str());
    let app_id = if cfg!(feature = "testkit") { "io.github.yappologistic.Maia.test" } else { "io.github.bardiat94.Mochi.Picker" };
    runner::run(runner::Options { title, app_id, size: [980.0, 640.0], min_size: [520.0, 340.0], icon: install::window_icon(), picker: Some(Box::new(p)) }, vec![start], None);
    picker::send_answer();
}
