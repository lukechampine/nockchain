fn main() {
    let emumath = "CARGO_FEATURE_EMULATE_EXTENDED_MATHS";
    println!("cargo:rerun-if-env-changed={emumath}");
    let emumath = std::env::var(emumath).is_ok();
    let out_dir = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("shaders-out");
    std::fs::create_dir_all(&out_dir).unwrap();
    nbx_shaders_src::build_shaders(out_dir, true, emumath);
}
