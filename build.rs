// Windows builds: the app's icon and version in the .exe, for Explorer, shortcuts and the
// taskbar. Other targets have nothing to do here.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/mochi.ico");
    println!("cargo:rerun-if-changed=MOCHI_VERSION");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets/mochi.ico");
    // Mochi's own version, as the app and its releases say it.
    let v = std::fs::read_to_string(std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("MOCHI_VERSION")).unwrap_or_default().trim().to_string();
    let mut n: Vec<u32> = v.split(['.', '-']).filter_map(|p| p.parse().ok()).collect();
    n.resize(4, 0);
    let rc = format!(
        r#"1 ICON "{ico}"
1 VERSIONINFO
FILEVERSION {a},{b},{c},{d}
PRODUCTVERSION {a},{b},{c},{d}
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "FileDescription", "Mochi"
      VALUE "ProductName", "Mochi"
      VALUE "FileVersion", "{v}"
      VALUE "ProductVersion", "{v}"
      VALUE "OriginalFilename", "mochi.exe"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        ico = ico.display().to_string().replace('\\', "/"),
        a = n[0],
        b = n[1],
        c = n[2],
        d = n[3],
    );
    let rc_path = out.join("mochi.rc");
    let obj = out.join("mochi-res.o");
    std::fs::write(&rc_path, rc).unwrap();
    let windres = std::env::var("WINDRES").unwrap_or_else(|_| "x86_64-w64-mingw32-windres".into());
    match std::process::Command::new(&windres).arg(&rc_path).arg("-O").arg("coff").arg("-o").arg(&obj).status() {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg-bins={}", obj.display()),
        _ => println!("cargo:warning={windres} not found or failed: mochi.exe gets no icon"),
    }
}
