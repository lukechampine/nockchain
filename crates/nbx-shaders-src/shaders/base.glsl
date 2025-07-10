#if !defined(BASE_GLSL)
#define BASE_GLSL

#include <generated/base>

const uint64_t PRIME = uint64_t(0xFFFFFFFFu) << 32 | uint64_t(1u);

struct ReduceOp {
    uint source;
    uint destination;
};

struct VariableReduceOp {
    ReduceOp inner;
    uint len;
};

bool lessThan(uint64_t a, uint64_t b) {
    return a < b;
}
bool lessThan(uint a, uint b) {
    return a < b;
}
bool lessThan(int a, int b) {
    return a < b;
}
bool notEqual(uint a, uint b) {
    return a != b;
}

#define U64 uint64_t
#define U32 uint
#define I32 int
#define U128 uint128_t
#define BOOL bool
#include <gbase>

#define U64 u64vec2
#define U32 uvec2
#define I32 ivec2
#define U128 u128vec2
#define BOOL bvec2
#include <gbase>

#define U64 u64vec3
#define U32 uvec3
#define I32 ivec3
#define U128 u128vec3
#define BOOL bvec3
#include <gbase>

#define U64 u64vec4
#define U32 uvec4
#define I32 ivec4
#define U128 u128vec4
#define BOOL bvec4
#include <gbase>

#endif
