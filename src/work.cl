enum Blake2b_IV
{
  iv0 = 0x6a09e667f3bcc908UL,
  iv1 = 0xbb67ae8584caa73bUL,
  iv2 = 0x3c6ef372fe94f82bUL,
  iv3 = 0xa54ff53a5f1d36f1UL,
  iv4 = 0x510e527fade682d1UL,
  iv5 = 0x9b05688c2b3e6c1fUL,
  iv6 = 0x1f83d9abfb41bd6bUL,
  iv7 = 0x5be0cd19137e2179UL,
  nano_xor_iv0 = 0x6a09e667f2bdc900, // iv1 ^ 0x1010000 ^ outlen
  nano_xor_iv4 = 0x510e527fade682f9UL, // iv4 ^ inbytes
  nano_xor_iv6 = 0xe07c265404be4294, // iv6 ^ ~0
};
static inline ulong rotr64(ulong a, ulong shift) { return rotate(a, 64 - shift); }
#define G32(m0, m1, m2, m3, vva, vb1, vb2, vvc, vd1, vd2) \
  do {                                                    \
    vva += (ulong2) (vb1 + m0, vb2 + m2);                 \
    vd1 = rotr64(vd1 ^ vva.s0, 32UL);                     \
    vd2 = rotr64(vd2 ^ vva.s1, 32UL);                     \
    vvc += (ulong2) (vd1, vd2);                           \
    vb1 = rotr64(vb1 ^ vvc.s0, 24UL);                     \
    vb2 = rotr64(vb2 ^ vvc.s1, 24UL);                     \
    vva += (ulong2) (vb1 + m1, vb2 + m3);                 \
    vd1 = rotr64(vd1 ^ vva.s0, 16UL);                     \
    vd2 = rotr64(vd2 ^ vva.s1, 16UL);                     \
    vvc += (ulong2) (vd1, vd2);                           \
    vb1 = rotr64(vb1 ^ vvc.s0, 63UL);                     \
    vb2 = rotr64(vb2 ^ vvc.s1, 63UL);                     \
  } while (0)
#define G2v(m0, m1, m2, m3, a, b, c, d) \
  G32(m0, m1, m2, m3, vv[a/2], vv[b/2].s0, vv[b/2].s1, vv[c/2], vv[d/2].s0, vv[d/2].s1)
#define G2v_split(m0, m1, m2, m3, a, vb1, vb2, c, vd1, vd2) \
  G32(m0, m1, m2, m3, vv[a/2], vb1, vb2, vv[c/2], vd1, vd2)
#define ROUND(m0, m1, m2, m3, m4, m5, m6, m7, m8, m9, m10, m11, m12, m13, m14, m15)         \
  do {                                                                                      \
    G2v     (m0, m1, m2, m3, 0, 4,  8, 12);                                                 \
    G2v     (m4, m5, m6, m7, 2, 6, 10, 14);                                                 \
    G2v_split(m8, m9, m10, m11, 0, vv[5/2].s1, vv[6/2].s0, 10, vv[15/2].s1, vv[12/2].s0);   \
    G2v_split(m12, m13, m14, m15, 2, vv[7/2].s1, vv[4/2].s0,  8, vv[13/2].s1, vv[14/2].s0); \
  } while(0)
static inline ulong blake2b(ulong const nonce, __global ulong const * hash)
{
  ulong2 vv[8] = {
    { nano_xor_iv0, iv1 },
    { iv2, iv3 },
    { iv4, iv5 },
    { iv6, iv7 },
    { iv0, iv1 },
    { iv2, iv3 },
    { nano_xor_iv4, iv5 },
    { nano_xor_iv6, iv7 },
  };
  ROUND(nonce, hash[0], hash[1], hash[2], hash[3], 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
  ROUND(0, 0, hash[3], 0, 0, 0, 0, 0, hash[0], 0, nonce, hash[1], 0, 0, 0, hash[2]);
  ROUND(0, 0, 0, nonce, 0, hash[1], 0, 0, 0, 0, hash[2], 0, 0, hash[0], 0, hash[3]);
  ROUND(0, 0, hash[2], hash[0], 0, 0, 0, 0, hash[1], 0, 0, 0, hash[3], nonce, 0, 0);
  ROUND(0, nonce, 0, 0, hash[1], hash[3], 0, 0, 0, hash[0], 0, 0, 0, 0, hash[2], 0);
  ROUND(hash[1], 0, 0, 0, nonce, 0, 0, hash[2], hash[3], 0, 0, 0, 0, 0, hash[0], 0);
  ROUND(0, 0, hash[0], 0, 0, 0, hash[3], 0, nonce, 0, 0, hash[2], 0, hash[1], 0, 0);
  ROUND(0, 0, 0, 0, 0, hash[0], hash[2], 0, 0, nonce, 0, hash[3], 0, 0, hash[1], 0);
  ROUND(0, 0, 0, 0, 0, hash[2], nonce, 0, 0, hash[1], 0, 0, hash[0], hash[3], 0, 0);
  ROUND(0, hash[1], 0, hash[3], 0, 0, hash[0], 0, 0, 0, 0, 0, hash[2], 0, 0, nonce);
  ROUND(nonce, hash[0], hash[1], hash[2], hash[3], 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
  ROUND(0, 0, hash[3], 0, 0, 0, 0, 0, hash[0], 0, nonce, hash[1], 0, 0, 0, hash[2]);
  return nano_xor_iv0 ^ vv[0].s0 ^ vv[4].s0;
}
#undef G32
#undef G2v
#undef G2v_split
#undef ROUND
__kernel void nano_work (__global uchar const * attempt, __global uchar * result_a, __global uchar const * item_a, ulong const difficulty)
{
  int const thread = get_global_id (0);
  ulong attempt_l = *(__global ulong const *)attempt + thread;
  ulong result = blake2b(attempt_l, item_a);
  if (result >= difficulty)
  {
    *(__global ulong *)result_a = attempt_l;
  }
}
