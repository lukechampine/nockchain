layout(local_size_x = 256) in;

layout(std140, binding = 0) uniform Globals {
    uint i;
    uint polyLen;
    uint numElems;
};

layout(std430, binding = 1) buffer OutputBuf {
    Felt outBuf[];
};

layout(std430, binding = 2) buffer FixedBuf {
    Felt fixedBuf[];
};

void main() {
    uint idx = gl_GlobalInvocationID.x;

    if (idx >= numElems) {
        return;
    }

    uint elemIdx = (i + idx) % polyLen;
    outBuf[i + idx] = fmul(outBuf[i + idx], fixedBuf[elemIdx]);
}
