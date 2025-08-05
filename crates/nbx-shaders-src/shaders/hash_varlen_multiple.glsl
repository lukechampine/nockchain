layout(local_size_x = 256) in;
layout(std140, binding = 0) uniform Globals {
    uint startIdx;
    uint numHashes;
    uint polyLen;
};
// Output of the shader.
layout(std430, binding = 1) buffer OutputBuf {
    uint64_t outBuf[];
};
// Input to the shader. The length of the array is determined by what buffer is bound.
//
// Out of bounds accesses
layout(std430, binding = 2) readonly buffer InputBuf {
    uint64_t inpBuf[];
};

// Ideal workgroup size depends on the hardware, the workload, and other factors. However, it should
// _generally_ be a multiple of 64. Common sizes are 64x1x1, 256x1x1; or 8x8x1, 16x16x1 for 2D workloads.
void main() {
    // While compute invocations are 3d, we're only using one dimension.
    uint idx = gl_GlobalInvocationID.x;

    // Because we're using a workgroup size of 64, if the input size isn't a multiple of 64,
    // we will have some "extra" invocations. This is fine, but we should tell them to stop
    // to avoid out-of-bounds accesses.
    if (idx >= numHashes) {
        return;
    }

    uint srcOff = (startIdx + idx) * polyLen;
    uint dstOff = (startIdx + idx) * 5;

    // let mut spo = new_sponge(true);
    Sponge tmp = variableSponge;

    /*if (srcOff == 0) {
        tip5SpongePrint(tmp);
        debugPrintfEXT("%08x%08x", uint(inpBuf[srcOff] >> 32), uint(inpBuf[srcOff]));
    }*/

    // Absorb all aligned regions
    uint amtLeft = polyLen % tip5Rate;
    uint amtOff = polyLen - amtLeft;

    for (uint j = 0; j < amtOff; j += tip5Rate) {
        for (uint i = 0; i < tip5Rate; i += 1) {
            spongeSet(tmp, i, inpBuf[srcOff + j + i]);
        }
        if (srcOff == 0) {
            tip5SpongePrint(tmp);
        }
        tip5Permute(tmp);
    }


    // And do padded absorb
    /*if (srcOff == 0) {
        debugPrintfEXT("POST UNPADDED %u %u", amtLeft, amtOff);
    }*/

    for (uint i = 0; i < amtLeft; i += 1) {
        spongeSet(tmp, i, inpBuf[srcOff + amtOff + i]);
    }
    spongeSet(tmp, amtLeft, oneMelt);
    for (uint i = amtLeft + 1; i < tip5Rate; i += 1) {
        spongeSet(tmp, i, 0);
    }

    /*if (srcOff == 0) {
        tip5SpongePrint(tmp);
    }*/
    tip5Permute(tmp);

    if (srcOff == 0) {
        tip5SpongePrint(tmp);
    }

    // let output = squeeze_sponge(spo);
    // output[..DIGEST_LENGTH].try_into().unwrap()
    for (uint i = 0; i < 5; i += 1) {
        outBuf[dstOff + i] = spongeGet(tmp, i);
    }
}
