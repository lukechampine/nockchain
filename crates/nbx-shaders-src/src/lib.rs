use std::path::Path;

use shaderc::{
    CompileOptions, Compiler, IncludeCallbackResult, IncludeType, ResolvedInclude, ShaderKind,
};
use spirv_cross2::{compile::msl, self};

pub struct BuildOptions<T> {
    out_dir: T,
    emit_cargo_rerun: bool,
    emulate_extended_math: bool,
    debug_info: bool,
    optimize: bool,
    printf_ext: bool,
    spirv_version: Option<usize>,
    has_int8: bool,
}

impl<T: AsRef<Path>> BuildOptions<T> {
    pub fn new(out_dir: T) -> Self {
        Self {
            out_dir,
            emit_cargo_rerun: true,
            emulate_extended_math: false,
            debug_info: false,
            optimize: true,
            printf_ext: false,
            spirv_version: None,
            has_int8: false,
        }
    }

    pub fn emit_cargo_rerun(self, emit_cargo_rerun: bool) -> Self {
        Self {
            emit_cargo_rerun,
            ..self
        }
    }

    pub fn emulate_extended_math(self, emulate_extended_math: bool) -> Self {
        Self {
            emulate_extended_math,
            ..self
        }
    }

    pub fn debug_info(self, debug_info: bool) -> Self {
        Self { debug_info, ..self }
    }

    pub fn optimize(self, optimize: bool) -> Self {
        Self { optimize, ..self }
    }

    pub fn printf_ext(self, printf_ext: bool) -> Self {
        Self { printf_ext, ..self }
    }

    pub fn spirv_version(self, spirv_version: Option<usize>) -> Self {
        Self {
            spirv_version,
            ..self
        }
    }

    pub fn has_int8(self, has_int8: bool) -> Self {
        Self { has_int8, ..self }
    }
}

fn u64_to_glsl(v: u64) -> String {
    format!(
        "(uint64_t(0x{:x}U) << 32) | uint64_t(0x{:x}U)",
        v >> 32,
        v as u32
    )
}

