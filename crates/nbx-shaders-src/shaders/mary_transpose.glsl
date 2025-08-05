layout(local_size_x = 256) in;

#define U64 uint64_t

layout(std140, binding = 0) uniform Globals {
    uint off;
    uint numElems;
    uint maryStep;
    uint maryLen;
    uint maryOff;
};

layout(std430, binding = 1) buffer OutputBuf {
    U64 outBuf[];
};

layout(std430, binding = 2) buffer InputBuf {
    U64 inpBuf[];
};

void main() {
    uint idx = gl_GlobalInvocationID.x;

    if (idx >= numElems) {
        return;
    }

    uint numCols = maryStep / maryOff;
    uint numRows = maryLen;

    uint curIdx = off + idx;

    uint inner = curIdx / maryOff;
    uint k = curIdx % maryOff;
    uint j = inner / numCols;
    uint i = inner % numCols;

    uint newIdx = maryOff * (i * numRows + j) + k;

    outBuf[newIdx] = inpBuf[curIdx];
}
