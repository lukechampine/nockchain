use nbx_shaders_src::BuildOptions;

fn main() {
    let emumath = "CARGO_FEATURE_EMULATE_EXTENDED_MATHS";
    println!("cargo:rerun-if-env-changed={emumath}");
    let emumath = std::env::var(emumath).is_ok();
    let spirv_1_3 = "CARGO_FEATURE_SPIRV_1_3";
    println!("cargo:rerun-if-env-changed={spirv_1_3}");
    let spirv_1_3 = std::env::var(spirv_1_3).is_ok();
    let out_dir =
        std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("shaders-out");
    std::fs::create_dir_all(&out_dir).unwrap();
    nbx_shaders_src::build_shaders(
        BuildOptions::new(out_dir)
            .emit_cargo_rerun(true)
            //.debug_info(true)
            .optimize(true)
            .emulate_extended_math(emumath)
            .spirv_version(if spirv_1_3 { Some(13) } else { None })
            //.printf_ext(true)
            .has_int8(true)
            ,
    );
}
