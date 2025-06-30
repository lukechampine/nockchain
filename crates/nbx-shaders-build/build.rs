use nbx_shaders_src::BuildOptions;

fn main() {
    let emumath = "CARGO_FEATURE_EMULATE_EXTENDED_MATHS";
    println!("cargo:rerun-if-env-changed={emumath}");
    let emumath = std::env::var(emumath).is_ok();
    let out_dir =
        std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("shaders-out");
    std::fs::create_dir_all(&out_dir).unwrap();
    nbx_shaders_src::build_shaders(
        BuildOptions::new(out_dir)
            .emit_cargo_rerun(true)
            .debug_info(false)
            .optimize(true)
            .emulate_extended_math(emumath),
    );
}
