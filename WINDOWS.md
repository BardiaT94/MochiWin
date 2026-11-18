# MochiWin: the Windows port

MochiWin is a separate fork of Mochi that also builds for Windows. Linux builds work exactly
as in Mochi. This note says what works on Windows so far and what is still switched off.

## How it is built

| Target | Command |
| :--- | :--- |
| Linux | `cargo build` |
| Windows check (no linker needed) | `cargo check --target x86_64-pc-windows-gnu` |
| Windows `.exe` (needs `mingw-w64-gcc`, `7z`) | `MESA_7Z=<path> tools/win-build.sh` |
| Installer as well | add `makensis` (AUR `nsis`), or `NSIS_DIR=<unzipped NSIS>` to run its `makensis.exe` under Wine |

The script leaves `dist-win/`. The Rust package is still named `maia`, as in Mochi, so Cargo
writes `maia.exe`; the script renames the copy to `mochi.exe`.

```text
MochiSetup.exe                 the installer (only when makensis was found)
dist-win/
  mochi.exe                    the app, with its icon and version (build.rs, windres)
  FONTS-LICENSE.txt            the OFL notice for the built-in fonts
  software-gl/mochi.exe        the same app
  software-gl/opengl32.dll     Mesa's software OpenGL (llvmpipe)
  software-gl/libgallium_wgl.dll
  software-gl/MESA-LICENSE.txt
```

