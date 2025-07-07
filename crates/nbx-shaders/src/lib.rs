use std::borrow::Cow;
use std::sync::LazyLock;

use wgpu::{Device, Label, ShaderModule, ShaderModuleDescriptor, ShaderSource};

#[cfg(not(feature = "comptime"))]
macro_rules! shader_path {
    ($shader:literal) => {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../nbx-shaders-build/shaders-out/",
            $shader,
            ".spv"
        )
    };
}

#[cfg(feature = "comptime")]
macro_rules! shader_path {
    ($shader:literal) => {
        concat!(env!("OUT_DIR"), "/", $shader, ".spv")
    };
}

macro_rules! include_shader {
    ($shader:literal) => {{
        //let bytes = include_bytes!(shader_path!($shader));
        let spirv: LazyLock<Cow<'static, [u32]>> = LazyLock::new(|| {
            #[cfg(feature = "comptime")]
            let bytes = include_bytes!(shader_path!($shader));
            #[cfg(not(feature = "comptime"))]
            let bytes = {
                let bytes = std::fs::read(shader_path!($shader)).unwrap();
                Box::leak(Box::from(bytes))
            };
            wgpu::util::make_spirv_raw(bytes)
        });

        ($shader, spirv)
    }};
}

static SHADERS: [(&str, LazyLock<Cow<'static, [u32]>>); 2] =
    [include_shader!("hash_fixed"), include_shader!("hash_variable")];

pub fn get_shader_module(device: &Device, shader: &str) -> ShaderModule {
    let source = &**SHADERS.iter().find(|(s, _)| *s == shader).unwrap().1;

    #[cfg(feature = "spirv_passthrough")]
    let module = unsafe {
        device.create_shader_module_passthrough(wgpu::ShaderModuleDescriptorPassthrough::SpirV(
            wgpu::ShaderModuleDescriptorSpirV {
                label: Label::Some(shader),
                source: source.into(),
            },
        ))
    };

    #[cfg(not(feature = "spirv_passthrough"))]
    let module = {
        let module_descriptor = ShaderModuleDescriptor {
            label: Label::Some(shader),
            source: ShaderSource::SpirV(source.into()),
        };

        device.create_shader_module(module_descriptor)
    };

    module
}
