layout(local_size_x = 256) in;
layout(std430, binding = 0) readonly buffer OpsBuf {
    VariableReduceOp ops[];
};
layout(std140, binding = 1) uniform Globals {
    uint opsOffset;
    uint numOps;
    uint inpOffset;
    uint outOffset;
};
// Output of the shader.
layout(std430, binding = 2) buffer OutputBuf {
    uint64_t outBuf[];
};
// Input to the shader. The length of the array is determined by what buffer is bound.
//
// Out of bounds accesses
layout(std430, binding = 3) readonly buffer InputBuf {
    uint64_t inpBuf[];
};

// Ideal workgroup size depends on the hardware, the workload, and other factors. However, it should
// _generally_ be a multiple of 64. Common sizes are 64x1x1, 256x1x1; or 8x8x1, 16x16x1 for 2D workloads.
void main() {
    // While compute invocations are 3d, we're only using one dimension.
    uint idx = gl_GlobalInvocationID.x + opsOffset;

    // Because we're using a workgroup size of 64, if the input size isn't a multiple of 64,
    // we will have some "extra" invocations. This is fine, but we should tell them to stop
    // to avoid out-of-bounds accesses.
    if (idx >= opsOffset + numOps) {
        return;
    }

    // Do the multiply by two and write to the output.
    VariableReduceOp op = ops[idx];

    // let mut spo = new_sponge(true);
    Sponge tmp = variableSponge;

    // TODO:
    // - pad the input to "1 0 0 .. 0" so it is exactly multiple of tip5Rate (could be done on cpu?)
    // - iterate over input in chunks of tip5Rate
    // - copy each input element (with length of tip5Rate) to tmp sponge (always beginning)
    // - run permute(sponge) for each input chunk

    /*if (idx + opsOffset == 0) {
        tip5SpongePrint(tmp);
        debugPrintfEXT("%08x%08x", uint(inpBuf[op.inner.source - inpOffset] >> 32), uint(inpBuf[op.inner.source - inpOffset]));
    }*/

    // absorb_sponge::<true, T>(&mut spo, input);
    for (uint j = 0; j < op.len; j += tip5Rate) {
        for (uint i = 0; i < tip5Rate; i += 1) {
            spongeSet(tmp, i, inpBuf[op.inner.source + j + i - inpOffset]);
        }
        /*if (idx + opsOffset == 0) {
            tip5SpongePrint(tmp);
        }*/
        tip5Permute(tmp);
    }
    /*if (idx + opsOffset == 0) {
        tip5SpongePrint(tmp);
    }*/

    // let output = squeeze_sponge(spo);
    // output[..DIGEST_LENGTH].try_into().unwrap()
    for (uint i = 0; i < 5; i += 1) {
        outBuf[op.inner.destination + i - outOffset] = spongeGet(tmp, i);
    }
}
