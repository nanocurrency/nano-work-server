#include <metal_stdlib>
#include <metal_atomic>

using namespace metal;

enum Blake2b_IV : uint64_t {
    iv0 = 0x6a09e667f3bcc908UL,
    iv1 = 0xbb67ae8584caa73bUL,
    iv2 = 0x3c6ef372fe94f82bUL,
    iv3 = 0xa54ff53a5f1d36f1UL,
    iv4 = 0x510e527fade682d1UL,
    iv5 = 0x9b05688c2b3e6c1fUL,
    iv6 = 0x1f83d9abfb41bd6bUL,
    iv7 = 0x5be0cd19137e2179UL,
};

enum IV_Derived : uint64_t {
    nano_xor_iv0 = 0x6a09e667f2bdc900UL,  // iv1 ^ 0x1010000 ^ outlen
    nano_xor_iv4 = 0x510e527fade682f9UL,  // iv4 ^ inbytes
    nano_xor_iv6 = 0xe07c265404be4294UL,  // iv6 ^ ~0
};

// MSL 64-bit rotate isn't supported on all targets
static inline uint64_t rotr64( const uint64_t val, const unsigned amount)
{
    return ( val >> amount ) | ( val << ( 64 - amount ) );
}

#define G32(m0, m1, m2, m3, vva, vb1, vb2, vvc, vd1, vd2) \
    do {                                                  \
        vva += ulong2(vb1 + m0, vb2 + m2);                \
        vd1 = rotr64(vd1 ^ vva.x, 32ULL);                 \
        vd2 = rotr64(vd2 ^ vva.y, 32ULL);                 \
        vvc += ulong2(vd1, vd2);                          \
        vb1 = rotr64(vb1 ^ vvc.x, 24ULL);                 \
        vb2 = rotr64(vb2 ^ vvc.y, 24ULL);                 \
        vva += ulong2(vb1 + m1, vb2 + m3);                \
        vd1 = rotr64(vd1 ^ vva.x, 16ULL);                 \
        vd2 = rotr64(vd2 ^ vva.y, 16ULL);                 \
        vvc += ulong2(vd1, vd2);                          \
        vb1 = rotr64(vb1 ^ vvc.x, 63ULL);                 \
        vb2 = rotr64(vb2 ^ vvc.y, 63ULL);                 \
    } while (0)

#define G2v(m0, m1, m2, m3, a, b, c, d)                                   \
    G32(m0, m1, m2, m3, vv[a / 2], vv[b / 2].x, vv[b / 2].y, vv[c / 2],   \
        vv[d / 2].x, vv[d / 2].y)

#define G2v_split(m0, m1, m2, m3, a, vb1, vb2, c, vd1, vd2) \
    G32(m0, m1, m2, m3, vv[a / 2], vb1, vb2, vv[c / 2], vd1, vd2)

#define ROUND(m0, m1, m2, m3, m4, m5, m6, m7, m8, m9, m10, m11, m12, m13, m14, \
              m15)                                                             \
    do {                                                                       \
        G2v(m0, m1, m2, m3, 0, 4, 8, 12);                                      \
        G2v(m4, m5, m6, m7, 2, 6, 10, 14);                                     \
        G2v_split(m8, m9, m10, m11, 0, vv[5 / 2].y, vv[6 / 2].x, 10,           \
                  vv[15 / 2].y, vv[12 / 2].x);                                 \
        G2v_split(m12, m13, m14, m15, 2, vv[7 / 2].y, vv[4 / 2].x, 8,          \
                  vv[13 / 2].y, vv[14 / 2].x);                                 \
    } while (0)


static inline ulong blake2b(uint64_t const nonce, constant ulong* h)
{
    ulong2 vv[8] = {
        {nano_xor_iv0, iv1}, {iv2, iv3},          {iv4, iv5},
        {iv6, iv7},          {iv0, iv1},          {iv2, iv3},
        {nano_xor_iv4, iv5}, {nano_xor_iv6, iv7},
    };

    ROUND(nonce, h[0], h[1], h[2], h[3], 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    ROUND(0, 0, h[3], 0, 0, 0, 0, 0, h[0], 0, nonce, h[1], 0, 0, 0, h[2]);
    ROUND(0, 0, 0, nonce, 0, h[1], 0, 0, 0, 0, h[2], 0, 0, h[0], 0, h[3]);
    ROUND(0, 0, h[2], h[0], 0, 0, 0, 0, h[1], 0, 0, 0, h[3], nonce, 0, 0);
    ROUND(0, nonce, 0, 0, h[1], h[3], 0, 0, 0, h[0], 0, 0, 0, 0, h[2], 0);
    ROUND(h[1], 0, 0, 0, nonce, 0, 0, h[2], h[3], 0, 0, 0, 0, 0, h[0], 0);
    ROUND(0, 0, h[0], 0, 0, 0, h[3], 0, nonce, 0, 0, h[2], 0, h[1], 0, 0);
    ROUND(0, 0, 0, 0, 0, h[0], h[2], 0, 0, nonce, 0, h[3], 0, 0, h[1], 0);
    ROUND(0, 0, 0, 0, 0, h[2], nonce, 0, 0, h[1], 0, 0, h[0], h[3], 0, 0);
    ROUND(0, h[1], 0, h[3], 0, 0, h[0], 0, 0, 0, 0, 0, h[2], 0, 0, nonce);
    ROUND(nonce, h[0], h[1], h[2], h[3], 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    ROUND(0, 0, h[3], 0, 0, 0, 0, 0, h[0], 0, nonce, h[1], 0, 0, 0, h[2]);

    return nano_xor_iv0 ^ vv[0].x ^ vv[4].x;
}
#undef G32
#undef G2v
#undef G2v_split
#undef ROUND

static inline uint64_t to_u64(constant uint* lo_hi)
{
    return static_cast<uint64_t>(uint64_t(lo_hi[1])) << 32UL | lo_hi[0];
}

static inline void atomic_store_result(volatile device atomic_uint* out, uint64_t val, uint64_t difficulty)
{
    uint expected = 0;

    // MSL does not have a strong compare/exchange
    while (!atomic_compare_exchange_weak_explicit(&out[0], &expected, val & 0x00000000ffffffffULL, memory_order_relaxed, memory_order_relaxed)) {
        if (expected) {
            return;
        }
    }

    atomic_store_explicit(&out[1], static_cast<uint>(val >> 32), memory_order_relaxed);
    atomic_store_explicit(&out[2], static_cast<uint>(difficulty & 0x00000000ffffffffULL), memory_order_relaxed);
    atomic_store_explicit(&out[3], static_cast<uint>(difficulty >> 32), memory_order_relaxed);
}

/**
 * Attempt a nonce
 * @param random uin64_t: random number from the CPU side. The current thread index is added to form the nonce to attempt.
 * @param root 32 bytes: hash or account
 * @param difficulty uint64_t: the required difficulty
 * @param result uint64[2]: [0] is the nonce if a solution is found, [1] is the difficulty of the solution.
 */
kernel void blake2b_attempt(constant uint *random [[ buffer(0) ]],
                            constant uchar *root [[ buffer(1) ]],
                            constant uint *difficulty [[ buffer(2) ]],
                            volatile device atomic_uint *result [[ buffer(3) ]],
                            uint index [[ thread_position_in_grid ]])
{
    uint64_t nonce = uint64_t(index) + to_u64(random);
    uint64_t actual_difficulty = blake2b(nonce, reinterpret_cast<constant ulong *> (root));
    if (actual_difficulty >= to_u64(difficulty))
    {
        atomic_store_result(result, nonce, actual_difficulty);
    }
}
