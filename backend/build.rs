use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/hash.c");

    let out_path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("lib")
        .join("hash.so");

    Command::new("gcc")
        .args(&["-fPIC", "-shared", "src/hash.c", "-o"])
        .arg(&out_path)
        .status()
        .unwrap();
}
