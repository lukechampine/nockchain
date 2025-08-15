use std::borrow::Cow;
use std::sync::LazyLock;

use wgpu::{Device, Label, ShaderModule, ShaderModuleDescriptor, ShaderSource};

#[cfg(any(not(feature = "shader_passthrough"), target_os = "linux"))]
macro_rules! shader_ext {
    () => (".spv");
}
#[cfg(all(feature = "shader_passthrough", target_os = "macos"))]
macro_rules! shader_ext {
    () => (".msl");
}

#[cfg(not(feature = "comptime"))]
macro_rules! shader_path {
    ($shader:literal) => {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../nbx-shaders-build/shaders-out/",
            $shader,
            shader_ext!(),
        )
    };
}

#[cfg(feature = "comptime")]
macro_rules! shader_path {
    ($shader:literal) => {
        concat!(env!("OUT_DIR"), "/", $shader, shader_ext!())
    };
}

#[cfg(all(feature = "shader_passthrough", target_os = "macos"))]
type ShaderElem = str;
#[cfg(any(not(feature = "shader_passthrough"), target_os = "linux"))]
type ShaderElem = [u32];

macro_rules! include_shader {
    ($shader:literal) => {{
        //let bytes = include_bytes!(shader_path!($shader));

        let shader: LazyLock<Cow<'static, ShaderElem>> = LazyLock::new(|| {
            #[cfg(target_os = "macos")]
            let make_shader = |b: &'static [u8]| Cow::Borrowed(str::from_utf8(b).unwrap());

            #[cfg(any(not(feature = "shader_passthrough"), target_os = "linux"))]
            let make_shader = wgpu::util::make_spirv_raw;

            #[cfg(feature = "comptime")]
            let bytes = include_bytes!(shader_path!($shader));
            #[cfg(not(feature = "comptime"))]
            let bytes = {
                let bytes = std::fs::read(shader_path!($shader)).unwrap();
                Box::leak(Box::from(bytes))
            };
            make_shader(bytes)
        });

        ($shader, shader)
    }};
}

static SHADERS: [(&str, LazyLock<Cow<'static, ShaderElem>>); 17] = [
    include_shader!("hash_fixed"),
    include_shader!("hash_variable"),
    include_shader!("substitute_mul"),
    include_shader!("substitute_accum"),
    include_shader!("bp_shift"),
    include_shader!("bp_ntt"),
    include_shader!("fp_ntt"),
    include_shader!("p_ntt_swap"),
    include_shader!("mary_transpose"),
    include_shader!("montify"),
    include_shader!("montyred"),
    include_shader!("hash_varlen_multiple"),
    include_shader!("hash_fixed_multiple"),
    include_shader!("hash_10_fixedprepend"),
    include_shader!("weighted_combo_finish"),
    include_shader!("fp_hadamard_samepoly"),
    include_shader!("fp_accum"),
];

pub fn all_shader_names() -> impl Iterator<Item = &'static str> {
    SHADERS.iter().map(|(v, _)| *v)
}

pub fn get_shader_module(device: &Device, shader: &str) -> ShaderModule {
    let source = &**SHADERS.iter().find(|(s, _)| *s == shader).unwrap().1;

    #[cfg(all(feature = "shader_passthrough", target_os = "linux"))]
    let module = unsafe {
        device.create_shader_module_passthrough(wgpu::ShaderModuleDescriptorPassthrough::SpirV(
            wgpu::ShaderModuleDescriptorSpirV {
                label: Label::Some(shader),
                source: source.into(),
            },
        ))
    };

    #[cfg(all(feature = "shader_passthrough", target_os = "macos"))]
    let module = unsafe {
        device.create_shader_module_passthrough(wgpu::ShaderModuleDescriptorPassthrough::Msl(
            wgpu::ShaderModuleDescriptorMsl {
                entry_point: "main".to_string(),
                label: Label::Some(shader),
                source: source.into(),
                num_workgroups: (256, 0, 0),
            },
        ))
    };

    #[cfg(not(feature = "shader_passthrough"))]
    let module = {
        let module_descriptor = ShaderModuleDescriptor {
            label: Label::Some(shader),
            source: ShaderSource::SpirV(source.into()),
        };

        device.create_shader_module(module_descriptor)
    };

    module
}
