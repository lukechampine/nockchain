use shaderc::{
    CompileOptions, Compiler, IncludeCallbackResult, IncludeType, ResolvedInclude, ShaderKind,
};
use std::path::Path;

pub fn build_shaders(out_dir: impl AsRef<Path>, emit_cargo_rerun: bool, emulate_extended_math: bool) {
    let shader_root = concat!(env!("CARGO_MANIFEST_DIR"), "/shaders");
    let shader_ext = "glsl";

    let get_shader_contents = |name: &str| {
        let path = format!("{shader_root}/{name}.{shader_ext}");
        eprintln!("{path}");
        if emit_cargo_rerun {
            println!("cargo:rerun-if-changed={path}");
        }
        let contents = std::fs::read_to_string(&path).unwrap();
        (path, contents)
    };

    let compiler = Compiler::new().unwrap();

    let mut options = CompileOptions::new().unwrap();
    options.set_optimization_level(shaderc::OptimizationLevel::Performance);
    options.set_include_callback(|name, ty, _src, _src_depth| {
        if ty == IncludeType::Relative {
            return IncludeCallbackResult::Err("Relative unsupported".to_string());
        }
        let (path, content) = get_shader_contents(name);
        eprintln!("Include {path}\n{content}");
        IncludeCallbackResult::Ok(ResolvedInclude {
            resolved_name: path,
            content,
        })
    });

    let math_emu = emulate_extended_math as usize;

    for shader in ["hash_fixed"] {
        let artifact = compiler
            .compile_into_spirv(
                &format!(
                    r"#version 460
#extension GL_ARB_gpu_shader_int64 : require
#define EMULATE_EXTENDED_MATH {math_emu}
#include <lib>
#include <{shader}>"
                ),
                ShaderKind::Compute,
                &format!("{shader}.glsl"),
                "main",
                Some(&options),
            )
            .unwrap();

        let spv_path = out_dir.as_ref().join(format!("{shader}.spv"));
        std::fs::write(&spv_path, artifact.as_binary_u8()).expect("Unable to write SPIR-V file");
    }
}
