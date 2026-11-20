use std::{env, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-changed=data/icons");
    println!("cargo:rerun-if-changed=data/resources.xml");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("icons.gresource");
    assert!(Command::new("glib-compile-resources")
        .args(["data/resources.xml", "--sourcedir=data", "--target"])
        .arg(output).status().expect("glib-compile-resources is required").success());
}
