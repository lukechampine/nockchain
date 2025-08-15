fn main() {
    #[cfg(feature = "comptime")]
    {
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let out_dir = std::path::Path::new(&out_dir);
        nbx_shaders_src::build_shaders(
            nbx_shaders_src::BuildOptions::new(out_dir)
                .emit_cargo_rerun(true)
                .emulate_extended_math(!cfg!(feature = "shader_passthrough")),
        );
    }
}