fn generate_tip5(has_uint8: bool) -> String {
    use nbx_tip5::tip5::*;

    let tb_ty = if has_uint8 { "uint8_t" } else { "uint" };

    format!(
        r"
#include <base>

const uint tip5DigestLength = {DIGEST_LENGTH};
const uint tip5StateSize = {STATE_SIZE};
const uint tip5NumSplitAndLookup = {NUM_SPLIT_AND_LOOKUP};
const uint tip5Log2StateSize = {LOG2_STATE_SIZE};
const uint tip5Capacity = {CAPACITY};
const uint tip5Rate = {RATE};
const uint tip5NumRounds = {NUM_ROUNDS};
const uint128_t tip5R = uint128_t(0, 1);

const uint tip5LookupTable[256] = uint[256]({lookup_table});
const uint64_t tip5RoundConstants[{rc_size}] = uint64_t[{rc_size}]({round_constants});
const uint64_t tip5MdsMatrix[{STATE_SIZE}][{STATE_SIZE}] = uint64_t[{STATE_SIZE}][{STATE_SIZE}]({mds_matrix});

const u64vec4 tip5RoundConstantsVec[{rc_size} / 4] = u64vec4[{rc_size} / 4]({round_constants_vec});
const u64vec4 tip5MdsMatrixVec[{STATE_SIZE}][{STATE_SIZE} / 4] = u64vec4[{STATE_SIZE}][{STATE_SIZE} / 4]({mds_matrix_vec});
",
        lookup_table = LOOKUP_TABLE
            .iter()
            .map(|v| format!("{tb_ty}({v})"))
            .collect::<Vec<_>>()
            .join(", "),
        rc_size = NUM_ROUNDS * STATE_SIZE,
        round_constants = ROUND_CONSTANTS2
            .into_iter()
            .map(|v| u64_to_glsl(v.0))
            .collect::<Vec<_>>()
            .join(", "),
        round_constants_vec = ROUND_CONSTANTS2
            .chunks(4)
            .map(|v| {
                let v = v
                    .iter()
                    .map(|v| u64_to_glsl(v.0))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("u64vec4({v})")
            })
            .collect::<Vec<_>>()
            .join(", "),
        mds_matrix = MDS_MATRIX_MONT
            .into_iter()
            .map(|m| format!(
                "uint64_t[{STATE_SIZE}]({})",
                m.into_iter()
                    .map(|v| u64_to_glsl(v.0))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
            .collect::<Vec<_>>()
            .join(", "),
        mds_matrix_vec = MDS_MATRIX_MONT
            .into_iter()
            .map(|m| format!(
                "u64vec4[{STATE_SIZE} / 4]({})",
                m.chunks(4)
                    .map(|c| {
                        let c = c
                            .iter()
                            .map(|v| u64_to_glsl(v.0))
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("u64vec4({c})")
                    })
                    .collect::<Vec<_>>()
                    .join(", ") /*m.into_iter()
                                .map(|v| u64_to_glsl(v.0))
                                .collect::<Vec<_>>()
                                .join(", ")*/
            ))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

fn generate_sponge() -> String {
    use nbx_tip5::tip5::*;
    format!(
        r"
const Sponge fixedSponge = SPONGE({fixed_sponge});
const Sponge variableSponge = SPONGE({variable_sponge});
",
        fixed_sponge = [0; RATE]
            .into_iter()
            .map(|v| v.to_string())
            .chain(["oneMelt"; CAPACITY].map(str::to_string))
            .collect::<Vec<_>>()
            .join(", "),
        variable_sponge = [0; STATE_SIZE]
            .into_iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", "),
    )
}

fn generate_base() -> String {
    use nbx_tip5::melt::Melt;

    format!(
        r"
const uint64_t zeroMelt = {zero_melt};
const uint64_t oneMelt = {one_melt};
",
        zero_melt = u64_to_glsl(Melt::from_u64(0).0),
        one_melt = u64_to_glsl(Melt::from_u64(1).0)
    )
}

pub fn build_shaders(options: BuildOptions<impl AsRef<Path>>) {
    let BuildOptions {
        out_dir,
        emit_cargo_rerun,
        emulate_extended_math,
        debug_info,
        printf_ext,
        optimize,
        spirv_version,
        has_int8,
    } = options;

    let shader_root = concat!(env!("CARGO_MANIFEST_DIR"), "/shaders");
    let shader_ext = "glsl";

    let generate_map = [
        ("tip5", generate_tip5(has_int8)),
        ("base", generate_base()),
        ("sponge", generate_sponge()),
    ]
    .into_iter()
    .collect::<std::collections::HashMap<_, _>>();

    let get_shader_contents = |name: &str| {
        let path = format!("{shader_root}/{name}.{shader_ext}");
        let unprefixed = name.strip_prefix("generated/");

        let contents = if let Some(name) = unprefixed {
            generate_map.get(name).unwrap().clone()
        } else {
            eprintln!("{path}");
            if emit_cargo_rerun {
                println!("cargo:rerun-if-changed={path}");
            }
            std::fs::read_to_string(&path).unwrap()
        };

        (path, contents)
    };

    let compiler = Compiler::new().unwrap();

    let mut options = CompileOptions::new().unwrap();

    if debug_info {
        options.set_generate_debug_info();
    }

    if optimize {
        options.set_optimization_level(shaderc::OptimizationLevel::Size);
    }

    let printf_ext = if printf_ext {
        "#extension GL_EXT_debug_printf : require\n#define HAS_PRINTF_EXT\n"
    } else {
        ""
    };

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

    //options.set_target_env(shaderc::TargetEnv::Vulkan, shaderc::EnvVersion::WebGPU as _);
    options.set_target_env(
        shaderc::TargetEnv::Vulkan,
        shaderc::EnvVersion::Vulkan1_1 as _,
    );
    match spirv_version {
        Some(10) => options.set_target_spirv(shaderc::SpirvVersion::V1_0),
        Some(11) => options.set_target_spirv(shaderc::SpirvVersion::V1_1),
        Some(12) => options.set_target_spirv(shaderc::SpirvVersion::V1_2),
        Some(13) => options.set_target_spirv(shaderc::SpirvVersion::V1_3),
        Some(14) => options.set_target_spirv(shaderc::SpirvVersion::V1_4),
        Some(15) => options.set_target_spirv(shaderc::SpirvVersion::V1_5),
        Some(16) => options.set_target_spirv(shaderc::SpirvVersion::V1_6),
        None => (),
        _ => panic!("Unrecognized spirv version"),
    }

    let math_emu = emulate_extended_math as usize;

    for shader in ["hash_fixed", "hash_variable", "substitute_mul", "substitute_accum", "bp_shift", "bp_ntt", "fp_ntt", "p_ntt_swap", "mary_transpose", "montify", "montyred", "hash_varlen_multiple", "hash_fixed_multiple", "hash_10_fixedprepend", "weighted_combo_finish", "fp_hadamard_samepoly", "fp_accum"] {
        let artifact = compiler
            .compile_into_spirv(
                &format!(
                    r"#version 460
#extension GL_EXT_shader_explicit_arithmetic_types : require
{printf_ext}
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

        let spv = spirv_cross2::Module::from_words(words_from_bytes(artifact.as_binary_u8()));
        let mut compiler = spirv_cross2::Compiler::<spirv_cross2::targets::Msl>::new(spv).unwrap();
        compiler.rename_entry_point("main", "main_", spirv_cross2::spirv::ExecutionModel::GLCompute);
        let mut compiler_options = msl::CompilerOptions::default();
        compiler_options.enable_decoration_binding = true;
        compiler_options.version = msl::MslVersion::new(2, 3, 0);
        let msl = compiler.compile(&compiler_options).unwrap();
        let msl_path = out_dir.as_ref().join(format!("{shader}.msl"));
        std::fs::write(&msl_path, msl.to_string().as_bytes()).expect("Unable to write MSL file");
    }
}

fn words_from_bytes(buf: &[u8]) -> &[u32] {
    unsafe {
        std::slice::from_raw_parts(
            buf.as_ptr() as *const u32,
            buf.len() / std::mem::size_of::<u32>(),
        )
    }
}
