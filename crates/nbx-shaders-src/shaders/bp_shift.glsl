layout(local_size_x = 256) in;

#define U64 uint64_t

layout(std140, binding = 0) uniform Globals {
    uint inpOffset;
    uint inpStep;
    uint numElems;
    uint outOffset;
    uint outStep;
};

layout(std430, binding = 1) buffer OutputBuf {
    U64 outBuf[];
};

layout(std430, binding = 2) buffer InputBuf {
    U64 inpBuf[];
};

layout(std430, binding = 3) buffer PowersBuf {
    U64 powersBuf[];
};

void main() {
    uint idx = gl_GlobalInvocationID.x;

    if (idx >= numElems) {
        return;
    }

    uint stepIdx = idx % inpStep;
    uint stepNum = idx / inpStep;

    U64 power = powersBuf[stepIdx];
    U64 a = inpBuf[inpOffset + idx];

    U64 res = bmul(a, power);

    outBuf[outOffset + (outStep * stepNum) + stepIdx] = res;
}
