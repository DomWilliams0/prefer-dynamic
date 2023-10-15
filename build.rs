#[macro_use]
extern crate build_cfg;

use std::path::Path;
use std::{borrow::Cow, ffi::OsStr, path::PathBuf};

#[build_cfg_main]
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=OUT_DIR");
    println!("cargo:rerun-if-env-changed=RUSTUP_HOME");
    println!("cargo:rerun-if-env-changed=RUSTUP_TOOLCHAIN");

    let target_dir =
        PathBuf::from(std::env::var("OUT_DIR").expect("Expected OUT_DIR env var to bet set"));

    let target_dir = if target_dir.join("deps").is_dir() {
        Cow::Owned(target_dir)
    } else {
        Cow::Borrowed(
            target_dir
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap(),
        )
    };

    let rustc = std::env::var("RUSTC").expect("Expected RUSTC env var to bet set");
    let sysroot = String::from_utf8(
        std::process::Command::new(rustc)
            .args(["--print", "sysroot"])
            .output()
            .expect("rustc sysroot")
            .stdout,
    )
    .expect("sysroot as utf8");
    let sysroot = sysroot.trim();

    let target = std::env::var("TARGET").expect("Expected TARGET env var to bet set");

    let lib_ext = OsStr::new(if build_cfg!(target_os = "windows") {
        "dll"
    } else if build_cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    });

    let lib_path = PathBuf::from(sysroot);

    let mut found = false;
    let mut remaining = 1;
    if cfg!(feature = "link-test") {
        remaining += 1
    };
    for lib_path in [
        lib_path.join("lib"),
        lib_path.join("bin"),
        lib_path.join("lib/rustlib").join(target).join("lib"),
    ] {
        if !lib_path.is_dir() {
            continue;
        }
        for lib in lib_path
            .read_dir()
            .expect("Failed to read toolchain directory")
            .map(|entry| entry.expect("Failed to read toolchain directory entry"))
            .filter_map(|entry| {
                if entry
                    .file_type()
                    .expect("Failed to read toolchain directory entry type")
                    .is_file()
                {
                    Some(entry.path())
                } else {
                    None
                }
            })
            .filter(|path| path.extension() == Some(lib_ext))
        {
            if remaining <= 0 {
                break;
            }
            if let Some(os_file_name) = lib.file_name() {
                let file_name = os_file_name.to_string_lossy();
                let file_name = file_name
                    .strip_prefix("lib")
                    .unwrap_or_else(|| file_name.as_ref());
                if file_name.starts_with("std-") {
                    remaining -= 1;
                    let dst = target_dir.join(os_file_name);
                    if !dst.exists() {
                        copy_file(&lib, &dst).expect("Failed to copy std lib to target directory");
                    }
                } else if cfg!(feature = "link-test") && file_name.starts_with("test-") {
                    remaining -= 1;
                    let dst = target_dir.join(os_file_name);
                    if !dst.exists() {
                        copy_file(&lib, &dst).expect("Failed to copy test lib to target directory");
                    }
                }
            }
        }
        if remaining <= 0 {
            break;
        }
    }

    if remaining > 0 {
        panic!(
            "Failed to find std lib in toolchain directory!
            lib_path: {lib_path:?}
            lib_ext: {lib_ext:?}"
        );
    }
}

fn copy_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::copy(src, dst)?;

    #[cfg(feature = "filetime")]
    {
        let src_meta = src.metadata()?;
        filetime::set_file_mtime(
            dst,
            filetime::FileTime::from_last_modification_time(&src_meta),
        )?;
    }

    Ok(())
}
