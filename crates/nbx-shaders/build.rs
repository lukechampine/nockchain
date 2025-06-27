fn main() {
    #[cfg(feature = "comptime")]
    {
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let out_dir = std::path::Path::new(&out_dir);
        nbx_shaders_src::build_shaders(out_dir, true, !cfg!(feature = "spirv_passthrough"));
    }
}
