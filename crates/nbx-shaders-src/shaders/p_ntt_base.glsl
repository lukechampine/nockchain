layout(local_size_x = 256) in;

#define U64 uint64_t

layout(std140, binding = 0) uniform Globals {
    uint off;
    uint i;
    uint polyLen;
    uint numElems;
};

// Output of the shader.
layout(std430, binding = 1) buffer OutputBuf {
    U64 buf[];
};

// Input to the shader. The length of the array is determined by what buffer is bound.
//
// Out of bounds accesses
layout(std430, binding = 2) buffer TwiddlesBuf {
    U64 twiddles[];
};

#define OFF 0

// Ideal workgroup size depends on the hardware, the workload, and other factors. However, it should
// _generally_ be a multiple of 64. Common sizes are 64x1x1, 256x1x1; or 8x8x1, 16x16x1 for 2D workloads.
void main() {
    // While compute invocations are 3d, we're only using one dimension.
    uint idx = gl_GlobalInvocationID.x;

    if (idx >= numElems) {
        return;
    }

    uint elemIdx = (i + idx) % (polyLen / 2);
    uint polyIdx = (i + idx) / (polyLen / 2);

    uint twiddleIdx = elemIdx % BUF_LEN(twiddles);
    uint twiddleNum = elemIdx / BUF_LEN(twiddles);

    uint offU = off + polyIdx * polyLen + twiddleNum * (BUF_LEN(twiddles) * 2) + twiddleIdx;
    uint offV = offU + BUF_LEN(twiddles);

    /*if (offU == OFF) {
        debugPrintfEXT("OFF 0: %u %u %u %u %u %u %u", elemIdx, polyIdx, twiddleIdx, twiddleNum, idx, offU, offV);
    }*/

    PELT w = GET_PELT(twiddles, twiddleIdx);
    PELT u = GET_PELT(buf, offU);
    PELT cV = GET_PELT(buf, offV);
    PELT v = PMUL(cV, w);

    /*if (offU == OFF) {
        PRINT_PELT("w: ", w);
        PRINT_PELT("u: ", u);
        PRINT_PELT("v_mut: ", cV);
        PRINT_PELT("v: ", v);
    }*/

    SET_PELT(buf, offU, PADD(u, v));
    SET_PELT(buf, offV, PSUB(u, v));

    /*if (offU == OFF) {
        PRINT_PELT("u: ", GET_PELT(buf, offU));
        PRINT_PELT("v: ", GET_PELT(buf, offV));
    }*/
}
