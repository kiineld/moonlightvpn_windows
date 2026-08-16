//! Stages the font files into `OUT_DIR` so `include_bytes!` always has
//! something to read.
//!
//! The faces are fetched by `scripts/fetch-fonts.ps1` rather than committed —
//! they are ~1 MB of binary each and Google Fonts is their home. A fresh clone
//! that has not run the script must still compile, so an absent face is staged
//! as an empty file: iced's font database refuses to parse it, logs nothing,
//! and text falls back to the system UI font. That is a build which looks wrong
//! but runs, which is the right failure for a missing asset — the alternative
//! is a clone that does not build until you have found the right shell script.

use std::path::{Path, PathBuf};
use std::{env, fs};

const FACES: &[&str] = &[
    "Onest-Medium.ttf",
    "Onest-Bold.ttf",
    "Onest-ExtraBold.ttf",
    "Unbounded-ExtraBold.ttf",
];

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is always set by cargo"));
    let manifest =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set"));
    // crates/moonlight-design → repository root
    let fonts = manifest
        .parent()
        .and_then(Path::parent)
        .expect("the crate always sits two levels below the workspace root")
        .join("resources/fonts");

    println!("cargo:rerun-if-changed={}", fonts.display());

    for face in FACES {
        let source = fonts.join(face);
        let target = out_dir.join(face);
        println!("cargo:rerun-if-changed={}", source.display());

        match fs::read(&source) {
            Ok(bytes) => {
                fs::write(&target, bytes).expect("OUT_DIR is writable");
            }
            Err(_) => {
                println!(
                    "cargo:warning={face} is missing from resources/fonts — \
                     run scripts/fetch-fonts.ps1 (or fetch-fonts.sh). \
                     Text will fall back to the system font."
                );
                fs::write(&target, []).expect("OUT_DIR is writable");
            }
        }
    }
}
