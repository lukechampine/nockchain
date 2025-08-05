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

    uint twiddleIdx = elemIdx % twiddles.length();
    uint twiddleNum = elemIdx / twiddles.length();

    uint offU = off + polyIdx * polyLen + twiddleNum * (twiddles.length() * 2) + twiddleIdx;
    uint offV = offU + twiddles.length();

    U64 w = twiddles[twiddleIdx];
    U64 u = buf[offU];
    U64 v = bmul(buf[offV], w);

    buf[offU] = badd(u, v);
    buf[offV] = bsub(u, v);
}
