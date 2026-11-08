const LINKER_SCRIPT: &str = "src/board/raspi4/link.ld";

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-arg=-T{manifest_dir}/{LINKER_SCRIPT}");
    println!("cargo:rerun-if-changed={LINKER_SCRIPT}");
}
