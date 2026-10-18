//! Embeds the app icon and version information into the Windows executable.
//!
//! The version comes from Cargo.toml. Windows needs four numbers, so the last
//! field orders prereleases before the release: `0.2.0-rc.1` → 0.2.0.1,
//! `0.2.0-rc.2` → 0.2.0.2, `0.2.0` → 0.2.0.1000. scripts/version.sh and
//! scripts/Version.ps1 apply the same mapping for the installer and macOS bundle.

use std::{env, fs, path::Path};

fn build_number(pre: &str) -> u32 {
    if pre.is_empty() {
        return 1000;
    }
    pre.strip_prefix("rc.")
        .and_then(|n| n.parse().ok())
        .filter(|n| (1..1000).contains(n))
        .unwrap_or_else(|| panic!("unsupported prerelease `{pre}`: use -rc.N (1-999) or none"))
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icon/nswpedia-search.ico");
    let pre = env::var("CARGO_PKG_VERSION_PRE").unwrap();
    let build = build_number(&pre);
    // Only the desktop executable is a Windows app; the CLI checker and tests
    // don't need resources.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var_os("CARGO_FEATURE_DESKTOP").is_none()
    {
        return;
    }

    let var = |name| env::var(name).unwrap();
    let numeric = format!(
        "{},{},{},{build}",
        var("CARGO_PKG_VERSION_MAJOR"),
        var("CARGO_PKG_VERSION_MINOR"),
        var("CARGO_PKG_VERSION_PATCH")
    );
    let version = var("CARGO_PKG_VERSION");
    let icon = Path::new(&var("CARGO_MANIFEST_DIR")).join("assets/icon/nswpedia-search.ico");
    let icon = icon.display().to_string().replace('\\', "\\\\");
    // VS_FF_PRERELEASE for -rc builds.
    let flags = if pre.is_empty() { 0 } else { 2 };
    // GPUI loads the window/taskbar icon from resource ID 1.
    let rc = format!(
        r#"1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEFLAGSMASK 0x3f
FILEFLAGS {flags}
FILEOS 0x40004
FILETYPE 0x1
FILESUBTYPE 0x0
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "NSWpedia Search"
      VALUE "FileDescription", "NSWpedia Search"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "nswpedia-search"
      VALUE "LegalCopyright", "MIT License"
      VALUE "OriginalFilename", "nswpedia-search.exe"
      VALUE "ProductName", "NSWpedia Search"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    );
    let path = Path::new(&var("OUT_DIR")).join("app.rc");
    fs::write(&path, rc).unwrap();
    embed_resource::compile_for(&path, ["nswpedia-search"], embed_resource::NONE)
        .manifest_required()
        .unwrap();
}