`MESA_7Z` is `mesa3d-<version>-release-mingw.7z` from
[mesa-dist-win](https://github.com/pal1000/mesa-dist-win/releases). The MSVC package has no
OpenGL driver. Without `MESA_7Z` the `software-gl` folder is left out.

### Installer

`packaging/windows/mochi.nsi` makes `MochiSetup.exe` from `dist-win/`:

- Installs for the current user only, to `%LOCALAPPDATA%\Programs\Mochi`. No admin rights.
- Start menu shortcut; a desktop shortcut that can be unticked.
- Listed in Settings > Apps, with an uninstaller. Uninstalling removes only Mochi's own files
  and keeps the settings in `%APPDATA%\mochi`.
- Asks to close a running Mochi before it copies files.
- Offers to start Mochi at the end.

It is not code-signed, so SmartScreen warns on first run ("More info", then "Run anyway").

### Software OpenGL fallback

Mochi needs OpenGL 3. A PC without a GPU driver (a VM, Remote Desktop) only has OpenGL 1.1.
When OpenGL fails, `mochi.exe` starts `software-gl\mochi.exe` with the same arguments and
exits. That copy loads Mesa's `opengl32.dll` from its own folder and draws on the CPU. A
copy of the exe is needed because Windows loads `opengl32.dll` from the program's folder
first, and a running process can't switch to a different one. If the fallback also fails,
a message box says why.

The folder adds about 86 MB (78 MB of it is Mesa's driver).

### Testing under Wine

- Wine's own OpenGL with an NVIDIA GPU fails, so the fallback starts. To let Wine use the
  bundled Mesa, set `WINEDLLOVERRIDES="opengl32=n,b"`.
- In a normal Wine window on Niri the frame stays blank. Inside a Wine virtual desktop
  (`wine explorer /desktop=test,1280x800 mochi.exe`) Mochi draws fully.
- A new Wine prefix links its Desktop, Documents and so on to the real home folders, and
  winemenubuilder adds Linux menu entries for shortcuts. Before running an installer in a test
  prefix, replace those links in `drive_c/users/<user>/` with empty folders and set
  `WINEDLLOVERRIDES="winemenubuilder.exe=d"`.
- Wine has no PowerShell. For the terminal pane, unzip PowerShell 7's `win-x64` zip into the
  prefix (for example `C:\pwsh`) and start with `WINEPATH='C:\pwsh'`.

## How the port is organised

- `src/compat.rs` has the Unix calls the code uses (`fs`, `ffi`, `process`, `net`, `sys`,
  `time`). On Linux it re-exports the standard library and libc, so nothing changes there. On
  Windows it has stand-ins, plus a `win` section with real Windows calls.
- `src/win/` has Windows versions of modules that are Linux-only as a whole: folder watching,
  following a growing file, and the shared clipboard types.
- Everything else uses `#[cfg(unix)]` or `#[cfg(windows)]` right where the code differs.
- `Cargo.toml` puts the Wayland, X11 and EGL/GLX dependencies under Linux only. Windows gets
  WGL for OpenGL and `windows-sys` for system calls.

## Stage 1: what works on Windows

| Feature | Windows behaviour |
| :--- | :--- |
| Window, drawing, fonts | winit + OpenGL (WGL); Exo 2 and Hurmit Nerd Font Mono built in (a font chosen in Settings still wins) |
| Text size | Default 14 pt (Linux: 12), times Settings > Accessibility > Text size. Settings saved at the old 12 move to it once |
| Browse folders | Yes; Storage lists each drive letter, named as Explorer does ("Local Disk (C:)") |
| Search in subfolders, Expand | Yes; folders are told apart by volume serial and file index (`fsx::file_id`), as Unix uses device and inode |
| Bookmarks | Maia's Bookmarks section in the sidebar (pinned folders), as on Linux |
| Terminal pane | ConPTY with PowerShell 7 (`pwsh.exe`) when on PATH, else Windows PowerShell |
| Copy, move, rename | Yes, through a buffered copy; "rename only if free" uses `MoveFileExW` |
| Delete | Moves to Mochi's own trash folder, not the Recycle Bin yet |
| Free space | `GetDiskFreeSpaceExW` |
| Network drives | Detected (mapped letters and `\\server\share`) |
| Open a file | The default app for its type (`ShellExecuteW`) |
| Open a terminal here | Windows Terminal, else PowerShell |
| Dates | Local time through the C runtime, English day and month names |
| Stop a running tool | `TerminateProcess` |

## Switched off on Windows for now

| Feature | Planned replacement | Stage |
| :--- | :--- | :--- |
| Recycle Bin | Shell file operations | 2 |
| Drives view (mount, eject, format) | Windows drive APIs | 2 |
| "Open with" app list | Registry file associations | 2 |
| Live folder refresh | `ReadDirectoryChangesW` | 2 |
| Clipboard and drag out | Windows clipboard and OLE drag-drop | 3 |
| Admin actions | UAC prompt | 3 |
| One window for many launches | Named pipe | 3 |
| Tags, file origin | NTFS streams | 3 |
| Pause and resume a tool | Suspend and resume the process | 3 |
| Replace Explorer (Win+E, opening folders) | Registry keys, opt-in | 4 |

### How the terminal pane works on Windows

ConPTY can't say which program runs in the foreground or which folder the shell is in, so the
shell tells Mochi. PowerShell starts with `-EncodedCommand` (see `win_shell` in
`terminal.rs`), which wraps the user's own prompt. Each prompt then:

- writes a count and the folder to `%TEMP%\mochi-prompt-<pid>-<n>.txt`
  (`MOCHI_PROMPT_FILE`). A prompt counted since the last Enter means the shell is idle;
- prints OSC 7 (`file://localhost/C:/...`, ended with ESC \), for the tab's name and links.

Following the panel types Ctrl+Home, Ctrl+End (clears the line) and
` Set-Location -LiteralPath '<dir>'`. A PSReadLine history handler keeps those lines out
of the history. Other shells (cmd, Git Bash) are not set up for this.

Tested under Wine 11 with PowerShell 7.4: start, typing, output, idle and folder reports, and
following the panel all work. Wine's console garbles PowerShell's colour codes a little; real
Windows still needs a check.

Things with no Windows equivalent stay off: the desktop portal's Open/Save dialog, the D-Bus
"show in folder" service, Unix owners and modes (shown as read-only or writable only).

> [!WARNING]
> Moving files between drives does not force the copy onto the disk before the original is
> deleted. Windows writes it back on its own schedule, as Explorer does. A power cut right after
> a move could lose the file.
