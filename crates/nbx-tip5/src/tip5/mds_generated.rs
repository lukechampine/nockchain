use std::simd::*;
// ===== SCALAR VERSION =====
#[allow(unused_parens)]
pub const fn generated(input: &[u32; 16]) -> [u64; 16] {
    // layer 0
    let n_160 = (input[0] as u64).wrapping_sub((input[8] as u64));
    let n_161 = (input[1] as u64).wrapping_sub((input[9] as u64));
    let n_162 = (input[2] as u64).wrapping_sub((input[10] as u64));
    let n_163 = (input[3] as u64).wrapping_sub((input[11] as u64));
    let n_164 = (input[4] as u64).wrapping_sub((input[12] as u64));
    let n_165 = (input[5] as u64).wrapping_sub((input[13] as u64));
    let n_166 = (input[6] as u64).wrapping_sub((input[14] as u64));
    let n_167 = (input[7] as u64).wrapping_sub((input[15] as u64));
    // layer 1
    let n_176 = n_160.wrapping_add(n_164);
    let n_177 = n_161.wrapping_add(n_165);
    let n_178 = n_162.wrapping_add(n_166);
    let n_179 = n_163.wrapping_add(n_167);
    // layer 2
    let n_34 = (input[0] as u64).wrapping_add((input[8] as u64));
    let n_35 = (input[1] as u64).wrapping_add((input[9] as u64));
    let n_36 = (input[2] as u64).wrapping_add((input[10] as u64));
    let n_37 = (input[3] as u64).wrapping_add((input[11] as u64));
    let n_38 = (input[4] as u64).wrapping_add((input[12] as u64));
    let n_39 = (input[5] as u64).wrapping_add((input[13] as u64));
    let n_40 = (input[6] as u64).wrapping_add((input[14] as u64));
    let n_41 = (input[7] as u64).wrapping_add((input[15] as u64));
    let n_185 = n_161.wrapping_add(n_163);
    let n_270 = n_176.wrapping_add(n_178);
    let n_271 = n_177.wrapping_add(n_179);
    // layer 3
    let n_90 = n_34.wrapping_sub(n_38);
    let n_91 = n_35.wrapping_sub(n_39);
    let n_92 = n_36.wrapping_sub(n_40);
    let n_93 = n_37.wrapping_sub(n_41);
    let n_184 = n_160.wrapping_add(n_162);
    let n_228 = n_165.wrapping_add(n_167);
    let n_1885 = n_271.wrapping_mul(0xfffffffffff931e8u64);
    let n_1891 = n_177.wrapping_mul(0xfffffffffffac4b0u64);
    let n_1909 = n_185.wrapping_mul(0xfffffffffffbe968u64);
    let n_1915 = n_161.wrapping_mul(0xfffffffffffcc698u64);
    let n_2047 = n_270.wrapping_mul(0x1c070u64);
    // layer 4
    let n_50 = n_34.wrapping_add(n_38);
    let n_51 = n_35.wrapping_add(n_39);
    let n_52 = n_36.wrapping_add(n_40);
    let n_53 = n_37.wrapping_add(n_41);
    let n_98 = n_90.wrapping_add(n_92);
    let n_99 = n_91.wrapping_add(n_93);
    let n_227 = n_164.wrapping_add(n_166);
    let n_651 = n_1885.wrapping_sub(n_1891);
    let n_1360 = n_1909.wrapping_sub(n_1915);
    let n_1630 = n_1891.wrapping_add(n_2047);
    let n_1897 = n_179.wrapping_mul(0xfffffffffffe6d38u64);
    let n_1921 = n_163.wrapping_mul(0xffffffffffff22d0u64);
    let n_1933 = n_228.wrapping_mul(0xfffffffffffd4880u64);
    let n_1939 = n_165.wrapping_mul(0xfffffffffffdfe18u64);
    let n_2007 = n_184.wrapping_mul(0xffffffffffff0150u64);
    let n_2035 = n_176.wrapping_mul(0xfffffffffffffc60u64);
    // layer 5
    let n_58 = n_50.wrapping_add(n_52);
    let n_59 = n_51.wrapping_add(n_53);
    let n_609 = n_651.wrapping_sub(n_1897);
    let n_1321 = n_1933.wrapping_sub(n_1939);
    let n_1351 = n_1360.wrapping_sub(n_1921);
    let n_1606 = n_1630.wrapping_sub(n_2035);
    let n_1615 = n_1915.wrapping_add(n_2007);
    let n_1861 = n_99.wrapping_mul(0xfffffffffffe33b4u64);
    let n_1865 = n_91.wrapping_mul(0xfffffffffffb7700u64);
    let n_1879 = n_160.wrapping_mul(0x8b18u64);
    let n_1903 = n_178.wrapping_mul(0x1c410u64);
    let n_1927 = n_162.wrapping_mul(0xfffffffffffe7638u64);
    let n_1945 = n_167.wrapping_mul(0xffffffffffff4a68u64);
    let n_1970 = n_160.wrapping_mul(0xfffffffffffcc698u64);
    let n_1973 = n_161.wrapping_mul(0x8b18u64);
    let n_1976 = n_178.wrapping_mul(0xfffffffffffe6d38u64);
    let n_1979 = n_179.wrapping_mul(0x1c410u64);
    let n_1982 = n_162.wrapping_mul(0xffffffffffff22d0u64);
    let n_1985 = n_163.wrapping_mul(0xfffffffffffe7638u64);
    let n_2001 = n_98.wrapping_mul(0x563f0u64);
    let n_2013 = n_227.wrapping_mul(0x2bf20u64);
    let n_2020 = n_184.wrapping_mul(0xfffffffffffbe968u64);
    let n_2023 = n_185.wrapping_mul(0xffffffffffff0150u64);
    let n_2038 = n_176.wrapping_mul(0xfffffffffffac4b0u64);
    let n_2041 = n_177.wrapping_mul(0xfffffffffffffc60u64);
    let n_2050 = n_270.wrapping_mul(0xfffffffffff931e8u64);
    let n_2053 = n_271.wrapping_mul(0x1c070u64);
    // layer 6
    let n_62 = n_58.wrapping_add(n_59);
    let n_65 = n_58.wrapping_sub(n_59);
    let n_71 = n_50.wrapping_sub(n_52);
    let n_72 = n_51.wrapping_sub(n_53);
    let n_485 = n_1861.wrapping_sub(n_1865);
    let n_570 = n_609.wrapping_add(n_1903);
    let n_970 = n_1865.wrapping_add(n_2001);
    let n_1300 = n_1321.wrapping_sub(n_1945);
    let n_1330 = n_1351.wrapping_add(n_1927);
    let n_1573 = n_1606.wrapping_sub(n_1903);
    let n_1582 = n_1615.wrapping_sub(n_1879);
    let n_1591 = n_1939.wrapping_add(n_2013);
    let n_1729 = n_1982.wrapping_add(n_1985);
    let n_1756 = n_1976.wrapping_add(n_1979);
    let n_1768 = n_1970.wrapping_add(n_1973);
    let n_1786 = n_2038.wrapping_add(n_2041);
    let n_1792 = n_2020.wrapping_add(n_2023);
    let n_1801 = n_2050.wrapping_add(n_2053);
    let n_1857 = n_90.wrapping_mul(0x608f8u64);
    let n_1869 = n_93.wrapping_mul(0x2bcb4u64);
    let n_1951 = n_166.wrapping_mul(0x34dd8u64);
    let n_1957 = n_164.wrapping_mul(0xffffffffffff7148u64);
    let n_1961 = n_90.wrapping_mul(0xfffffffffffb7700u64);
    let n_1963 = n_91.wrapping_mul(0x608f8u64);
    let n_1988 = n_166.wrapping_mul(0xffffffffffff4a68u64);
    let n_1991 = n_167.wrapping_mul(0x34dd8u64);
    let n_1994 = n_164.wrapping_mul(0xfffffffffffdfe18u64);
    let n_1997 = n_165.wrapping_mul(0xffffffffffff7148u64);
    let n_2015 = n_98.wrapping_mul(0xfffffffffffe33b4u64);
    let n_2017 = n_99.wrapping_mul(0x563f0u64);
    let n_2026 = n_227.wrapping_mul(0xfffffffffffd4880u64);
    let n_2029 = n_228.wrapping_mul(0x2bf20u64);
    // layer 7
    let n_64 = n_62.wrapping_mul(0x801d5u64);
    let n_67 = n_65.wrapping_mul(0xcccbu64);
    let n_454 = n_485.wrapping_sub(n_1869);
    let n_504 = n_570.wrapping_sub(n_1330);
    let n_801 = n_1756.wrapping_sub(n_1729);
    let n_937 = n_970.wrapping_sub(n_1857);
    let n_988 = n_1897.wrapping_sub(n_1921);
    let n_1116 = n_1961.wrapping_add(n_1963);
    let n_1140 = n_2015.wrapping_add(n_2017);
    let n_1276 = n_1300.wrapping_add(n_1951);
    let n_1285 = n_1330.wrapping_add(n_2035);
    let n_1420 = n_1729.wrapping_add(n_1786);
    let n_1540 = n_1921.wrapping_add(n_1573);
    let n_1549 = n_1582.wrapping_sub(n_1927);
    let n_1558 = n_1591.wrapping_sub(n_1957);
    let n_1705 = n_1988.wrapping_add(n_1991);
    let n_1723 = n_1792.wrapping_sub(n_1768);
    let n_1741 = n_1994.wrapping_add(n_1997);
    let n_1750 = n_1801.wrapping_sub(n_1786);
    let n_1774 = n_2026.wrapping_add(n_2029);
    let n_1851 = n_71.wrapping_mul(0xffffffffffff9af0u64);
    let n_1853 = n_72.wrapping_mul(0xd29eu64);
    let n_1873 = n_92.wrapping_mul(0xffffffffffff5af8u64);
    let n_1958 = n_71.wrapping_mul(0xd29eu64);
    let n_1959 = n_72.wrapping_mul(0xffffffffffff9af0u64);
    let n_1965 = n_92.wrapping_mul(0x2bcb4u64);
    let n_1967 = n_93.wrapping_mul(0xffffffffffff5af8u64);
    // layer 8
    let n_69 = n_64.wrapping_add(n_67);
    let n_70 = n_64.wrapping_sub(n_67);
    let n_397 = n_1851.wrapping_sub(n_1853);
    let n_428 = n_454.wrapping_add(n_1873);
    let n_473 = n_504.wrapping_sub(n_1276);
    let n_702 = n_1958.wrapping_add(n_1959);
    let n_769 = n_801.wrapping_sub(n_1705);
    let n_912 = n_937.wrapping_sub(n_1873);
    let n_955 = n_988.wrapping_sub(n_1945);
    let n_1092 = n_1140.wrapping_sub(n_1116);
    let n_1096 = n_1965.wrapping_add(n_1967);
    let n_1261 = n_1285.wrapping_sub(n_1879);
    let n_1399 = n_1420.wrapping_sub(n_1768);
    let n_1516 = n_1540.wrapping_sub(n_1549);
    let n_1525 = n_1558.wrapping_sub(n_1951);
    let n_1690 = n_1723.wrapping_sub(n_1729);
    let n_1699 = n_1774.wrapping_sub(n_1741);
    let n_1714 = n_1750.wrapping_sub(n_1756);
    // layer 9
    let n_86 = n_69.wrapping_add(n_397);
    let n_87 = n_69.wrapping_sub(n_397);
    let n_88 = n_70.wrapping_add(n_702);
    let n_89 = n_70.wrapping_sub(n_702);
    let n_403 = n_1857.wrapping_sub(n_428);
    let n_444 = n_473.wrapping_add(n_1957);
    let n_708 = n_1116.wrapping_sub(n_1096);
    let n_742 = n_769.wrapping_add(n_1741);
    let n_897 = n_912.wrapping_sub(n_1869);
    let n_931 = n_955.wrapping_add(n_1525);
    let n_1077 = n_1092.wrapping_sub(n_1096);
    let n_1246 = n_1261.wrapping_sub(n_1957);
    let n_1384 = n_1399.wrapping_sub(n_1741);
    let n_1501 = n_1516.wrapping_sub(n_1525);
    let n_1666 = n_1714.wrapping_sub(n_1690);
    let n_1675 = n_1699.wrapping_sub(n_1705);
    // layer 10
    let n_152 = n_86.wrapping_add(n_403);
    let n_153 = n_86.wrapping_sub(n_403);
    let n_154 = n_88.wrapping_add(n_708);
    let n_155 = n_88.wrapping_sub(n_708);
    let n_156 = n_87.wrapping_add(n_897);
    let n_157 = n_87.wrapping_sub(n_897);
    let n_158 = n_89.wrapping_add(n_1077);
    let n_159 = n_89.wrapping_sub(n_1077);
    let n_412 = n_1879.wrapping_sub(n_444);
    let n_717 = n_1768.wrapping_sub(n_742);
    let n_906 = n_1549.wrapping_sub(n_931);
    let n_1086 = n_1690.wrapping_sub(n_1675);
    let n_1237 = n_1246.wrapping_sub(n_1276);
    let n_1375 = n_1384.wrapping_sub(n_1705);
    let n_1492 = n_1501.wrapping_sub(n_1945);
    let n_1657 = n_1666.wrapping_sub(n_1675);
    // output
    [
        n_152.wrapping_add(n_412),
        n_154.wrapping_add(n_717),
        n_156.wrapping_add(n_906),
        n_158.wrapping_add(n_1086),
        n_153.wrapping_add(n_1237),
        n_155.wrapping_add(n_1375),
        n_157.wrapping_add(n_1492),
        n_159.wrapping_add(n_1657),
        n_152.wrapping_sub(n_412),
        n_154.wrapping_sub(n_717),
        n_156.wrapping_sub(n_906),
        n_158.wrapping_sub(n_1086),
        n_153.wrapping_sub(n_1237),
        n_155.wrapping_sub(n_1375),
        n_157.wrapping_sub(n_1492),
        n_159.wrapping_sub(n_1657),
    ]
}

// ===== SIMD VERSION (x2 lanes) =====

#[allow(unused_parens)]
#[rustfmt::skip]
pub fn generated_simd_x2(input: &[[u32; 16]; 2]) -> [u64x2; 16] {
    // Load all inputs into SIMD registers
    let input_0_simd = u64x2::from_array([input[0][0] as u64, input[1][0] as u64]);
    let input_1_simd = u64x2::from_array([input[0][1] as u64, input[1][1] as u64]);
    let input_2_simd = u64x2::from_array([input[0][2] as u64, input[1][2] as u64]);
    let input_3_simd = u64x2::from_array([input[0][3] as u64, input[1][3] as u64]);
    let input_4_simd = u64x2::from_array([input[0][4] as u64, input[1][4] as u64]);
    let input_5_simd = u64x2::from_array([input[0][5] as u64, input[1][5] as u64]);
    let input_6_simd = u64x2::from_array([input[0][6] as u64, input[1][6] as u64]);
    let input_7_simd = u64x2::from_array([input[0][7] as u64, input[1][7] as u64]);
    let input_8_simd = u64x2::from_array([input[0][8] as u64, input[1][8] as u64]);
    let input_9_simd = u64x2::from_array([input[0][9] as u64, input[1][9] as u64]);
    let input_10_simd = u64x2::from_array([input[0][10] as u64, input[1][10] as u64]);
    let input_11_simd = u64x2::from_array([input[0][11] as u64, input[1][11] as u64]);
    let input_12_simd = u64x2::from_array([input[0][12] as u64, input[1][12] as u64]);
    let input_13_simd = u64x2::from_array([input[0][13] as u64, input[1][13] as u64]);
    let input_14_simd = u64x2::from_array([input[0][14] as u64, input[1][14] as u64]);
    let input_15_simd = u64x2::from_array([input[0][15] as u64, input[1][15] as u64]);

    // layer 0
    let n_160_simd = (input_0_simd - input_8_simd);
    let n_161_simd = (input_1_simd - input_9_simd);
    let n_162_simd = (input_2_simd - input_10_simd);
    let n_163_simd = (input_3_simd - input_11_simd);
    let n_164_simd = (input_4_simd - input_12_simd);
    let n_165_simd = (input_5_simd - input_13_simd);
    let n_166_simd = (input_6_simd - input_14_simd);
    let n_167_simd = (input_7_simd - input_15_simd);
    // layer 1
    let n_176_simd = (n_160_simd + n_164_simd);
    let n_177_simd = (n_161_simd + n_165_simd);
    let n_178_simd = (n_162_simd + n_166_simd);
    let n_179_simd = (n_163_simd + n_167_simd);
    // layer 2
    let n_34_simd = (input_0_simd + input_8_simd);
    let n_35_simd = (input_1_simd + input_9_simd);
    let n_36_simd = (input_2_simd + input_10_simd);
    let n_37_simd = (input_3_simd + input_11_simd);
    let n_38_simd = (input_4_simd + input_12_simd);
    let n_39_simd = (input_5_simd + input_13_simd);
    let n_40_simd = (input_6_simd + input_14_simd);
    let n_41_simd = (input_7_simd + input_15_simd);
    let n_185_simd = (n_161_simd + n_163_simd);
    let n_270_simd = (n_176_simd + n_178_simd);
    let n_271_simd = (n_177_simd + n_179_simd);
    // layer 3
    let n_90_simd = (n_34_simd - n_38_simd);
    let n_91_simd = (n_35_simd - n_39_simd);
    let n_92_simd = (n_36_simd - n_40_simd);
    let n_93_simd = (n_37_simd - n_41_simd);
    let n_184_simd = (n_160_simd + n_162_simd);
    let n_228_simd = (n_165_simd + n_167_simd);
    let n_1885_simd = (n_271_simd * u64x2::splat(0xfffffffffff931e8u64));
    let n_1891_simd = (n_177_simd * u64x2::splat(0xfffffffffffac4b0u64));
    let n_1909_simd = (n_185_simd * u64x2::splat(0xfffffffffffbe968u64));
    let n_1915_simd = (n_161_simd * u64x2::splat(0xfffffffffffcc698u64));
    let n_2047_simd = (n_270_simd * u64x2::splat(0x1c070u64));
    // layer 4
    let n_50_simd = (n_34_simd + n_38_simd);
    let n_51_simd = (n_35_simd + n_39_simd);
    let n_52_simd = (n_36_simd + n_40_simd);
    let n_53_simd = (n_37_simd + n_41_simd);
    let n_98_simd = (n_90_simd + n_92_simd);
    let n_99_simd = (n_91_simd + n_93_simd);
    let n_227_simd = (n_164_simd + n_166_simd);
    let n_651_simd = (n_1885_simd - n_1891_simd);
    let n_1360_simd = (n_1909_simd - n_1915_simd);
    let n_1630_simd = (n_1891_simd + n_2047_simd);
    let n_1897_simd = (n_179_simd * u64x2::splat(0xfffffffffffe6d38u64));
    let n_1921_simd = (n_163_simd * u64x2::splat(0xffffffffffff22d0u64));
    let n_1933_simd = (n_228_simd * u64x2::splat(0xfffffffffffd4880u64));
    let n_1939_simd = (n_165_simd * u64x2::splat(0xfffffffffffdfe18u64));
    let n_2007_simd = (n_184_simd * u64x2::splat(0xffffffffffff0150u64));
    let n_2035_simd = (n_176_simd * u64x2::splat(0xfffffffffffffc60u64));
    // layer 5
    let n_58_simd = (n_50_simd + n_52_simd);
    let n_59_simd = (n_51_simd + n_53_simd);
    let n_609_simd = (n_651_simd - n_1897_simd);
    let n_1321_simd = (n_1933_simd - n_1939_simd);
    let n_1351_simd = (n_1360_simd - n_1921_simd);
    let n_1606_simd = (n_1630_simd - n_2035_simd);
    let n_1615_simd = (n_1915_simd + n_2007_simd);
    let n_1861_simd = (n_99_simd * u64x2::splat(0xfffffffffffe33b4u64));
    let n_1865_simd = (n_91_simd * u64x2::splat(0xfffffffffffb7700u64));
    let n_1879_simd = (n_160_simd * u64x2::splat(0x8b18u64));
    let n_1903_simd = (n_178_simd * u64x2::splat(0x1c410u64));
    let n_1927_simd = (n_162_simd * u64x2::splat(0xfffffffffffe7638u64));
    let n_1945_simd = (n_167_simd * u64x2::splat(0xffffffffffff4a68u64));
    let n_1970_simd = (n_160_simd * u64x2::splat(0xfffffffffffcc698u64));
    let n_1973_simd = (n_161_simd * u64x2::splat(0x8b18u64));
    let n_1976_simd = (n_178_simd * u64x2::splat(0xfffffffffffe6d38u64));
    let n_1979_simd = (n_179_simd * u64x2::splat(0x1c410u64));
    let n_1982_simd = (n_162_simd * u64x2::splat(0xffffffffffff22d0u64));
    let n_1985_simd = (n_163_simd * u64x2::splat(0xfffffffffffe7638u64));
    let n_2001_simd = (n_98_simd * u64x2::splat(0x563f0u64));
    let n_2013_simd = (n_227_simd * u64x2::splat(0x2bf20u64));
    let n_2020_simd = (n_184_simd * u64x2::splat(0xfffffffffffbe968u64));
    let n_2023_simd = (n_185_simd * u64x2::splat(0xffffffffffff0150u64));
    let n_2038_simd = (n_176_simd * u64x2::splat(0xfffffffffffac4b0u64));
    let n_2041_simd = (n_177_simd * u64x2::splat(0xfffffffffffffc60u64));
    let n_2050_simd = (n_270_simd * u64x2::splat(0xfffffffffff931e8u64));
    let n_2053_simd = (n_271_simd * u64x2::splat(0x1c070u64));
    // layer 6
    let n_62_simd = (n_58_simd + n_59_simd);
    let n_65_simd = (n_58_simd - n_59_simd);
    let n_71_simd = (n_50_simd - n_52_simd);
    let n_72_simd = (n_51_simd - n_53_simd);
    let n_485_simd = (n_1861_simd - n_1865_simd);
    let n_570_simd = (n_609_simd + n_1903_simd);
    let n_970_simd = (n_1865_simd + n_2001_simd);
    let n_1300_simd = (n_1321_simd - n_1945_simd);
    let n_1330_simd = (n_1351_simd + n_1927_simd);
    let n_1573_simd = (n_1606_simd - n_1903_simd);
    let n_1582_simd = (n_1615_simd - n_1879_simd);
    let n_1591_simd = (n_1939_simd + n_2013_simd);
    let n_1729_simd = (n_1982_simd + n_1985_simd);
    let n_1756_simd = (n_1976_simd + n_1979_simd);
    let n_1768_simd = (n_1970_simd + n_1973_simd);
    let n_1786_simd = (n_2038_simd + n_2041_simd);
    let n_1792_simd = (n_2020_simd + n_2023_simd);
    let n_1801_simd = (n_2050_simd + n_2053_simd);
    let n_1857_simd = (n_90_simd * u64x2::splat(0x608f8u64));
    let n_1869_simd = (n_93_simd * u64x2::splat(0x2bcb4u64));
    let n_1951_simd = (n_166_simd * u64x2::splat(0x34dd8u64));
    let n_1957_simd = (n_164_simd * u64x2::splat(0xffffffffffff7148u64));
    let n_1961_simd = (n_90_simd * u64x2::splat(0xfffffffffffb7700u64));
    let n_1963_simd = (n_91_simd * u64x2::splat(0x608f8u64));
    let n_1988_simd = (n_166_simd * u64x2::splat(0xffffffffffff4a68u64));
    let n_1991_simd = (n_167_simd * u64x2::splat(0x34dd8u64));
    let n_1994_simd = (n_164_simd * u64x2::splat(0xfffffffffffdfe18u64));
    let n_1997_simd = (n_165_simd * u64x2::splat(0xffffffffffff7148u64));
    let n_2015_simd = (n_98_simd * u64x2::splat(0xfffffffffffe33b4u64));
    let n_2017_simd = (n_99_simd * u64x2::splat(0x563f0u64));
    let n_2026_simd = (n_227_simd * u64x2::splat(0xfffffffffffd4880u64));
    let n_2029_simd = (n_228_simd * u64x2::splat(0x2bf20u64));
    // layer 7
    let n_64_simd = (n_62_simd * u64x2::splat(0x801d5u64));
    let n_67_simd = (n_65_simd * u64x2::splat(0xcccbu64));
    let n_454_simd = (n_485_simd - n_1869_simd);
    let n_504_simd = (n_570_simd - n_1330_simd);
    let n_801_simd = (n_1756_simd - n_1729_simd);
    let n_937_simd = (n_970_simd - n_1857_simd);
    let n_988_simd = (n_1897_simd - n_1921_simd);
    let n_1116_simd = (n_1961_simd + n_1963_simd);
    let n_1140_simd = (n_2015_simd + n_2017_simd);
    let n_1276_simd = (n_1300_simd + n_1951_simd);
    let n_1285_simd = (n_1330_simd + n_2035_simd);
    let n_1420_simd = (n_1729_simd + n_1786_simd);
    let n_1540_simd = (n_1921_simd + n_1573_simd);
    let n_1549_simd = (n_1582_simd - n_1927_simd);
    let n_1558_simd = (n_1591_simd - n_1957_simd);
    let n_1705_simd = (n_1988_simd + n_1991_simd);
    let n_1723_simd = (n_1792_simd - n_1768_simd);
    let n_1741_simd = (n_1994_simd + n_1997_simd);
    let n_1750_simd = (n_1801_simd - n_1786_simd);
    let n_1774_simd = (n_2026_simd + n_2029_simd);
    let n_1851_simd = (n_71_simd * u64x2::splat(0xffffffffffff9af0u64));
    let n_1853_simd = (n_72_simd * u64x2::splat(0xd29eu64));
    let n_1873_simd = (n_92_simd * u64x2::splat(0xffffffffffff5af8u64));
    let n_1958_simd = (n_71_simd * u64x2::splat(0xd29eu64));
    let n_1959_simd = (n_72_simd * u64x2::splat(0xffffffffffff9af0u64));
    let n_1965_simd = (n_92_simd * u64x2::splat(0x2bcb4u64));
    let n_1967_simd = (n_93_simd * u64x2::splat(0xffffffffffff5af8u64));
    // layer 8
    let n_69_simd = (n_64_simd + n_67_simd);
    let n_70_simd = (n_64_simd - n_67_simd);
    let n_397_simd = (n_1851_simd - n_1853_simd);
    let n_428_simd = (n_454_simd + n_1873_simd);
    let n_473_simd = (n_504_simd - n_1276_simd);
    let n_702_simd = (n_1958_simd + n_1959_simd);
    let n_769_simd = (n_801_simd - n_1705_simd);
    let n_912_simd = (n_937_simd - n_1873_simd);
    let n_955_simd = (n_988_simd - n_1945_simd);
    let n_1092_simd = (n_1140_simd - n_1116_simd);
    let n_1096_simd = (n_1965_simd + n_1967_simd);
    let n_1261_simd = (n_1285_simd - n_1879_simd);
    let n_1399_simd = (n_1420_simd - n_1768_simd);
    let n_1516_simd = (n_1540_simd - n_1549_simd);
    let n_1525_simd = (n_1558_simd - n_1951_simd);
    let n_1690_simd = (n_1723_simd - n_1729_simd);
    let n_1699_simd = (n_1774_simd - n_1741_simd);
    let n_1714_simd = (n_1750_simd - n_1756_simd);
    // layer 9
    let n_86_simd = (n_69_simd + n_397_simd);
    let n_87_simd = (n_69_simd - n_397_simd);
    let n_88_simd = (n_70_simd + n_702_simd);
    let n_89_simd = (n_70_simd - n_702_simd);
    let n_403_simd = (n_1857_simd - n_428_simd);
    let n_444_simd = (n_473_simd + n_1957_simd);
    let n_708_simd = (n_1116_simd - n_1096_simd);
    let n_742_simd = (n_769_simd + n_1741_simd);
    let n_897_simd = (n_912_simd - n_1869_simd);
    let n_931_simd = (n_955_simd + n_1525_simd);
    let n_1077_simd = (n_1092_simd - n_1096_simd);
    let n_1246_simd = (n_1261_simd - n_1957_simd);
    let n_1384_simd = (n_1399_simd - n_1741_simd);
    let n_1501_simd = (n_1516_simd - n_1525_simd);
    let n_1666_simd = (n_1714_simd - n_1690_simd);
    let n_1675_simd = (n_1699_simd - n_1705_simd);
    // layer 10
    let n_152_simd = (n_86_simd + n_403_simd);
    let n_153_simd = (n_86_simd - n_403_simd);
    let n_154_simd = (n_88_simd + n_708_simd);
    let n_155_simd = (n_88_simd - n_708_simd);
    let n_156_simd = (n_87_simd + n_897_simd);
    let n_157_simd = (n_87_simd - n_897_simd);
    let n_158_simd = (n_89_simd + n_1077_simd);
    let n_159_simd = (n_89_simd - n_1077_simd);
    let n_412_simd = (n_1879_simd - n_444_simd);
    let n_717_simd = (n_1768_simd - n_742_simd);
    let n_906_simd = (n_1549_simd - n_931_simd);
    let n_1086_simd = (n_1690_simd - n_1675_simd);
    let n_1237_simd = (n_1246_simd - n_1276_simd);
    let n_1375_simd = (n_1384_simd - n_1705_simd);
    let n_1492_simd = (n_1501_simd - n_1945_simd);
    let n_1657_simd = (n_1666_simd - n_1675_simd);
    // output
    [
        (n_152_simd + n_412_simd),
        (n_154_simd + n_717_simd),
        (n_156_simd + n_906_simd),
        (n_158_simd + n_1086_simd),
        (n_153_simd + n_1237_simd),
        (n_155_simd + n_1375_simd),
        (n_157_simd + n_1492_simd),
        (n_159_simd + n_1657_simd),
        (n_152_simd - n_412_simd),
        (n_154_simd - n_717_simd),
        (n_156_simd - n_906_simd),
        (n_158_simd - n_1086_simd),
        (n_153_simd - n_1237_simd),
        (n_155_simd - n_1375_simd),
        (n_157_simd - n_1492_simd),
        (n_159_simd - n_1657_simd),
    ]
}

// ===== SIMD VERSION (x4 lanes) =====

#[allow(unused_parens)]
#[rustfmt::skip]
pub fn generated_simd_x4(input: &[[u32; 16]; 4]) -> [u64x4; 16] {
    // Load all inputs into SIMD registers
    let input_0_simd = u64x4::from_array([input[0][0] as u64, input[1][0] as u64, input[2][0] as u64, input[3][0] as u64]);
    let input_1_simd = u64x4::from_array([input[0][1] as u64, input[1][1] as u64, input[2][1] as u64, input[3][1] as u64]);
    let input_2_simd = u64x4::from_array([input[0][2] as u64, input[1][2] as u64, input[2][2] as u64, input[3][2] as u64]);
    let input_3_simd = u64x4::from_array([input[0][3] as u64, input[1][3] as u64, input[2][3] as u64, input[3][3] as u64]);
    let input_4_simd = u64x4::from_array([input[0][4] as u64, input[1][4] as u64, input[2][4] as u64, input[3][4] as u64]);
    let input_5_simd = u64x4::from_array([input[0][5] as u64, input[1][5] as u64, input[2][5] as u64, input[3][5] as u64]);
    let input_6_simd = u64x4::from_array([input[0][6] as u64, input[1][6] as u64, input[2][6] as u64, input[3][6] as u64]);
    let input_7_simd = u64x4::from_array([input[0][7] as u64, input[1][7] as u64, input[2][7] as u64, input[3][7] as u64]);
    let input_8_simd = u64x4::from_array([input[0][8] as u64, input[1][8] as u64, input[2][8] as u64, input[3][8] as u64]);
    let input_9_simd = u64x4::from_array([input[0][9] as u64, input[1][9] as u64, input[2][9] as u64, input[3][9] as u64]);
    let input_10_simd = u64x4::from_array([input[0][10] as u64, input[1][10] as u64, input[2][10] as u64, input[3][10] as u64]);
    let input_11_simd = u64x4::from_array([input[0][11] as u64, input[1][11] as u64, input[2][11] as u64, input[3][11] as u64]);
    let input_12_simd = u64x4::from_array([input[0][12] as u64, input[1][12] as u64, input[2][12] as u64, input[3][12] as u64]);
    let input_13_simd = u64x4::from_array([input[0][13] as u64, input[1][13] as u64, input[2][13] as u64, input[3][13] as u64]);
    let input_14_simd = u64x4::from_array([input[0][14] as u64, input[1][14] as u64, input[2][14] as u64, input[3][14] as u64]);
    let input_15_simd = u64x4::from_array([input[0][15] as u64, input[1][15] as u64, input[2][15] as u64, input[3][15] as u64]);

    // layer 0
    let n_160_simd = (input_0_simd - input_8_simd);
    let n_161_simd = (input_1_simd - input_9_simd);
    let n_162_simd = (input_2_simd - input_10_simd);
    let n_163_simd = (input_3_simd - input_11_simd);
    let n_164_simd = (input_4_simd - input_12_simd);
    let n_165_simd = (input_5_simd - input_13_simd);
    let n_166_simd = (input_6_simd - input_14_simd);
    let n_167_simd = (input_7_simd - input_15_simd);
    // layer 1
    let n_176_simd = (n_160_simd + n_164_simd);
    let n_177_simd = (n_161_simd + n_165_simd);
    let n_178_simd = (n_162_simd + n_166_simd);
    let n_179_simd = (n_163_simd + n_167_simd);
    // layer 2
    let n_34_simd = (input_0_simd + input_8_simd);
    let n_35_simd = (input_1_simd + input_9_simd);
    let n_36_simd = (input_2_simd + input_10_simd);
    let n_37_simd = (input_3_simd + input_11_simd);
    let n_38_simd = (input_4_simd + input_12_simd);
    let n_39_simd = (input_5_simd + input_13_simd);
    let n_40_simd = (input_6_simd + input_14_simd);
    let n_41_simd = (input_7_simd + input_15_simd);
    let n_185_simd = (n_161_simd + n_163_simd);
    let n_270_simd = (n_176_simd + n_178_simd);
    let n_271_simd = (n_177_simd + n_179_simd);
    // layer 3
    let n_90_simd = (n_34_simd - n_38_simd);
    let n_91_simd = (n_35_simd - n_39_simd);
    let n_92_simd = (n_36_simd - n_40_simd);
    let n_93_simd = (n_37_simd - n_41_simd);
    let n_184_simd = (n_160_simd + n_162_simd);
    let n_228_simd = (n_165_simd + n_167_simd);
    let n_1885_simd = (n_271_simd * u64x4::splat(0xfffffffffff931e8u64));
    let n_1891_simd = (n_177_simd * u64x4::splat(0xfffffffffffac4b0u64));
    let n_1909_simd = (n_185_simd * u64x4::splat(0xfffffffffffbe968u64));
    let n_1915_simd = (n_161_simd * u64x4::splat(0xfffffffffffcc698u64));
    let n_2047_simd = (n_270_simd * u64x4::splat(0x1c070u64));
    // layer 4
    let n_50_simd = (n_34_simd + n_38_simd);
    let n_51_simd = (n_35_simd + n_39_simd);
    let n_52_simd = (n_36_simd + n_40_simd);
    let n_53_simd = (n_37_simd + n_41_simd);
    let n_98_simd = (n_90_simd + n_92_simd);
    let n_99_simd = (n_91_simd + n_93_simd);
    let n_227_simd = (n_164_simd + n_166_simd);
    let n_651_simd = (n_1885_simd - n_1891_simd);
    let n_1360_simd = (n_1909_simd - n_1915_simd);
    let n_1630_simd = (n_1891_simd + n_2047_simd);
    let n_1897_simd = (n_179_simd * u64x4::splat(0xfffffffffffe6d38u64));
    let n_1921_simd = (n_163_simd * u64x4::splat(0xffffffffffff22d0u64));
    let n_1933_simd = (n_228_simd * u64x4::splat(0xfffffffffffd4880u64));
    let n_1939_simd = (n_165_simd * u64x4::splat(0xfffffffffffdfe18u64));
    let n_2007_simd = (n_184_simd * u64x4::splat(0xffffffffffff0150u64));
    let n_2035_simd = (n_176_simd * u64x4::splat(0xfffffffffffffc60u64));
    // layer 5
    let n_58_simd = (n_50_simd + n_52_simd);
    let n_59_simd = (n_51_simd + n_53_simd);
    let n_609_simd = (n_651_simd - n_1897_simd);
    let n_1321_simd = (n_1933_simd - n_1939_simd);
    let n_1351_simd = (n_1360_simd - n_1921_simd);
    let n_1606_simd = (n_1630_simd - n_2035_simd);
    let n_1615_simd = (n_1915_simd + n_2007_simd);
    let n_1861_simd = (n_99_simd * u64x4::splat(0xfffffffffffe33b4u64));
    let n_1865_simd = (n_91_simd * u64x4::splat(0xfffffffffffb7700u64));
    let n_1879_simd = (n_160_simd * u64x4::splat(0x8b18u64));
    let n_1903_simd = (n_178_simd * u64x4::splat(0x1c410u64));
    let n_1927_simd = (n_162_simd * u64x4::splat(0xfffffffffffe7638u64));
    let n_1945_simd = (n_167_simd * u64x4::splat(0xffffffffffff4a68u64));
    let n_1970_simd = (n_160_simd * u64x4::splat(0xfffffffffffcc698u64));
    let n_1973_simd = (n_161_simd * u64x4::splat(0x8b18u64));
    let n_1976_simd = (n_178_simd * u64x4::splat(0xfffffffffffe6d38u64));
    let n_1979_simd = (n_179_simd * u64x4::splat(0x1c410u64));
    let n_1982_simd = (n_162_simd * u64x4::splat(0xffffffffffff22d0u64));
    let n_1985_simd = (n_163_simd * u64x4::splat(0xfffffffffffe7638u64));
    let n_2001_simd = (n_98_simd * u64x4::splat(0x563f0u64));
    let n_2013_simd = (n_227_simd * u64x4::splat(0x2bf20u64));
    let n_2020_simd = (n_184_simd * u64x4::splat(0xfffffffffffbe968u64));
    let n_2023_simd = (n_185_simd * u64x4::splat(0xffffffffffff0150u64));
    let n_2038_simd = (n_176_simd * u64x4::splat(0xfffffffffffac4b0u64));
    let n_2041_simd = (n_177_simd * u64x4::splat(0xfffffffffffffc60u64));
    let n_2050_simd = (n_270_simd * u64x4::splat(0xfffffffffff931e8u64));
    let n_2053_simd = (n_271_simd * u64x4::splat(0x1c070u64));
    // layer 6
    let n_62_simd = (n_58_simd + n_59_simd);
    let n_65_simd = (n_58_simd - n_59_simd);
    let n_71_simd = (n_50_simd - n_52_simd);
    let n_72_simd = (n_51_simd - n_53_simd);
    let n_485_simd = (n_1861_simd - n_1865_simd);
    let n_570_simd = (n_609_simd + n_1903_simd);
    let n_970_simd = (n_1865_simd + n_2001_simd);
    let n_1300_simd = (n_1321_simd - n_1945_simd);
    let n_1330_simd = (n_1351_simd + n_1927_simd);
    let n_1573_simd = (n_1606_simd - n_1903_simd);
    let n_1582_simd = (n_1615_simd - n_1879_simd);
    let n_1591_simd = (n_1939_simd + n_2013_simd);
    let n_1729_simd = (n_1982_simd + n_1985_simd);
    let n_1756_simd = (n_1976_simd + n_1979_simd);
    let n_1768_simd = (n_1970_simd + n_1973_simd);
    let n_1786_simd = (n_2038_simd + n_2041_simd);
    let n_1792_simd = (n_2020_simd + n_2023_simd);
    let n_1801_simd = (n_2050_simd + n_2053_simd);
    let n_1857_simd = (n_90_simd * u64x4::splat(0x608f8u64));
    let n_1869_simd = (n_93_simd * u64x4::splat(0x2bcb4u64));
    let n_1951_simd = (n_166_simd * u64x4::splat(0x34dd8u64));
    let n_1957_simd = (n_164_simd * u64x4::splat(0xffffffffffff7148u64));
    let n_1961_simd = (n_90_simd * u64x4::splat(0xfffffffffffb7700u64));
    let n_1963_simd = (n_91_simd * u64x4::splat(0x608f8u64));
    let n_1988_simd = (n_166_simd * u64x4::splat(0xffffffffffff4a68u64));
    let n_1991_simd = (n_167_simd * u64x4::splat(0x34dd8u64));
    let n_1994_simd = (n_164_simd * u64x4::splat(0xfffffffffffdfe18u64));
    let n_1997_simd = (n_165_simd * u64x4::splat(0xffffffffffff7148u64));
    let n_2015_simd = (n_98_simd * u64x4::splat(0xfffffffffffe33b4u64));
    let n_2017_simd = (n_99_simd * u64x4::splat(0x563f0u64));
    let n_2026_simd = (n_227_simd * u64x4::splat(0xfffffffffffd4880u64));
    let n_2029_simd = (n_228_simd * u64x4::splat(0x2bf20u64));
    // layer 7
    let n_64_simd = (n_62_simd * u64x4::splat(0x801d5u64));
    let n_67_simd = (n_65_simd * u64x4::splat(0xcccbu64));
    let n_454_simd = (n_485_simd - n_1869_simd);
    let n_504_simd = (n_570_simd - n_1330_simd);
    let n_801_simd = (n_1756_simd - n_1729_simd);
    let n_937_simd = (n_970_simd - n_1857_simd);
    let n_988_simd = (n_1897_simd - n_1921_simd);
    let n_1116_simd = (n_1961_simd + n_1963_simd);
    let n_1140_simd = (n_2015_simd + n_2017_simd);
    let n_1276_simd = (n_1300_simd + n_1951_simd);
    let n_1285_simd = (n_1330_simd + n_2035_simd);
    let n_1420_simd = (n_1729_simd + n_1786_simd);
    let n_1540_simd = (n_1921_simd + n_1573_simd);
    let n_1549_simd = (n_1582_simd - n_1927_simd);
    let n_1558_simd = (n_1591_simd - n_1957_simd);
    let n_1705_simd = (n_1988_simd + n_1991_simd);
    let n_1723_simd = (n_1792_simd - n_1768_simd);
    let n_1741_simd = (n_1994_simd + n_1997_simd);
    let n_1750_simd = (n_1801_simd - n_1786_simd);
    let n_1774_simd = (n_2026_simd + n_2029_simd);
    let n_1851_simd = (n_71_simd * u64x4::splat(0xffffffffffff9af0u64));
    let n_1853_simd = (n_72_simd * u64x4::splat(0xd29eu64));
    let n_1873_simd = (n_92_simd * u64x4::splat(0xffffffffffff5af8u64));
    let n_1958_simd = (n_71_simd * u64x4::splat(0xd29eu64));
    let n_1959_simd = (n_72_simd * u64x4::splat(0xffffffffffff9af0u64));
    let n_1965_simd = (n_92_simd * u64x4::splat(0x2bcb4u64));
    let n_1967_simd = (n_93_simd * u64x4::splat(0xffffffffffff5af8u64));
    // layer 8
    let n_69_simd = (n_64_simd + n_67_simd);
    let n_70_simd = (n_64_simd - n_67_simd);
    let n_397_simd = (n_1851_simd - n_1853_simd);
    let n_428_simd = (n_454_simd + n_1873_simd);
    let n_473_simd = (n_504_simd - n_1276_simd);
    let n_702_simd = (n_1958_simd + n_1959_simd);
    let n_769_simd = (n_801_simd - n_1705_simd);
    let n_912_simd = (n_937_simd - n_1873_simd);
    let n_955_simd = (n_988_simd - n_1945_simd);
    let n_1092_simd = (n_1140_simd - n_1116_simd);
    let n_1096_simd = (n_1965_simd + n_1967_simd);
    let n_1261_simd = (n_1285_simd - n_1879_simd);
    let n_1399_simd = (n_1420_simd - n_1768_simd);
    let n_1516_simd = (n_1540_simd - n_1549_simd);
    let n_1525_simd = (n_1558_simd - n_1951_simd);
    let n_1690_simd = (n_1723_simd - n_1729_simd);
    let n_1699_simd = (n_1774_simd - n_1741_simd);
    let n_1714_simd = (n_1750_simd - n_1756_simd);
    // layer 9
    let n_86_simd = (n_69_simd + n_397_simd);
    let n_87_simd = (n_69_simd - n_397_simd);
    let n_88_simd = (n_70_simd + n_702_simd);
    let n_89_simd = (n_70_simd - n_702_simd);
    let n_403_simd = (n_1857_simd - n_428_simd);
    let n_444_simd = (n_473_simd + n_1957_simd);
    let n_708_simd = (n_1116_simd - n_1096_simd);
    let n_742_simd = (n_769_simd + n_1741_simd);
    let n_897_simd = (n_912_simd - n_1869_simd);
    let n_931_simd = (n_955_simd + n_1525_simd);
    let n_1077_simd = (n_1092_simd - n_1096_simd);
    let n_1246_simd = (n_1261_simd - n_1957_simd);
    let n_1384_simd = (n_1399_simd - n_1741_simd);
    let n_1501_simd = (n_1516_simd - n_1525_simd);
    let n_1666_simd = (n_1714_simd - n_1690_simd);
    let n_1675_simd = (n_1699_simd - n_1705_simd);
    // layer 10
    let n_152_simd = (n_86_simd + n_403_simd);
    let n_153_simd = (n_86_simd - n_403_simd);
    let n_154_simd = (n_88_simd + n_708_simd);
    let n_155_simd = (n_88_simd - n_708_simd);
    let n_156_simd = (n_87_simd + n_897_simd);
    let n_157_simd = (n_87_simd - n_897_simd);
    let n_158_simd = (n_89_simd + n_1077_simd);
    let n_159_simd = (n_89_simd - n_1077_simd);
    let n_412_simd = (n_1879_simd - n_444_simd);
    let n_717_simd = (n_1768_simd - n_742_simd);
    let n_906_simd = (n_1549_simd - n_931_simd);
    let n_1086_simd = (n_1690_simd - n_1675_simd);
    let n_1237_simd = (n_1246_simd - n_1276_simd);
    let n_1375_simd = (n_1384_simd - n_1705_simd);
    let n_1492_simd = (n_1501_simd - n_1945_simd);
    let n_1657_simd = (n_1666_simd - n_1675_simd);
    // output
    [
        (n_152_simd + n_412_simd),
        (n_154_simd + n_717_simd),
        (n_156_simd + n_906_simd),
        (n_158_simd + n_1086_simd),
        (n_153_simd + n_1237_simd),
        (n_155_simd + n_1375_simd),
        (n_157_simd + n_1492_simd),
        (n_159_simd + n_1657_simd),
        (n_152_simd - n_412_simd),
        (n_154_simd - n_717_simd),
        (n_156_simd - n_906_simd),
        (n_158_simd - n_1086_simd),
        (n_153_simd - n_1237_simd),
        (n_155_simd - n_1375_simd),
        (n_157_simd - n_1492_simd),
        (n_159_simd - n_1657_simd),
    ]
}

// ===== SIMD VERSION (x8 lanes) =====

#[allow(unused_parens)]
#[rustfmt::skip]
pub fn generated_simd_x8(input: &[[u32; 16]; 8]) -> [u64x8; 16] {
    // Load all inputs into SIMD registers
    let input_0_simd = u64x8::from_array([input[0][0] as u64, input[1][0] as u64, input[2][0] as u64, input[3][0] as u64, input[4][0] as u64, input[5][0] as u64, input[6][0] as u64, input[7][0] as u64]);
    let input_1_simd = u64x8::from_array([input[0][1] as u64, input[1][1] as u64, input[2][1] as u64, input[3][1] as u64, input[4][1] as u64, input[5][1] as u64, input[6][1] as u64, input[7][1] as u64]);
    let input_2_simd = u64x8::from_array([input[0][2] as u64, input[1][2] as u64, input[2][2] as u64, input[3][2] as u64, input[4][2] as u64, input[5][2] as u64, input[6][2] as u64, input[7][2] as u64]);
    let input_3_simd = u64x8::from_array([input[0][3] as u64, input[1][3] as u64, input[2][3] as u64, input[3][3] as u64, input[4][3] as u64, input[5][3] as u64, input[6][3] as u64, input[7][3] as u64]);
    let input_4_simd = u64x8::from_array([input[0][4] as u64, input[1][4] as u64, input[2][4] as u64, input[3][4] as u64, input[4][4] as u64, input[5][4] as u64, input[6][4] as u64, input[7][4] as u64]);
    let input_5_simd = u64x8::from_array([input[0][5] as u64, input[1][5] as u64, input[2][5] as u64, input[3][5] as u64, input[4][5] as u64, input[5][5] as u64, input[6][5] as u64, input[7][5] as u64]);
    let input_6_simd = u64x8::from_array([input[0][6] as u64, input[1][6] as u64, input[2][6] as u64, input[3][6] as u64, input[4][6] as u64, input[5][6] as u64, input[6][6] as u64, input[7][6] as u64]);
    let input_7_simd = u64x8::from_array([input[0][7] as u64, input[1][7] as u64, input[2][7] as u64, input[3][7] as u64, input[4][7] as u64, input[5][7] as u64, input[6][7] as u64, input[7][7] as u64]);
    let input_8_simd = u64x8::from_array([input[0][8] as u64, input[1][8] as u64, input[2][8] as u64, input[3][8] as u64, input[4][8] as u64, input[5][8] as u64, input[6][8] as u64, input[7][8] as u64]);
    let input_9_simd = u64x8::from_array([input[0][9] as u64, input[1][9] as u64, input[2][9] as u64, input[3][9] as u64, input[4][9] as u64, input[5][9] as u64, input[6][9] as u64, input[7][9] as u64]);
    let input_10_simd = u64x8::from_array([input[0][10] as u64, input[1][10] as u64, input[2][10] as u64, input[3][10] as u64, input[4][10] as u64, input[5][10] as u64, input[6][10] as u64, input[7][10] as u64]);
    let input_11_simd = u64x8::from_array([input[0][11] as u64, input[1][11] as u64, input[2][11] as u64, input[3][11] as u64, input[4][11] as u64, input[5][11] as u64, input[6][11] as u64, input[7][11] as u64]);
    let input_12_simd = u64x8::from_array([input[0][12] as u64, input[1][12] as u64, input[2][12] as u64, input[3][12] as u64, input[4][12] as u64, input[5][12] as u64, input[6][12] as u64, input[7][12] as u64]);
    let input_13_simd = u64x8::from_array([input[0][13] as u64, input[1][13] as u64, input[2][13] as u64, input[3][13] as u64, input[4][13] as u64, input[5][13] as u64, input[6][13] as u64, input[7][13] as u64]);
    let input_14_simd = u64x8::from_array([input[0][14] as u64, input[1][14] as u64, input[2][14] as u64, input[3][14] as u64, input[4][14] as u64, input[5][14] as u64, input[6][14] as u64, input[7][14] as u64]);
    let input_15_simd = u64x8::from_array([input[0][15] as u64, input[1][15] as u64, input[2][15] as u64, input[3][15] as u64, input[4][15] as u64, input[5][15] as u64, input[6][15] as u64, input[7][15] as u64]);

    // layer 0
    let n_160_simd = (input_0_simd - input_8_simd);
    let n_161_simd = (input_1_simd - input_9_simd);
    let n_162_simd = (input_2_simd - input_10_simd);
    let n_163_simd = (input_3_simd - input_11_simd);
    let n_164_simd = (input_4_simd - input_12_simd);
    let n_165_simd = (input_5_simd - input_13_simd);
    let n_166_simd = (input_6_simd - input_14_simd);
    let n_167_simd = (input_7_simd - input_15_simd);
    // layer 1
    let n_176_simd = (n_160_simd + n_164_simd);
    let n_177_simd = (n_161_simd + n_165_simd);
    let n_178_simd = (n_162_simd + n_166_simd);
    let n_179_simd = (n_163_simd + n_167_simd);
    // layer 2
    let n_34_simd = (input_0_simd + input_8_simd);
    let n_35_simd = (input_1_simd + input_9_simd);
    let n_36_simd = (input_2_simd + input_10_simd);
    let n_37_simd = (input_3_simd + input_11_simd);
    let n_38_simd = (input_4_simd + input_12_simd);
    let n_39_simd = (input_5_simd + input_13_simd);
    let n_40_simd = (input_6_simd + input_14_simd);
    let n_41_simd = (input_7_simd + input_15_simd);
    let n_185_simd = (n_161_simd + n_163_simd);
    let n_270_simd = (n_176_simd + n_178_simd);
    let n_271_simd = (n_177_simd + n_179_simd);
    // layer 3
    let n_90_simd = (n_34_simd - n_38_simd);
    let n_91_simd = (n_35_simd - n_39_simd);
    let n_92_simd = (n_36_simd - n_40_simd);
    let n_93_simd = (n_37_simd - n_41_simd);
    let n_184_simd = (n_160_simd + n_162_simd);
    let n_228_simd = (n_165_simd + n_167_simd);
    let n_1885_simd = (n_271_simd * u64x8::splat(0xfffffffffff931e8u64));
    let n_1891_simd = (n_177_simd * u64x8::splat(0xfffffffffffac4b0u64));
    let n_1909_simd = (n_185_simd * u64x8::splat(0xfffffffffffbe968u64));
    let n_1915_simd = (n_161_simd * u64x8::splat(0xfffffffffffcc698u64));
    let n_2047_simd = (n_270_simd * u64x8::splat(0x1c070u64));
    // layer 4
    let n_50_simd = (n_34_simd + n_38_simd);
    let n_51_simd = (n_35_simd + n_39_simd);
    let n_52_simd = (n_36_simd + n_40_simd);
    let n_53_simd = (n_37_simd + n_41_simd);
    let n_98_simd = (n_90_simd + n_92_simd);
    let n_99_simd = (n_91_simd + n_93_simd);
    let n_227_simd = (n_164_simd + n_166_simd);
    let n_651_simd = (n_1885_simd - n_1891_simd);
    let n_1360_simd = (n_1909_simd - n_1915_simd);
    let n_1630_simd = (n_1891_simd + n_2047_simd);
    let n_1897_simd = (n_179_simd * u64x8::splat(0xfffffffffffe6d38u64));
    let n_1921_simd = (n_163_simd * u64x8::splat(0xffffffffffff22d0u64));
    let n_1933_simd = (n_228_simd * u64x8::splat(0xfffffffffffd4880u64));
    let n_1939_simd = (n_165_simd * u64x8::splat(0xfffffffffffdfe18u64));
    let n_2007_simd = (n_184_simd * u64x8::splat(0xffffffffffff0150u64));
    let n_2035_simd = (n_176_simd * u64x8::splat(0xfffffffffffffc60u64));
    // layer 5
    let n_58_simd = (n_50_simd + n_52_simd);
    let n_59_simd = (n_51_simd + n_53_simd);
    let n_609_simd = (n_651_simd - n_1897_simd);
    let n_1321_simd = (n_1933_simd - n_1939_simd);
    let n_1351_simd = (n_1360_simd - n_1921_simd);
    let n_1606_simd = (n_1630_simd - n_2035_simd);
    let n_1615_simd = (n_1915_simd + n_2007_simd);
    let n_1861_simd = (n_99_simd * u64x8::splat(0xfffffffffffe33b4u64));
    let n_1865_simd = (n_91_simd * u64x8::splat(0xfffffffffffb7700u64));
    let n_1879_simd = (n_160_simd * u64x8::splat(0x8b18u64));
    let n_1903_simd = (n_178_simd * u64x8::splat(0x1c410u64));
    let n_1927_simd = (n_162_simd * u64x8::splat(0xfffffffffffe7638u64));
    let n_1945_simd = (n_167_simd * u64x8::splat(0xffffffffffff4a68u64));
    let n_1970_simd = (n_160_simd * u64x8::splat(0xfffffffffffcc698u64));
    let n_1973_simd = (n_161_simd * u64x8::splat(0x8b18u64));
    let n_1976_simd = (n_178_simd * u64x8::splat(0xfffffffffffe6d38u64));
    let n_1979_simd = (n_179_simd * u64x8::splat(0x1c410u64));
    let n_1982_simd = (n_162_simd * u64x8::splat(0xffffffffffff22d0u64));
    let n_1985_simd = (n_163_simd * u64x8::splat(0xfffffffffffe7638u64));
    let n_2001_simd = (n_98_simd * u64x8::splat(0x563f0u64));
    let n_2013_simd = (n_227_simd * u64x8::splat(0x2bf20u64));
    let n_2020_simd = (n_184_simd * u64x8::splat(0xfffffffffffbe968u64));
    let n_2023_simd = (n_185_simd * u64x8::splat(0xffffffffffff0150u64));
    let n_2038_simd = (n_176_simd * u64x8::splat(0xfffffffffffac4b0u64));
    let n_2041_simd = (n_177_simd * u64x8::splat(0xfffffffffffffc60u64));
    let n_2050_simd = (n_270_simd * u64x8::splat(0xfffffffffff931e8u64));
    let n_2053_simd = (n_271_simd * u64x8::splat(0x1c070u64));
    // layer 6
    let n_62_simd = (n_58_simd + n_59_simd);
    let n_65_simd = (n_58_simd - n_59_simd);
    let n_71_simd = (n_50_simd - n_52_simd);
    let n_72_simd = (n_51_simd - n_53_simd);
    let n_485_simd = (n_1861_simd - n_1865_simd);
    let n_570_simd = (n_609_simd + n_1903_simd);
    let n_970_simd = (n_1865_simd + n_2001_simd);
    let n_1300_simd = (n_1321_simd - n_1945_simd);
    let n_1330_simd = (n_1351_simd + n_1927_simd);
    let n_1573_simd = (n_1606_simd - n_1903_simd);
    let n_1582_simd = (n_1615_simd - n_1879_simd);
    let n_1591_simd = (n_1939_simd + n_2013_simd);
    let n_1729_simd = (n_1982_simd + n_1985_simd);
    let n_1756_simd = (n_1976_simd + n_1979_simd);
    let n_1768_simd = (n_1970_simd + n_1973_simd);
    let n_1786_simd = (n_2038_simd + n_2041_simd);
    let n_1792_simd = (n_2020_simd + n_2023_simd);
    let n_1801_simd = (n_2050_simd + n_2053_simd);
    let n_1857_simd = (n_90_simd * u64x8::splat(0x608f8u64));
    let n_1869_simd = (n_93_simd * u64x8::splat(0x2bcb4u64));
    let n_1951_simd = (n_166_simd * u64x8::splat(0x34dd8u64));
    let n_1957_simd = (n_164_simd * u64x8::splat(0xffffffffffff7148u64));
    let n_1961_simd = (n_90_simd * u64x8::splat(0xfffffffffffb7700u64));
    let n_1963_simd = (n_91_simd * u64x8::splat(0x608f8u64));
    let n_1988_simd = (n_166_simd * u64x8::splat(0xffffffffffff4a68u64));
    let n_1991_simd = (n_167_simd * u64x8::splat(0x34dd8u64));
    let n_1994_simd = (n_164_simd * u64x8::splat(0xfffffffffffdfe18u64));
    let n_1997_simd = (n_165_simd * u64x8::splat(0xffffffffffff7148u64));
    let n_2015_simd = (n_98_simd * u64x8::splat(0xfffffffffffe33b4u64));
    let n_2017_simd = (n_99_simd * u64x8::splat(0x563f0u64));
    let n_2026_simd = (n_227_simd * u64x8::splat(0xfffffffffffd4880u64));
    let n_2029_simd = (n_228_simd * u64x8::splat(0x2bf20u64));
    // layer 7
    let n_64_simd = (n_62_simd * u64x8::splat(0x801d5u64));
    let n_67_simd = (n_65_simd * u64x8::splat(0xcccbu64));
    let n_454_simd = (n_485_simd - n_1869_simd);
    let n_504_simd = (n_570_simd - n_1330_simd);
    let n_801_simd = (n_1756_simd - n_1729_simd);
    let n_937_simd = (n_970_simd - n_1857_simd);
    let n_988_simd = (n_1897_simd - n_1921_simd);
    let n_1116_simd = (n_1961_simd + n_1963_simd);
    let n_1140_simd = (n_2015_simd + n_2017_simd);
    let n_1276_simd = (n_1300_simd + n_1951_simd);
    let n_1285_simd = (n_1330_simd + n_2035_simd);
    let n_1420_simd = (n_1729_simd + n_1786_simd);
    let n_1540_simd = (n_1921_simd + n_1573_simd);
    let n_1549_simd = (n_1582_simd - n_1927_simd);
    let n_1558_simd = (n_1591_simd - n_1957_simd);
    let n_1705_simd = (n_1988_simd + n_1991_simd);
    let n_1723_simd = (n_1792_simd - n_1768_simd);
    let n_1741_simd = (n_1994_simd + n_1997_simd);
    let n_1750_simd = (n_1801_simd - n_1786_simd);
    let n_1774_simd = (n_2026_simd + n_2029_simd);
    let n_1851_simd = (n_71_simd * u64x8::splat(0xffffffffffff9af0u64));
    let n_1853_simd = (n_72_simd * u64x8::splat(0xd29eu64));
    let n_1873_simd = (n_92_simd * u64x8::splat(0xffffffffffff5af8u64));
    let n_1958_simd = (n_71_simd * u64x8::splat(0xd29eu64));
    let n_1959_simd = (n_72_simd * u64x8::splat(0xffffffffffff9af0u64));
    let n_1965_simd = (n_92_simd * u64x8::splat(0x2bcb4u64));
    let n_1967_simd = (n_93_simd * u64x8::splat(0xffffffffffff5af8u64));
    // layer 8
    let n_69_simd = (n_64_simd + n_67_simd);
    let n_70_simd = (n_64_simd - n_67_simd);
    let n_397_simd = (n_1851_simd - n_1853_simd);
    let n_428_simd = (n_454_simd + n_1873_simd);
    let n_473_simd = (n_504_simd - n_1276_simd);
    let n_702_simd = (n_1958_simd + n_1959_simd);
    let n_769_simd = (n_801_simd - n_1705_simd);
    let n_912_simd = (n_937_simd - n_1873_simd);
    let n_955_simd = (n_988_simd - n_1945_simd);
    let n_1092_simd = (n_1140_simd - n_1116_simd);
    let n_1096_simd = (n_1965_simd + n_1967_simd);
    let n_1261_simd = (n_1285_simd - n_1879_simd);
    let n_1399_simd = (n_1420_simd - n_1768_simd);
    let n_1516_simd = (n_1540_simd - n_1549_simd);
    let n_1525_simd = (n_1558_simd - n_1951_simd);
    let n_1690_simd = (n_1723_simd - n_1729_simd);
    let n_1699_simd = (n_1774_simd - n_1741_simd);
    let n_1714_simd = (n_1750_simd - n_1756_simd);
    // layer 9
    let n_86_simd = (n_69_simd + n_397_simd);
    let n_87_simd = (n_69_simd - n_397_simd);
    let n_88_simd = (n_70_simd + n_702_simd);
    let n_89_simd = (n_70_simd - n_702_simd);
    let n_403_simd = (n_1857_simd - n_428_simd);
    let n_444_simd = (n_473_simd + n_1957_simd);
    let n_708_simd = (n_1116_simd - n_1096_simd);
    let n_742_simd = (n_769_simd + n_1741_simd);
    let n_897_simd = (n_912_simd - n_1869_simd);
    let n_931_simd = (n_955_simd + n_1525_simd);
    let n_1077_simd = (n_1092_simd - n_1096_simd);
    let n_1246_simd = (n_1261_simd - n_1957_simd);
    let n_1384_simd = (n_1399_simd - n_1741_simd);
    let n_1501_simd = (n_1516_simd - n_1525_simd);
    let n_1666_simd = (n_1714_simd - n_1690_simd);
    let n_1675_simd = (n_1699_simd - n_1705_simd);
    // layer 10
    let n_152_simd = (n_86_simd + n_403_simd);
    let n_153_simd = (n_86_simd - n_403_simd);
    let n_154_simd = (n_88_simd + n_708_simd);
    let n_155_simd = (n_88_simd - n_708_simd);
    let n_156_simd = (n_87_simd + n_897_simd);
    let n_157_simd = (n_87_simd - n_897_simd);
    let n_158_simd = (n_89_simd + n_1077_simd);
    let n_159_simd = (n_89_simd - n_1077_simd);
    let n_412_simd = (n_1879_simd - n_444_simd);
    let n_717_simd = (n_1768_simd - n_742_simd);
    let n_906_simd = (n_1549_simd - n_931_simd);
    let n_1086_simd = (n_1690_simd - n_1675_simd);
    let n_1237_simd = (n_1246_simd - n_1276_simd);
    let n_1375_simd = (n_1384_simd - n_1705_simd);
    let n_1492_simd = (n_1501_simd - n_1945_simd);
    let n_1657_simd = (n_1666_simd - n_1675_simd);
    // output
    [
        (n_152_simd + n_412_simd),
        (n_154_simd + n_717_simd),
        (n_156_simd + n_906_simd),
        (n_158_simd + n_1086_simd),
        (n_153_simd + n_1237_simd),
        (n_155_simd + n_1375_simd),
        (n_157_simd + n_1492_simd),
        (n_159_simd + n_1657_simd),
        (n_152_simd - n_412_simd),
        (n_154_simd - n_717_simd),
        (n_156_simd - n_906_simd),
        (n_158_simd - n_1086_simd),
        (n_153_simd - n_1237_simd),
        (n_155_simd - n_1375_simd),
        (n_157_simd - n_1492_simd),
        (n_159_simd - n_1657_simd),
    ]
}

// ===== SCALAR VERSION =====
#[allow(unused_parens)]
pub const fn generated_intermediate(input: &[u32; 16]) -> [u64; 6] {
    // layer 0
    let n_160 = (input[0] as u64).wrapping_sub((input[8] as u64));
    let n_162 = (input[2] as u64).wrapping_sub((input[10] as u64));
    let n_164 = (input[4] as u64).wrapping_sub((input[12] as u64));
    let n_166 = (input[6] as u64).wrapping_sub((input[14] as u64));
    // layer 1
    let n_161 = (input[1] as u64).wrapping_sub((input[9] as u64));
    let n_163 = (input[3] as u64).wrapping_sub((input[11] as u64));
    let n_165 = (input[5] as u64).wrapping_sub((input[13] as u64));
    let n_176 = n_160.wrapping_add(n_164);
    let n_178 = n_162.wrapping_add(n_166);
    // layer 2
    let n_34 = (input[0] as u64).wrapping_add((input[8] as u64));
    let n_35 = (input[1] as u64).wrapping_add((input[9] as u64));
    let n_36 = (input[2] as u64).wrapping_add((input[10] as u64));
    let n_37 = (input[3] as u64).wrapping_add((input[11] as u64));
    let n_38 = (input[4] as u64).wrapping_add((input[12] as u64));
    let n_39 = (input[5] as u64).wrapping_add((input[13] as u64));
    let n_40 = (input[6] as u64).wrapping_add((input[14] as u64));
    let n_41 = (input[7] as u64).wrapping_add((input[15] as u64));
    let n_167 = (input[7] as u64).wrapping_sub((input[15] as u64));
    let n_177 = n_161.wrapping_add(n_165);
    let n_185 = n_161.wrapping_add(n_163);
    let n_270 = n_176.wrapping_add(n_178);
    // layer 3
    let n_90 = n_34.wrapping_sub(n_38);
    let n_91 = n_35.wrapping_sub(n_39);
    let n_92 = n_36.wrapping_sub(n_40);
    let n_93 = n_37.wrapping_sub(n_41);
    let n_179 = n_163.wrapping_add(n_167);
    let n_184 = n_160.wrapping_add(n_162);
    let n_1891 = n_177.wrapping_mul(0xfffffffffffac4b0u64);
    let n_1909 = n_185.wrapping_mul(0xfffffffffffbe968u64);
    let n_1915 = n_161.wrapping_mul(0xfffffffffffcc698u64);
    let n_2047 = n_270.wrapping_mul(0x1c070u64);
    // layer 4
    let n_50 = n_34.wrapping_add(n_38);
    let n_51 = n_35.wrapping_add(n_39);
    let n_52 = n_36.wrapping_add(n_40);
    let n_53 = n_37.wrapping_add(n_41);
    let n_98 = n_90.wrapping_add(n_92);
    let n_99 = n_91.wrapping_add(n_93);
    let n_227 = n_164.wrapping_add(n_166);
    let n_271 = n_177.wrapping_add(n_179);
    let n_1360 = n_1909.wrapping_sub(n_1915);
    let n_1630 = n_1891.wrapping_add(n_2047);
    let n_1921 = n_163.wrapping_mul(0xffffffffffff22d0u64);
    let n_2007 = n_184.wrapping_mul(0xffffffffffff0150u64);
    let n_2035 = n_176.wrapping_mul(0xfffffffffffffc60u64);
    // layer 5
    let n_58 = n_50.wrapping_add(n_52);
    let n_59 = n_51.wrapping_add(n_53);
    let n_228 = n_165.wrapping_add(n_167);
    let n_1351 = n_1360.wrapping_sub(n_1921);
    let n_1606 = n_1630.wrapping_sub(n_2035);
    let n_1615 = n_1915.wrapping_add(n_2007);
    let n_1861 = n_99.wrapping_mul(0xfffffffffffe33b4u64);
    let n_1865 = n_91.wrapping_mul(0xfffffffffffb7700u64);
    let n_1879 = n_160.wrapping_mul(0x8b18u64);
    let n_1903 = n_178.wrapping_mul(0x1c410u64);
    let n_1927 = n_162.wrapping_mul(0xfffffffffffe7638u64);
    let n_1939 = n_165.wrapping_mul(0xfffffffffffdfe18u64);
    let n_1970 = n_160.wrapping_mul(0xfffffffffffcc698u64);
    let n_1973 = n_161.wrapping_mul(0x8b18u64);
    let n_1982 = n_162.wrapping_mul(0xffffffffffff22d0u64);
    let n_1985 = n_163.wrapping_mul(0xfffffffffffe7638u64);
    let n_2001 = n_98.wrapping_mul(0x563f0u64);
    let n_2013 = n_227.wrapping_mul(0x2bf20u64);
    let n_2020 = n_184.wrapping_mul(0xfffffffffffbe968u64);
    let n_2023 = n_185.wrapping_mul(0xffffffffffff0150u64);
    let n_2038 = n_176.wrapping_mul(0xfffffffffffac4b0u64);
    let n_2041 = n_177.wrapping_mul(0xfffffffffffffc60u64);
    let n_2050 = n_270.wrapping_mul(0xfffffffffff931e8u64);
    let n_2053 = n_271.wrapping_mul(0x1c070u64);
    // layer 6
    let n_62 = n_58.wrapping_add(n_59);
    let n_65 = n_58.wrapping_sub(n_59);
    let n_71 = n_50.wrapping_sub(n_52);
    let n_72 = n_51.wrapping_sub(n_53);
    let n_485 = n_1861.wrapping_sub(n_1865);
    let n_970 = n_1865.wrapping_add(n_2001);
    let n_1330 = n_1351.wrapping_add(n_1927);
    let n_1573 = n_1606.wrapping_sub(n_1903);
    let n_1582 = n_1615.wrapping_sub(n_1879);
    let n_1591 = n_1939.wrapping_add(n_2013);
    let n_1729 = n_1982.wrapping_add(n_1985);
    let n_1768 = n_1970.wrapping_add(n_1973);
    let n_1786 = n_2038.wrapping_add(n_2041);
    let n_1792 = n_2020.wrapping_add(n_2023);
    let n_1801 = n_2050.wrapping_add(n_2053);
    let n_1857 = n_90.wrapping_mul(0x608f8u64);
    let n_1869 = n_93.wrapping_mul(0x2bcb4u64);
    let n_1897 = n_179.wrapping_mul(0xfffffffffffe6d38u64);
    let n_1933 = n_228.wrapping_mul(0xfffffffffffd4880u64);
    let n_1957 = n_164.wrapping_mul(0xffffffffffff7148u64);
    let n_1961 = n_90.wrapping_mul(0xfffffffffffb7700u64);
    let n_1963 = n_91.wrapping_mul(0x608f8u64);
    let n_1976 = n_178.wrapping_mul(0xfffffffffffe6d38u64);
    let n_1979 = n_179.wrapping_mul(0x1c410u64);
    let n_1994 = n_164.wrapping_mul(0xfffffffffffdfe18u64);
    let n_1997 = n_165.wrapping_mul(0xffffffffffff7148u64);
    let n_2015 = n_98.wrapping_mul(0xfffffffffffe33b4u64);
    let n_2017 = n_99.wrapping_mul(0x563f0u64);
    let n_2026 = n_227.wrapping_mul(0xfffffffffffd4880u64);
    let n_2029 = n_228.wrapping_mul(0x2bf20u64);
    // layer 7
    let n_64 = n_62.wrapping_mul(0x801d5u64);
    let n_67 = n_65.wrapping_mul(0xcccbu64);
    let n_454 = n_485.wrapping_sub(n_1869);
    let n_937 = n_970.wrapping_sub(n_1857);
    let n_988 = n_1897.wrapping_sub(n_1921);
    let n_1116 = n_1961.wrapping_add(n_1963);
    let n_1140 = n_2015.wrapping_add(n_2017);
    let n_1285 = n_1330.wrapping_add(n_2035);
    let n_1321 = n_1933.wrapping_sub(n_1939);
    let n_1420 = n_1729.wrapping_add(n_1786);
    let n_1540 = n_1921.wrapping_add(n_1573);
    let n_1549 = n_1582.wrapping_sub(n_1927);
    let n_1558 = n_1591.wrapping_sub(n_1957);
    let n_1723 = n_1792.wrapping_sub(n_1768);
    let n_1741 = n_1994.wrapping_add(n_1997);
    let n_1750 = n_1801.wrapping_sub(n_1786);
    let n_1756 = n_1976.wrapping_add(n_1979);
    let n_1774 = n_2026.wrapping_add(n_2029);
    let n_1851 = n_71.wrapping_mul(0xffffffffffff9af0u64);
    let n_1853 = n_72.wrapping_mul(0xd29eu64);
    let n_1873 = n_92.wrapping_mul(0xffffffffffff5af8u64);
    let n_1945 = n_167.wrapping_mul(0xffffffffffff4a68u64);
    let n_1951 = n_166.wrapping_mul(0x34dd8u64);
    let n_1958 = n_71.wrapping_mul(0xd29eu64);
    let n_1959 = n_72.wrapping_mul(0xffffffffffff9af0u64);
    let n_1965 = n_92.wrapping_mul(0x2bcb4u64);
    let n_1967 = n_93.wrapping_mul(0xffffffffffff5af8u64);
    let n_1988 = n_166.wrapping_mul(0xffffffffffff4a68u64);
    let n_1991 = n_167.wrapping_mul(0x34dd8u64);
    // layer 8
    let n_69 = n_64.wrapping_add(n_67);
    let n_70 = n_64.wrapping_sub(n_67);
    let n_397 = n_1851.wrapping_sub(n_1853);
    let n_428 = n_454.wrapping_add(n_1873);
    let n_702 = n_1958.wrapping_add(n_1959);
    let n_912 = n_937.wrapping_sub(n_1873);
    let n_955 = n_988.wrapping_sub(n_1945);
    let n_1092 = n_1140.wrapping_sub(n_1116);
    let n_1096 = n_1965.wrapping_add(n_1967);
    let n_1261 = n_1285.wrapping_sub(n_1879);
    let n_1300 = n_1321.wrapping_sub(n_1945);
    let n_1399 = n_1420.wrapping_sub(n_1768);
    let n_1516 = n_1540.wrapping_sub(n_1549);
    let n_1525 = n_1558.wrapping_sub(n_1951);
    let n_1690 = n_1723.wrapping_sub(n_1729);
    let n_1699 = n_1774.wrapping_sub(n_1741);
    let n_1705 = n_1988.wrapping_add(n_1991);
    let n_1714 = n_1750.wrapping_sub(n_1756);
    // layer 9
    let n_86 = n_69.wrapping_add(n_397);
    let n_87 = n_69.wrapping_sub(n_397);
    let n_88 = n_70.wrapping_add(n_702);
    let n_89 = n_70.wrapping_sub(n_702);
    let n_403 = n_1857.wrapping_sub(n_428);
    let n_708 = n_1116.wrapping_sub(n_1096);
    let n_897 = n_912.wrapping_sub(n_1869);
    let n_931 = n_955.wrapping_add(n_1525);
    let n_1077 = n_1092.wrapping_sub(n_1096);
    let n_1246 = n_1261.wrapping_sub(n_1957);
    let n_1276 = n_1300.wrapping_add(n_1951);
    let n_1384 = n_1399.wrapping_sub(n_1741);
    let n_1501 = n_1516.wrapping_sub(n_1525);
    let n_1666 = n_1714.wrapping_sub(n_1690);
    let n_1675 = n_1699.wrapping_sub(n_1705);
    // layer 10
    let n_153 = n_86.wrapping_sub(n_403);
    let n_155 = n_88.wrapping_sub(n_708);
    let n_156 = n_87.wrapping_add(n_897);
    let n_157 = n_87.wrapping_sub(n_897);
    let n_158 = n_89.wrapping_add(n_1077);
    let n_159 = n_89.wrapping_sub(n_1077);
    let n_906 = n_1549.wrapping_sub(n_931);
    let n_1086 = n_1690.wrapping_sub(n_1675);
    let n_1237 = n_1246.wrapping_sub(n_1276);
    let n_1375 = n_1384.wrapping_sub(n_1705);
    let n_1492 = n_1501.wrapping_sub(n_1945);
    let n_1657 = n_1666.wrapping_sub(n_1675);
    // output
    [
        n_156.wrapping_sub(n_906),
        n_158.wrapping_sub(n_1086),
        n_153.wrapping_sub(n_1237),
        n_155.wrapping_sub(n_1375),
        n_157.wrapping_sub(n_1492),
        n_159.wrapping_sub(n_1657),
    ]
}

// ===== SIMD VERSION (x2 lanes) =====

#[allow(unused_parens)]
#[rustfmt::skip]
pub fn generated_intermediate_simd_x2(input: &[[u32; 16]; 2]) -> [u64x2; 6] {
    // Load all inputs into SIMD registers
    let input_0_simd = u64x2::from_array([input[0][0] as u64, input[1][0] as u64]);
    let input_1_simd = u64x2::from_array([input[0][1] as u64, input[1][1] as u64]);
    let input_2_simd = u64x2::from_array([input[0][2] as u64, input[1][2] as u64]);
    let input_3_simd = u64x2::from_array([input[0][3] as u64, input[1][3] as u64]);
    let input_4_simd = u64x2::from_array([input[0][4] as u64, input[1][4] as u64]);
    let input_5_simd = u64x2::from_array([input[0][5] as u64, input[1][5] as u64]);
    let input_6_simd = u64x2::from_array([input[0][6] as u64, input[1][6] as u64]);
    let input_7_simd = u64x2::from_array([input[0][7] as u64, input[1][7] as u64]);
    let input_8_simd = u64x2::from_array([input[0][8] as u64, input[1][8] as u64]);
    let input_9_simd = u64x2::from_array([input[0][9] as u64, input[1][9] as u64]);
    let input_10_simd = u64x2::from_array([input[0][10] as u64, input[1][10] as u64]);
    let input_11_simd = u64x2::from_array([input[0][11] as u64, input[1][11] as u64]);
    let input_12_simd = u64x2::from_array([input[0][12] as u64, input[1][12] as u64]);
    let input_13_simd = u64x2::from_array([input[0][13] as u64, input[1][13] as u64]);
    let input_14_simd = u64x2::from_array([input[0][14] as u64, input[1][14] as u64]);
    let input_15_simd = u64x2::from_array([input[0][15] as u64, input[1][15] as u64]);

    // layer 0
    let n_160_simd = (input_0_simd - input_8_simd);
    let n_162_simd = (input_2_simd - input_10_simd);
    let n_164_simd = (input_4_simd - input_12_simd);
    let n_166_simd = (input_6_simd - input_14_simd);
    // layer 1
    let n_161_simd = (input_1_simd - input_9_simd);
    let n_163_simd = (input_3_simd - input_11_simd);
    let n_165_simd = (input_5_simd - input_13_simd);
    let n_176_simd = (n_160_simd + n_164_simd);
    let n_178_simd = (n_162_simd + n_166_simd);
    // layer 2
    let n_34_simd = (input_0_simd + input_8_simd);
    let n_35_simd = (input_1_simd + input_9_simd);
    let n_36_simd = (input_2_simd + input_10_simd);
    let n_37_simd = (input_3_simd + input_11_simd);
    let n_38_simd = (input_4_simd + input_12_simd);
    let n_39_simd = (input_5_simd + input_13_simd);
    let n_40_simd = (input_6_simd + input_14_simd);
    let n_41_simd = (input_7_simd + input_15_simd);
    let n_167_simd = (input_7_simd - input_15_simd);
    let n_177_simd = (n_161_simd + n_165_simd);
    let n_185_simd = (n_161_simd + n_163_simd);
    let n_270_simd = (n_176_simd + n_178_simd);
    // layer 3
    let n_90_simd = (n_34_simd - n_38_simd);
    let n_91_simd = (n_35_simd - n_39_simd);
    let n_92_simd = (n_36_simd - n_40_simd);
    let n_93_simd = (n_37_simd - n_41_simd);
    let n_179_simd = (n_163_simd + n_167_simd);
    let n_184_simd = (n_160_simd + n_162_simd);
    let n_1891_simd = (n_177_simd * u64x2::splat(0xfffffffffffac4b0u64));
    let n_1909_simd = (n_185_simd * u64x2::splat(0xfffffffffffbe968u64));
    let n_1915_simd = (n_161_simd * u64x2::splat(0xfffffffffffcc698u64));
    let n_2047_simd = (n_270_simd * u64x2::splat(0x1c070u64));
    // layer 4
    let n_50_simd = (n_34_simd + n_38_simd);
    let n_51_simd = (n_35_simd + n_39_simd);
    let n_52_simd = (n_36_simd + n_40_simd);
    let n_53_simd = (n_37_simd + n_41_simd);
    let n_98_simd = (n_90_simd + n_92_simd);
    let n_99_simd = (n_91_simd + n_93_simd);
    let n_227_simd = (n_164_simd + n_166_simd);
    let n_271_simd = (n_177_simd + n_179_simd);
    let n_1360_simd = (n_1909_simd - n_1915_simd);
    let n_1630_simd = (n_1891_simd + n_2047_simd);
    let n_1921_simd = (n_163_simd * u64x2::splat(0xffffffffffff22d0u64));
    let n_2007_simd = (n_184_simd * u64x2::splat(0xffffffffffff0150u64));
    let n_2035_simd = (n_176_simd * u64x2::splat(0xfffffffffffffc60u64));
    // layer 5
    let n_58_simd = (n_50_simd + n_52_simd);
    let n_59_simd = (n_51_simd + n_53_simd);
    let n_228_simd = (n_165_simd + n_167_simd);
    let n_1351_simd = (n_1360_simd - n_1921_simd);
    let n_1606_simd = (n_1630_simd - n_2035_simd);
    let n_1615_simd = (n_1915_simd + n_2007_simd);
    let n_1861_simd = (n_99_simd * u64x2::splat(0xfffffffffffe33b4u64));
    let n_1865_simd = (n_91_simd * u64x2::splat(0xfffffffffffb7700u64));
    let n_1879_simd = (n_160_simd * u64x2::splat(0x8b18u64));
    let n_1903_simd = (n_178_simd * u64x2::splat(0x1c410u64));
    let n_1927_simd = (n_162_simd * u64x2::splat(0xfffffffffffe7638u64));
    let n_1939_simd = (n_165_simd * u64x2::splat(0xfffffffffffdfe18u64));
    let n_1970_simd = (n_160_simd * u64x2::splat(0xfffffffffffcc698u64));
    let n_1973_simd = (n_161_simd * u64x2::splat(0x8b18u64));
    let n_1982_simd = (n_162_simd * u64x2::splat(0xffffffffffff22d0u64));
    let n_1985_simd = (n_163_simd * u64x2::splat(0xfffffffffffe7638u64));
    let n_2001_simd = (n_98_simd * u64x2::splat(0x563f0u64));
    let n_2013_simd = (n_227_simd * u64x2::splat(0x2bf20u64));
    let n_2020_simd = (n_184_simd * u64x2::splat(0xfffffffffffbe968u64));
    let n_2023_simd = (n_185_simd * u64x2::splat(0xffffffffffff0150u64));
    let n_2038_simd = (n_176_simd * u64x2::splat(0xfffffffffffac4b0u64));
    let n_2041_simd = (n_177_simd * u64x2::splat(0xfffffffffffffc60u64));
    let n_2050_simd = (n_270_simd * u64x2::splat(0xfffffffffff931e8u64));
    let n_2053_simd = (n_271_simd * u64x2::splat(0x1c070u64));
    // layer 6
    let n_62_simd = (n_58_simd + n_59_simd);
    let n_65_simd = (n_58_simd - n_59_simd);
    let n_71_simd = (n_50_simd - n_52_simd);
    let n_72_simd = (n_51_simd - n_53_simd);
    let n_485_simd = (n_1861_simd - n_1865_simd);
    let n_970_simd = (n_1865_simd + n_2001_simd);
    let n_1330_simd = (n_1351_simd + n_1927_simd);
    let n_1573_simd = (n_1606_simd - n_1903_simd);
    let n_1582_simd = (n_1615_simd - n_1879_simd);
    let n_1591_simd = (n_1939_simd + n_2013_simd);
    let n_1729_simd = (n_1982_simd + n_1985_simd);
    let n_1768_simd = (n_1970_simd + n_1973_simd);
    let n_1786_simd = (n_2038_simd + n_2041_simd);
    let n_1792_simd = (n_2020_simd + n_2023_simd);
    let n_1801_simd = (n_2050_simd + n_2053_simd);
    let n_1857_simd = (n_90_simd * u64x2::splat(0x608f8u64));
    let n_1869_simd = (n_93_simd * u64x2::splat(0x2bcb4u64));
    let n_1897_simd = (n_179_simd * u64x2::splat(0xfffffffffffe6d38u64));
    let n_1933_simd = (n_228_simd * u64x2::splat(0xfffffffffffd4880u64));
    let n_1957_simd = (n_164_simd * u64x2::splat(0xffffffffffff7148u64));
    let n_1961_simd = (n_90_simd * u64x2::splat(0xfffffffffffb7700u64));
    let n_1963_simd = (n_91_simd * u64x2::splat(0x608f8u64));
    let n_1976_simd = (n_178_simd * u64x2::splat(0xfffffffffffe6d38u64));
    let n_1979_simd = (n_179_simd * u64x2::splat(0x1c410u64));
    let n_1994_simd = (n_164_simd * u64x2::splat(0xfffffffffffdfe18u64));
    let n_1997_simd = (n_165_simd * u64x2::splat(0xffffffffffff7148u64));
    let n_2015_simd = (n_98_simd * u64x2::splat(0xfffffffffffe33b4u64));
    let n_2017_simd = (n_99_simd * u64x2::splat(0x563f0u64));
    let n_2026_simd = (n_227_simd * u64x2::splat(0xfffffffffffd4880u64));
    let n_2029_simd = (n_228_simd * u64x2::splat(0x2bf20u64));
    // layer 7
    let n_64_simd = (n_62_simd * u64x2::splat(0x801d5u64));
    let n_67_simd = (n_65_simd * u64x2::splat(0xcccbu64));
    let n_454_simd = (n_485_simd - n_1869_simd);
    let n_937_simd = (n_970_simd - n_1857_simd);
    let n_988_simd = (n_1897_simd - n_1921_simd);
    let n_1116_simd = (n_1961_simd + n_1963_simd);
    let n_1140_simd = (n_2015_simd + n_2017_simd);
    let n_1285_simd = (n_1330_simd + n_2035_simd);
    let n_1321_simd = (n_1933_simd - n_1939_simd);
    let n_1420_simd = (n_1729_simd + n_1786_simd);
    let n_1540_simd = (n_1921_simd + n_1573_simd);
    let n_1549_simd = (n_1582_simd - n_1927_simd);
    let n_1558_simd = (n_1591_simd - n_1957_simd);
    let n_1723_simd = (n_1792_simd - n_1768_simd);
    let n_1741_simd = (n_1994_simd + n_1997_simd);
    let n_1750_simd = (n_1801_simd - n_1786_simd);
    let n_1756_simd = (n_1976_simd + n_1979_simd);
    let n_1774_simd = (n_2026_simd + n_2029_simd);
    let n_1851_simd = (n_71_simd * u64x2::splat(0xffffffffffff9af0u64));
    let n_1853_simd = (n_72_simd * u64x2::splat(0xd29eu64));
    let n_1873_simd = (n_92_simd * u64x2::splat(0xffffffffffff5af8u64));
    let n_1945_simd = (n_167_simd * u64x2::splat(0xffffffffffff4a68u64));
    let n_1951_simd = (n_166_simd * u64x2::splat(0x34dd8u64));
    let n_1958_simd = (n_71_simd * u64x2::splat(0xd29eu64));
    let n_1959_simd = (n_72_simd * u64x2::splat(0xffffffffffff9af0u64));
    let n_1965_simd = (n_92_simd * u64x2::splat(0x2bcb4u64));
    let n_1967_simd = (n_93_simd * u64x2::splat(0xffffffffffff5af8u64));
    let n_1988_simd = (n_166_simd * u64x2::splat(0xffffffffffff4a68u64));
    let n_1991_simd = (n_167_simd * u64x2::splat(0x34dd8u64));
    // layer 8
    let n_69_simd = (n_64_simd + n_67_simd);
    let n_70_simd = (n_64_simd - n_67_simd);
    let n_397_simd = (n_1851_simd - n_1853_simd);
    let n_428_simd = (n_454_simd + n_1873_simd);
    let n_702_simd = (n_1958_simd + n_1959_simd);
    let n_912_simd = (n_937_simd - n_1873_simd);
    let n_955_simd = (n_988_simd - n_1945_simd);
    let n_1092_simd = (n_1140_simd - n_1116_simd);
    let n_1096_simd = (n_1965_simd + n_1967_simd);
    let n_1261_simd = (n_1285_simd - n_1879_simd);
    let n_1300_simd = (n_1321_simd - n_1945_simd);
    let n_1399_simd = (n_1420_simd - n_1768_simd);
    let n_1516_simd = (n_1540_simd - n_1549_simd);
    let n_1525_simd = (n_1558_simd - n_1951_simd);
    let n_1690_simd = (n_1723_simd - n_1729_simd);
    let n_1699_simd = (n_1774_simd - n_1741_simd);
    let n_1705_simd = (n_1988_simd + n_1991_simd);
    let n_1714_simd = (n_1750_simd - n_1756_simd);
    // layer 9
    let n_86_simd = (n_69_simd + n_397_simd);
    let n_87_simd = (n_69_simd - n_397_simd);
    let n_88_simd = (n_70_simd + n_702_simd);
    let n_89_simd = (n_70_simd - n_702_simd);
    let n_403_simd = (n_1857_simd - n_428_simd);
    let n_708_simd = (n_1116_simd - n_1096_simd);
    let n_897_simd = (n_912_simd - n_1869_simd);
    let n_931_simd = (n_955_simd + n_1525_simd);
    let n_1077_simd = (n_1092_simd - n_1096_simd);
    let n_1246_simd = (n_1261_simd - n_1957_simd);
    let n_1276_simd = (n_1300_simd + n_1951_simd);
    let n_1384_simd = (n_1399_simd - n_1741_simd);
    let n_1501_simd = (n_1516_simd - n_1525_simd);
    let n_1666_simd = (n_1714_simd - n_1690_simd);
    let n_1675_simd = (n_1699_simd - n_1705_simd);
    // layer 10
    let n_153_simd = (n_86_simd - n_403_simd);
    let n_155_simd = (n_88_simd - n_708_simd);
    let n_156_simd = (n_87_simd + n_897_simd);
    let n_157_simd = (n_87_simd - n_897_simd);
    let n_158_simd = (n_89_simd + n_1077_simd);
    let n_159_simd = (n_89_simd - n_1077_simd);
    let n_906_simd = (n_1549_simd - n_931_simd);
    let n_1086_simd = (n_1690_simd - n_1675_simd);
    let n_1237_simd = (n_1246_simd - n_1276_simd);
    let n_1375_simd = (n_1384_simd - n_1705_simd);
    let n_1492_simd = (n_1501_simd - n_1945_simd);
    let n_1657_simd = (n_1666_simd - n_1675_simd);
    // output
    [
        (n_156_simd - n_906_simd),
        (n_158_simd - n_1086_simd),
        (n_153_simd - n_1237_simd),
        (n_155_simd - n_1375_simd),
        (n_157_simd - n_1492_simd),
        (n_159_simd - n_1657_simd),
    ]
}

// ===== SIMD VERSION (x4 lanes) =====

#[allow(unused_parens)]
#[rustfmt::skip]
pub fn generated_intermediate_simd_x4(input: &[[u32; 16]; 4]) -> [u64x4; 6] {
    // Load all inputs into SIMD registers
    let input_0_simd = u64x4::from_array([input[0][0] as u64, input[1][0] as u64, input[2][0] as u64, input[3][0] as u64]);
    let input_1_simd = u64x4::from_array([input[0][1] as u64, input[1][1] as u64, input[2][1] as u64, input[3][1] as u64]);
    let input_2_simd = u64x4::from_array([input[0][2] as u64, input[1][2] as u64, input[2][2] as u64, input[3][2] as u64]);
    let input_3_simd = u64x4::from_array([input[0][3] as u64, input[1][3] as u64, input[2][3] as u64, input[3][3] as u64]);
    let input_4_simd = u64x4::from_array([input[0][4] as u64, input[1][4] as u64, input[2][4] as u64, input[3][4] as u64]);
    let input_5_simd = u64x4::from_array([input[0][5] as u64, input[1][5] as u64, input[2][5] as u64, input[3][5] as u64]);
    let input_6_simd = u64x4::from_array([input[0][6] as u64, input[1][6] as u64, input[2][6] as u64, input[3][6] as u64]);
    let input_7_simd = u64x4::from_array([input[0][7] as u64, input[1][7] as u64, input[2][7] as u64, input[3][7] as u64]);
    let input_8_simd = u64x4::from_array([input[0][8] as u64, input[1][8] as u64, input[2][8] as u64, input[3][8] as u64]);
    let input_9_simd = u64x4::from_array([input[0][9] as u64, input[1][9] as u64, input[2][9] as u64, input[3][9] as u64]);
    let input_10_simd = u64x4::from_array([input[0][10] as u64, input[1][10] as u64, input[2][10] as u64, input[3][10] as u64]);
    let input_11_simd = u64x4::from_array([input[0][11] as u64, input[1][11] as u64, input[2][11] as u64, input[3][11] as u64]);
    let input_12_simd = u64x4::from_array([input[0][12] as u64, input[1][12] as u64, input[2][12] as u64, input[3][12] as u64]);
    let input_13_simd = u64x4::from_array([input[0][13] as u64, input[1][13] as u64, input[2][13] as u64, input[3][13] as u64]);
    let input_14_simd = u64x4::from_array([input[0][14] as u64, input[1][14] as u64, input[2][14] as u64, input[3][14] as u64]);
    let input_15_simd = u64x4::from_array([input[0][15] as u64, input[1][15] as u64, input[2][15] as u64, input[3][15] as u64]);

    // layer 0
    let n_160_simd = (input_0_simd - input_8_simd);
    let n_162_simd = (input_2_simd - input_10_simd);
    let n_164_simd = (input_4_simd - input_12_simd);
    let n_166_simd = (input_6_simd - input_14_simd);
    // layer 1
    let n_161_simd = (input_1_simd - input_9_simd);
    let n_163_simd = (input_3_simd - input_11_simd);
    let n_165_simd = (input_5_simd - input_13_simd);
    let n_176_simd = (n_160_simd + n_164_simd);
    let n_178_simd = (n_162_simd + n_166_simd);
    // layer 2
    let n_34_simd = (input_0_simd + input_8_simd);
    let n_35_simd = (input_1_simd + input_9_simd);
    let n_36_simd = (input_2_simd + input_10_simd);
    let n_37_simd = (input_3_simd + input_11_simd);
    let n_38_simd = (input_4_simd + input_12_simd);
    let n_39_simd = (input_5_simd + input_13_simd);
    let n_40_simd = (input_6_simd + input_14_simd);
    let n_41_simd = (input_7_simd + input_15_simd);
    let n_167_simd = (input_7_simd - input_15_simd);
    let n_177_simd = (n_161_simd + n_165_simd);
    let n_185_simd = (n_161_simd + n_163_simd);
    let n_270_simd = (n_176_simd + n_178_simd);
    // layer 3
    let n_90_simd = (n_34_simd - n_38_simd);
    let n_91_simd = (n_35_simd - n_39_simd);
    let n_92_simd = (n_36_simd - n_40_simd);
    let n_93_simd = (n_37_simd - n_41_simd);
    let n_179_simd = (n_163_simd + n_167_simd);
    let n_184_simd = (n_160_simd + n_162_simd);
    let n_1891_simd = (n_177_simd * u64x4::splat(0xfffffffffffac4b0u64));
    let n_1909_simd = (n_185_simd * u64x4::splat(0xfffffffffffbe968u64));
    let n_1915_simd = (n_161_simd * u64x4::splat(0xfffffffffffcc698u64));
    let n_2047_simd = (n_270_simd * u64x4::splat(0x1c070u64));
    // layer 4
    let n_50_simd = (n_34_simd + n_38_simd);
    let n_51_simd = (n_35_simd + n_39_simd);
    let n_52_simd = (n_36_simd + n_40_simd);
    let n_53_simd = (n_37_simd + n_41_simd);
    let n_98_simd = (n_90_simd + n_92_simd);
    let n_99_simd = (n_91_simd + n_93_simd);
    let n_227_simd = (n_164_simd + n_166_simd);
    let n_271_simd = (n_177_simd + n_179_simd);
    let n_1360_simd = (n_1909_simd - n_1915_simd);
    let n_1630_simd = (n_1891_simd + n_2047_simd);
    let n_1921_simd = (n_163_simd * u64x4::splat(0xffffffffffff22d0u64));
    let n_2007_simd = (n_184_simd * u64x4::splat(0xffffffffffff0150u64));
    let n_2035_simd = (n_176_simd * u64x4::splat(0xfffffffffffffc60u64));
    // layer 5
    let n_58_simd = (n_50_simd + n_52_simd);
    let n_59_simd = (n_51_simd + n_53_simd);
    let n_228_simd = (n_165_simd + n_167_simd);
    let n_1351_simd = (n_1360_simd - n_1921_simd);
    let n_1606_simd = (n_1630_simd - n_2035_simd);
    let n_1615_simd = (n_1915_simd + n_2007_simd);
    let n_1861_simd = (n_99_simd * u64x4::splat(0xfffffffffffe33b4u64));
    let n_1865_simd = (n_91_simd * u64x4::splat(0xfffffffffffb7700u64));
    let n_1879_simd = (n_160_simd * u64x4::splat(0x8b18u64));
    let n_1903_simd = (n_178_simd * u64x4::splat(0x1c410u64));
    let n_1927_simd = (n_162_simd * u64x4::splat(0xfffffffffffe7638u64));
    let n_1939_simd = (n_165_simd * u64x4::splat(0xfffffffffffdfe18u64));
    let n_1970_simd = (n_160_simd * u64x4::splat(0xfffffffffffcc698u64));
    let n_1973_simd = (n_161_simd * u64x4::splat(0x8b18u64));
    let n_1982_simd = (n_162_simd * u64x4::splat(0xffffffffffff22d0u64));
    let n_1985_simd = (n_163_simd * u64x4::splat(0xfffffffffffe7638u64));
    let n_2001_simd = (n_98_simd * u64x4::splat(0x563f0u64));
    let n_2013_simd = (n_227_simd * u64x4::splat(0x2bf20u64));
    let n_2020_simd = (n_184_simd * u64x4::splat(0xfffffffffffbe968u64));
    let n_2023_simd = (n_185_simd * u64x4::splat(0xffffffffffff0150u64));
    let n_2038_simd = (n_176_simd * u64x4::splat(0xfffffffffffac4b0u64));
    let n_2041_simd = (n_177_simd * u64x4::splat(0xfffffffffffffc60u64));
    let n_2050_simd = (n_270_simd * u64x4::splat(0xfffffffffff931e8u64));
    let n_2053_simd = (n_271_simd * u64x4::splat(0x1c070u64));
    // layer 6
    let n_62_simd = (n_58_simd + n_59_simd);
    let n_65_simd = (n_58_simd - n_59_simd);
    let n_71_simd = (n_50_simd - n_52_simd);
    let n_72_simd = (n_51_simd - n_53_simd);
    let n_485_simd = (n_1861_simd - n_1865_simd);
    let n_970_simd = (n_1865_simd + n_2001_simd);
    let n_1330_simd = (n_1351_simd + n_1927_simd);
    let n_1573_simd = (n_1606_simd - n_1903_simd);
    let n_1582_simd = (n_1615_simd - n_1879_simd);
    let n_1591_simd = (n_1939_simd + n_2013_simd);
    let n_1729_simd = (n_1982_simd + n_1985_simd);
    let n_1768_simd = (n_1970_simd + n_1973_simd);
    let n_1786_simd = (n_2038_simd + n_2041_simd);
    let n_1792_simd = (n_2020_simd + n_2023_simd);
    let n_1801_simd = (n_2050_simd + n_2053_simd);
    let n_1857_simd = (n_90_simd * u64x4::splat(0x608f8u64));
    let n_1869_simd = (n_93_simd * u64x4::splat(0x2bcb4u64));
    let n_1897_simd = (n_179_simd * u64x4::splat(0xfffffffffffe6d38u64));
    let n_1933_simd = (n_228_simd * u64x4::splat(0xfffffffffffd4880u64));
    let n_1957_simd = (n_164_simd * u64x4::splat(0xffffffffffff7148u64));
    let n_1961_simd = (n_90_simd * u64x4::splat(0xfffffffffffb7700u64));
    let n_1963_simd = (n_91_simd * u64x4::splat(0x608f8u64));
    let n_1976_simd = (n_178_simd * u64x4::splat(0xfffffffffffe6d38u64));
    let n_1979_simd = (n_179_simd * u64x4::splat(0x1c410u64));
    let n_1994_simd = (n_164_simd * u64x4::splat(0xfffffffffffdfe18u64));
    let n_1997_simd = (n_165_simd * u64x4::splat(0xffffffffffff7148u64));
    let n_2015_simd = (n_98_simd * u64x4::splat(0xfffffffffffe33b4u64));
    let n_2017_simd = (n_99_simd * u64x4::splat(0x563f0u64));
    let n_2026_simd = (n_227_simd * u64x4::splat(0xfffffffffffd4880u64));
    let n_2029_simd = (n_228_simd * u64x4::splat(0x2bf20u64));
    // layer 7
    let n_64_simd = (n_62_simd * u64x4::splat(0x801d5u64));
    let n_67_simd = (n_65_simd * u64x4::splat(0xcccbu64));
    let n_454_simd = (n_485_simd - n_1869_simd);
    let n_937_simd = (n_970_simd - n_1857_simd);
    let n_988_simd = (n_1897_simd - n_1921_simd);
    let n_1116_simd = (n_1961_simd + n_1963_simd);
    let n_1140_simd = (n_2015_simd + n_2017_simd);
    let n_1285_simd = (n_1330_simd + n_2035_simd);
    let n_1321_simd = (n_1933_simd - n_1939_simd);
    let n_1420_simd = (n_1729_simd + n_1786_simd);
    let n_1540_simd = (n_1921_simd + n_1573_simd);
    let n_1549_simd = (n_1582_simd - n_1927_simd);
    let n_1558_simd = (n_1591_simd - n_1957_simd);
    let n_1723_simd = (n_1792_simd - n_1768_simd);
    let n_1741_simd = (n_1994_simd + n_1997_simd);
    let n_1750_simd = (n_1801_simd - n_1786_simd);
    let n_1756_simd = (n_1976_simd + n_1979_simd);
    let n_1774_simd = (n_2026_simd + n_2029_simd);
    let n_1851_simd = (n_71_simd * u64x4::splat(0xffffffffffff9af0u64));
    let n_1853_simd = (n_72_simd * u64x4::splat(0xd29eu64));
    let n_1873_simd = (n_92_simd * u64x4::splat(0xffffffffffff5af8u64));
    let n_1945_simd = (n_167_simd * u64x4::splat(0xffffffffffff4a68u64));
    let n_1951_simd = (n_166_simd * u64x4::splat(0x34dd8u64));
    let n_1958_simd = (n_71_simd * u64x4::splat(0xd29eu64));
    let n_1959_simd = (n_72_simd * u64x4::splat(0xffffffffffff9af0u64));
    let n_1965_simd = (n_92_simd * u64x4::splat(0x2bcb4u64));
    let n_1967_simd = (n_93_simd * u64x4::splat(0xffffffffffff5af8u64));
    let n_1988_simd = (n_166_simd * u64x4::splat(0xffffffffffff4a68u64));
    let n_1991_simd = (n_167_simd * u64x4::splat(0x34dd8u64));
    // layer 8
    let n_69_simd = (n_64_simd + n_67_simd);
    let n_70_simd = (n_64_simd - n_67_simd);
    let n_397_simd = (n_1851_simd - n_1853_simd);
    let n_428_simd = (n_454_simd + n_1873_simd);
    let n_702_simd = (n_1958_simd + n_1959_simd);
    let n_912_simd = (n_937_simd - n_1873_simd);
    let n_955_simd = (n_988_simd - n_1945_simd);
    let n_1092_simd = (n_1140_simd - n_1116_simd);
    let n_1096_simd = (n_1965_simd + n_1967_simd);
    let n_1261_simd = (n_1285_simd - n_1879_simd);
    let n_1300_simd = (n_1321_simd - n_1945_simd);
    let n_1399_simd = (n_1420_simd - n_1768_simd);
    let n_1516_simd = (n_1540_simd - n_1549_simd);
    let n_1525_simd = (n_1558_simd - n_1951_simd);
    let n_1690_simd = (n_1723_simd - n_1729_simd);
    let n_1699_simd = (n_1774_simd - n_1741_simd);
    let n_1705_simd = (n_1988_simd + n_1991_simd);
    let n_1714_simd = (n_1750_simd - n_1756_simd);
    // layer 9
    let n_86_simd = (n_69_simd + n_397_simd);
    let n_87_simd = (n_69_simd - n_397_simd);
    let n_88_simd = (n_70_simd + n_702_simd);
    let n_89_simd = (n_70_simd - n_702_simd);
    let n_403_simd = (n_1857_simd - n_428_simd);
    let n_708_simd = (n_1116_simd - n_1096_simd);
    let n_897_simd = (n_912_simd - n_1869_simd);
    let n_931_simd = (n_955_simd + n_1525_simd);
    let n_1077_simd = (n_1092_simd - n_1096_simd);
    let n_1246_simd = (n_1261_simd - n_1957_simd);
    let n_1276_simd = (n_1300_simd + n_1951_simd);
    let n_1384_simd = (n_1399_simd - n_1741_simd);
    let n_1501_simd = (n_1516_simd - n_1525_simd);
    let n_1666_simd = (n_1714_simd - n_1690_simd);
    let n_1675_simd = (n_1699_simd - n_1705_simd);
    // layer 10
    let n_153_simd = (n_86_simd - n_403_simd);
    let n_155_simd = (n_88_simd - n_708_simd);
    let n_156_simd = (n_87_simd + n_897_simd);
    let n_157_simd = (n_87_simd - n_897_simd);
    let n_158_simd = (n_89_simd + n_1077_simd);
    let n_159_simd = (n_89_simd - n_1077_simd);
    let n_906_simd = (n_1549_simd - n_931_simd);
    let n_1086_simd = (n_1690_simd - n_1675_simd);
    let n_1237_simd = (n_1246_simd - n_1276_simd);
    let n_1375_simd = (n_1384_simd - n_1705_simd);
    let n_1492_simd = (n_1501_simd - n_1945_simd);
    let n_1657_simd = (n_1666_simd - n_1675_simd);
    // output
    [
        (n_156_simd - n_906_simd),
        (n_158_simd - n_1086_simd),
        (n_153_simd - n_1237_simd),
        (n_155_simd - n_1375_simd),
        (n_157_simd - n_1492_simd),
        (n_159_simd - n_1657_simd),
    ]
}

// ===== SIMD VERSION (x8 lanes) =====

#[allow(unused_parens)]
#[rustfmt::skip]
pub fn generated_intermediate_simd_x8(input: &[[u32; 16]; 8]) -> [u64x8; 6] {
    // Load all inputs into SIMD registers
    let input_0_simd = u64x8::from_array([input[0][0] as u64, input[1][0] as u64, input[2][0] as u64, input[3][0] as u64, input[4][0] as u64, input[5][0] as u64, input[6][0] as u64, input[7][0] as u64]);
    let input_1_simd = u64x8::from_array([input[0][1] as u64, input[1][1] as u64, input[2][1] as u64, input[3][1] as u64, input[4][1] as u64, input[5][1] as u64, input[6][1] as u64, input[7][1] as u64]);
    let input_2_simd = u64x8::from_array([input[0][2] as u64, input[1][2] as u64, input[2][2] as u64, input[3][2] as u64, input[4][2] as u64, input[5][2] as u64, input[6][2] as u64, input[7][2] as u64]);
    let input_3_simd = u64x8::from_array([input[0][3] as u64, input[1][3] as u64, input[2][3] as u64, input[3][3] as u64, input[4][3] as u64, input[5][3] as u64, input[6][3] as u64, input[7][3] as u64]);
    let input_4_simd = u64x8::from_array([input[0][4] as u64, input[1][4] as u64, input[2][4] as u64, input[3][4] as u64, input[4][4] as u64, input[5][4] as u64, input[6][4] as u64, input[7][4] as u64]);
    let input_5_simd = u64x8::from_array([input[0][5] as u64, input[1][5] as u64, input[2][5] as u64, input[3][5] as u64, input[4][5] as u64, input[5][5] as u64, input[6][5] as u64, input[7][5] as u64]);
    let input_6_simd = u64x8::from_array([input[0][6] as u64, input[1][6] as u64, input[2][6] as u64, input[3][6] as u64, input[4][6] as u64, input[5][6] as u64, input[6][6] as u64, input[7][6] as u64]);
    let input_7_simd = u64x8::from_array([input[0][7] as u64, input[1][7] as u64, input[2][7] as u64, input[3][7] as u64, input[4][7] as u64, input[5][7] as u64, input[6][7] as u64, input[7][7] as u64]);
    let input_8_simd = u64x8::from_array([input[0][8] as u64, input[1][8] as u64, input[2][8] as u64, input[3][8] as u64, input[4][8] as u64, input[5][8] as u64, input[6][8] as u64, input[7][8] as u64]);
    let input_9_simd = u64x8::from_array([input[0][9] as u64, input[1][9] as u64, input[2][9] as u64, input[3][9] as u64, input[4][9] as u64, input[5][9] as u64, input[6][9] as u64, input[7][9] as u64]);
    let input_10_simd = u64x8::from_array([input[0][10] as u64, input[1][10] as u64, input[2][10] as u64, input[3][10] as u64, input[4][10] as u64, input[5][10] as u64, input[6][10] as u64, input[7][10] as u64]);
    let input_11_simd = u64x8::from_array([input[0][11] as u64, input[1][11] as u64, input[2][11] as u64, input[3][11] as u64, input[4][11] as u64, input[5][11] as u64, input[6][11] as u64, input[7][11] as u64]);
    let input_12_simd = u64x8::from_array([input[0][12] as u64, input[1][12] as u64, input[2][12] as u64, input[3][12] as u64, input[4][12] as u64, input[5][12] as u64, input[6][12] as u64, input[7][12] as u64]);
    let input_13_simd = u64x8::from_array([input[0][13] as u64, input[1][13] as u64, input[2][13] as u64, input[3][13] as u64, input[4][13] as u64, input[5][13] as u64, input[6][13] as u64, input[7][13] as u64]);
    let input_14_simd = u64x8::from_array([input[0][14] as u64, input[1][14] as u64, input[2][14] as u64, input[3][14] as u64, input[4][14] as u64, input[5][14] as u64, input[6][14] as u64, input[7][14] as u64]);
    let input_15_simd = u64x8::from_array([input[0][15] as u64, input[1][15] as u64, input[2][15] as u64, input[3][15] as u64, input[4][15] as u64, input[5][15] as u64, input[6][15] as u64, input[7][15] as u64]);

    // layer 0
    let n_160_simd = (input_0_simd - input_8_simd);
    let n_162_simd = (input_2_simd - input_10_simd);
    let n_164_simd = (input_4_simd - input_12_simd);
    let n_166_simd = (input_6_simd - input_14_simd);
    // layer 1
    let n_161_simd = (input_1_simd - input_9_simd);
    let n_163_simd = (input_3_simd - input_11_simd);
    let n_165_simd = (input_5_simd - input_13_simd);
    let n_176_simd = (n_160_simd + n_164_simd);
    let n_178_simd = (n_162_simd + n_166_simd);
    // layer 2
    let n_34_simd = (input_0_simd + input_8_simd);
    let n_35_simd = (input_1_simd + input_9_simd);
    let n_36_simd = (input_2_simd + input_10_simd);
    let n_37_simd = (input_3_simd + input_11_simd);
    let n_38_simd = (input_4_simd + input_12_simd);
    let n_39_simd = (input_5_simd + input_13_simd);
    let n_40_simd = (input_6_simd + input_14_simd);
    let n_41_simd = (input_7_simd + input_15_simd);
    let n_167_simd = (input_7_simd - input_15_simd);
    let n_177_simd = (n_161_simd + n_165_simd);
    let n_185_simd = (n_161_simd + n_163_simd);
    let n_270_simd = (n_176_simd + n_178_simd);
    // layer 3
    let n_90_simd = (n_34_simd - n_38_simd);
    let n_91_simd = (n_35_simd - n_39_simd);
    let n_92_simd = (n_36_simd - n_40_simd);
    let n_93_simd = (n_37_simd - n_41_simd);
    let n_179_simd = (n_163_simd + n_167_simd);
    let n_184_simd = (n_160_simd + n_162_simd);
    let n_1891_simd = (n_177_simd * u64x8::splat(0xfffffffffffac4b0u64));
    let n_1909_simd = (n_185_simd * u64x8::splat(0xfffffffffffbe968u64));
    let n_1915_simd = (n_161_simd * u64x8::splat(0xfffffffffffcc698u64));
    let n_2047_simd = (n_270_simd * u64x8::splat(0x1c070u64));
    // layer 4
    let n_50_simd = (n_34_simd + n_38_simd);
    let n_51_simd = (n_35_simd + n_39_simd);
    let n_52_simd = (n_36_simd + n_40_simd);
    let n_53_simd = (n_37_simd + n_41_simd);
    let n_98_simd = (n_90_simd + n_92_simd);
    let n_99_simd = (n_91_simd + n_93_simd);
    let n_227_simd = (n_164_simd + n_166_simd);
    let n_271_simd = (n_177_simd + n_179_simd);
    let n_1360_simd = (n_1909_simd - n_1915_simd);
    let n_1630_simd = (n_1891_simd + n_2047_simd);
    let n_1921_simd = (n_163_simd * u64x8::splat(0xffffffffffff22d0u64));
    let n_2007_simd = (n_184_simd * u64x8::splat(0xffffffffffff0150u64));
    let n_2035_simd = (n_176_simd * u64x8::splat(0xfffffffffffffc60u64));
    // layer 5
    let n_58_simd = (n_50_simd + n_52_simd);
    let n_59_simd = (n_51_simd + n_53_simd);
    let n_228_simd = (n_165_simd + n_167_simd);
    let n_1351_simd = (n_1360_simd - n_1921_simd);
    let n_1606_simd = (n_1630_simd - n_2035_simd);
    let n_1615_simd = (n_1915_simd + n_2007_simd);
    let n_1861_simd = (n_99_simd * u64x8::splat(0xfffffffffffe33b4u64));
    let n_1865_simd = (n_91_simd * u64x8::splat(0xfffffffffffb7700u64));
    let n_1879_simd = (n_160_simd * u64x8::splat(0x8b18u64));
    let n_1903_simd = (n_178_simd * u64x8::splat(0x1c410u64));
    let n_1927_simd = (n_162_simd * u64x8::splat(0xfffffffffffe7638u64));
    let n_1939_simd = (n_165_simd * u64x8::splat(0xfffffffffffdfe18u64));
    let n_1970_simd = (n_160_simd * u64x8::splat(0xfffffffffffcc698u64));
    let n_1973_simd = (n_161_simd * u64x8::splat(0x8b18u64));
    let n_1982_simd = (n_162_simd * u64x8::splat(0xffffffffffff22d0u64));
    let n_1985_simd = (n_163_simd * u64x8::splat(0xfffffffffffe7638u64));
    let n_2001_simd = (n_98_simd * u64x8::splat(0x563f0u64));
    let n_2013_simd = (n_227_simd * u64x8::splat(0x2bf20u64));
    let n_2020_simd = (n_184_simd * u64x8::splat(0xfffffffffffbe968u64));
    let n_2023_simd = (n_185_simd * u64x8::splat(0xffffffffffff0150u64));
    let n_2038_simd = (n_176_simd * u64x8::splat(0xfffffffffffac4b0u64));
    let n_2041_simd = (n_177_simd * u64x8::splat(0xfffffffffffffc60u64));
    let n_2050_simd = (n_270_simd * u64x8::splat(0xfffffffffff931e8u64));
    let n_2053_simd = (n_271_simd * u64x8::splat(0x1c070u64));
    // layer 6
    let n_62_simd = (n_58_simd + n_59_simd);
    let n_65_simd = (n_58_simd - n_59_simd);
    let n_71_simd = (n_50_simd - n_52_simd);
    let n_72_simd = (n_51_simd - n_53_simd);
    let n_485_simd = (n_1861_simd - n_1865_simd);
    let n_970_simd = (n_1865_simd + n_2001_simd);
    let n_1330_simd = (n_1351_simd + n_1927_simd);
    let n_1573_simd = (n_1606_simd - n_1903_simd);
    let n_1582_simd = (n_1615_simd - n_1879_simd);
    let n_1591_simd = (n_1939_simd + n_2013_simd);
    let n_1729_simd = (n_1982_simd + n_1985_simd);
    let n_1768_simd = (n_1970_simd + n_1973_simd);
    let n_1786_simd = (n_2038_simd + n_2041_simd);
    let n_1792_simd = (n_2020_simd + n_2023_simd);
    let n_1801_simd = (n_2050_simd + n_2053_simd);
    let n_1857_simd = (n_90_simd * u64x8::splat(0x608f8u64));
    let n_1869_simd = (n_93_simd * u64x8::splat(0x2bcb4u64));
    let n_1897_simd = (n_179_simd * u64x8::splat(0xfffffffffffe6d38u64));
    let n_1933_simd = (n_228_simd * u64x8::splat(0xfffffffffffd4880u64));
    let n_1957_simd = (n_164_simd * u64x8::splat(0xffffffffffff7148u64));
    let n_1961_simd = (n_90_simd * u64x8::splat(0xfffffffffffb7700u64));
    let n_1963_simd = (n_91_simd * u64x8::splat(0x608f8u64));
    let n_1976_simd = (n_178_simd * u64x8::splat(0xfffffffffffe6d38u64));
    let n_1979_simd = (n_179_simd * u64x8::splat(0x1c410u64));
    let n_1994_simd = (n_164_simd * u64x8::splat(0xfffffffffffdfe18u64));
    let n_1997_simd = (n_165_simd * u64x8::splat(0xffffffffffff7148u64));
    let n_2015_simd = (n_98_simd * u64x8::splat(0xfffffffffffe33b4u64));
    let n_2017_simd = (n_99_simd * u64x8::splat(0x563f0u64));
    let n_2026_simd = (n_227_simd * u64x8::splat(0xfffffffffffd4880u64));
    let n_2029_simd = (n_228_simd * u64x8::splat(0x2bf20u64));
    // layer 7
    let n_64_simd = (n_62_simd * u64x8::splat(0x801d5u64));
    let n_67_simd = (n_65_simd * u64x8::splat(0xcccbu64));
    let n_454_simd = (n_485_simd - n_1869_simd);
    let n_937_simd = (n_970_simd - n_1857_simd);
    let n_988_simd = (n_1897_simd - n_1921_simd);
    let n_1116_simd = (n_1961_simd + n_1963_simd);
    let n_1140_simd = (n_2015_simd + n_2017_simd);
    let n_1285_simd = (n_1330_simd + n_2035_simd);
    let n_1321_simd = (n_1933_simd - n_1939_simd);
    let n_1420_simd = (n_1729_simd + n_1786_simd);
    let n_1540_simd = (n_1921_simd + n_1573_simd);
    let n_1549_simd = (n_1582_simd - n_1927_simd);
    let n_1558_simd = (n_1591_simd - n_1957_simd);
    let n_1723_simd = (n_1792_simd - n_1768_simd);
    let n_1741_simd = (n_1994_simd + n_1997_simd);
    let n_1750_simd = (n_1801_simd - n_1786_simd);
    let n_1756_simd = (n_1976_simd + n_1979_simd);
    let n_1774_simd = (n_2026_simd + n_2029_simd);
    let n_1851_simd = (n_71_simd * u64x8::splat(0xffffffffffff9af0u64));
    let n_1853_simd = (n_72_simd * u64x8::splat(0xd29eu64));
    let n_1873_simd = (n_92_simd * u64x8::splat(0xffffffffffff5af8u64));
    let n_1945_simd = (n_167_simd * u64x8::splat(0xffffffffffff4a68u64));
    let n_1951_simd = (n_166_simd * u64x8::splat(0x34dd8u64));
    let n_1958_simd = (n_71_simd * u64x8::splat(0xd29eu64));
    let n_1959_simd = (n_72_simd * u64x8::splat(0xffffffffffff9af0u64));
    let n_1965_simd = (n_92_simd * u64x8::splat(0x2bcb4u64));
    let n_1967_simd = (n_93_simd * u64x8::splat(0xffffffffffff5af8u64));
    let n_1988_simd = (n_166_simd * u64x8::splat(0xffffffffffff4a68u64));
    let n_1991_simd = (n_167_simd * u64x8::splat(0x34dd8u64));
    // layer 8
    let n_69_simd = (n_64_simd + n_67_simd);
    let n_70_simd = (n_64_simd - n_67_simd);
    let n_397_simd = (n_1851_simd - n_1853_simd);
    let n_428_simd = (n_454_simd + n_1873_simd);
    let n_702_simd = (n_1958_simd + n_1959_simd);
    let n_912_simd = (n_937_simd - n_1873_simd);
    let n_955_simd = (n_988_simd - n_1945_simd);
    let n_1092_simd = (n_1140_simd - n_1116_simd);
    let n_1096_simd = (n_1965_simd + n_1967_simd);
    let n_1261_simd = (n_1285_simd - n_1879_simd);
    let n_1300_simd = (n_1321_simd - n_1945_simd);
    let n_1399_simd = (n_1420_simd - n_1768_simd);
    let n_1516_simd = (n_1540_simd - n_1549_simd);
    let n_1525_simd = (n_1558_simd - n_1951_simd);
    let n_1690_simd = (n_1723_simd - n_1729_simd);
    let n_1699_simd = (n_1774_simd - n_1741_simd);
    let n_1705_simd = (n_1988_simd + n_1991_simd);
    let n_1714_simd = (n_1750_simd - n_1756_simd);
    // layer 9
    let n_86_simd = (n_69_simd + n_397_simd);
    let n_87_simd = (n_69_simd - n_397_simd);
    let n_88_simd = (n_70_simd + n_702_simd);
    let n_89_simd = (n_70_simd - n_702_simd);
    let n_403_simd = (n_1857_simd - n_428_simd);
    let n_708_simd = (n_1116_simd - n_1096_simd);
    let n_897_simd = (n_912_simd - n_1869_simd);
    let n_931_simd = (n_955_simd + n_1525_simd);
    let n_1077_simd = (n_1092_simd - n_1096_simd);
    let n_1246_simd = (n_1261_simd - n_1957_simd);
    let n_1276_simd = (n_1300_simd + n_1951_simd);
    let n_1384_simd = (n_1399_simd - n_1741_simd);
    let n_1501_simd = (n_1516_simd - n_1525_simd);
    let n_1666_simd = (n_1714_simd - n_1690_simd);
    let n_1675_simd = (n_1699_simd - n_1705_simd);
    // layer 10
    let n_153_simd = (n_86_simd - n_403_simd);
    let n_155_simd = (n_88_simd - n_708_simd);
    let n_156_simd = (n_87_simd + n_897_simd);
    let n_157_simd = (n_87_simd - n_897_simd);
    let n_158_simd = (n_89_simd + n_1077_simd);
    let n_159_simd = (n_89_simd - n_1077_simd);
    let n_906_simd = (n_1549_simd - n_931_simd);
    let n_1086_simd = (n_1690_simd - n_1675_simd);
    let n_1237_simd = (n_1246_simd - n_1276_simd);
    let n_1375_simd = (n_1384_simd - n_1705_simd);
    let n_1492_simd = (n_1501_simd - n_1945_simd);
    let n_1657_simd = (n_1666_simd - n_1675_simd);
    // output
    [
        (n_156_simd - n_906_simd),
        (n_158_simd - n_1086_simd),
        (n_153_simd - n_1237_simd),
        (n_155_simd - n_1375_simd),
        (n_157_simd - n_1492_simd),
        (n_159_simd - n_1657_simd),
    ]
}

// ===== SCALAR VERSION =====
#[allow(unused_parens)]
pub const fn generated_last(input: &[u32; 16]) -> [u64; 5] {
    // layer 0
    let n_161 = (input[1] as u64).wrapping_sub((input[9] as u64));
    let n_163 = (input[3] as u64).wrapping_sub((input[11] as u64));
    let n_165 = (input[5] as u64).wrapping_sub((input[13] as u64));
    let n_167 = (input[7] as u64).wrapping_sub((input[15] as u64));
    // layer 1
    let n_177 = n_161.wrapping_add(n_165);
    let n_179 = n_163.wrapping_add(n_167);
    // layer 2
    let n_34 = (input[0] as u64).wrapping_add((input[8] as u64));
    let n_35 = (input[1] as u64).wrapping_add((input[9] as u64));
    let n_36 = (input[2] as u64).wrapping_add((input[10] as u64));
    let n_37 = (input[3] as u64).wrapping_add((input[11] as u64));
    let n_38 = (input[4] as u64).wrapping_add((input[12] as u64));
    let n_39 = (input[5] as u64).wrapping_add((input[13] as u64));
    let n_40 = (input[6] as u64).wrapping_add((input[14] as u64));
    let n_41 = (input[7] as u64).wrapping_add((input[15] as u64));
    let n_185 = n_161.wrapping_add(n_163);
    let n_271 = n_177.wrapping_add(n_179);
    // layer 3
    let n_90 = n_34.wrapping_sub(n_38);
    let n_91 = n_35.wrapping_sub(n_39);
    let n_92 = n_36.wrapping_sub(n_40);
    let n_93 = n_37.wrapping_sub(n_41);
    let n_162 = (input[2] as u64).wrapping_sub((input[10] as u64));
    let n_164 = (input[4] as u64).wrapping_sub((input[12] as u64));
    let n_166 = (input[6] as u64).wrapping_sub((input[14] as u64));
    let n_228 = n_165.wrapping_add(n_167);
    let n_1885 = n_271.wrapping_mul(0xfffffffffff931e8u64);
    let n_1891 = n_177.wrapping_mul(0xfffffffffffac4b0u64);
    let n_1909 = n_185.wrapping_mul(0xfffffffffffbe968u64);
    let n_1915 = n_161.wrapping_mul(0xfffffffffffcc698u64);
    // layer 4
    let n_50 = n_34.wrapping_add(n_38);
    let n_51 = n_35.wrapping_add(n_39);
    let n_52 = n_36.wrapping_add(n_40);
    let n_53 = n_37.wrapping_add(n_41);
    let n_98 = n_90.wrapping_add(n_92);
    let n_99 = n_91.wrapping_add(n_93);
    let n_160 = (input[0] as u64).wrapping_sub((input[8] as u64));
    let n_178 = n_162.wrapping_add(n_166);
    let n_227 = n_164.wrapping_add(n_166);
    let n_651 = n_1885.wrapping_sub(n_1891);
    let n_1360 = n_1909.wrapping_sub(n_1915);
    let n_1897 = n_179.wrapping_mul(0xfffffffffffe6d38u64);
    let n_1921 = n_163.wrapping_mul(0xffffffffffff22d0u64);
    let n_1933 = n_228.wrapping_mul(0xfffffffffffd4880u64);
    let n_1939 = n_165.wrapping_mul(0xfffffffffffdfe18u64);
    // layer 5
    let n_58 = n_50.wrapping_add(n_52);
    let n_59 = n_51.wrapping_add(n_53);
    let n_176 = n_160.wrapping_add(n_164);
    let n_184 = n_160.wrapping_add(n_162);
    let n_609 = n_651.wrapping_sub(n_1897);
    let n_1321 = n_1933.wrapping_sub(n_1939);
    let n_1351 = n_1360.wrapping_sub(n_1921);
    let n_1861 = n_99.wrapping_mul(0xfffffffffffe33b4u64);
    let n_1865 = n_91.wrapping_mul(0xfffffffffffb7700u64);
    let n_1903 = n_178.wrapping_mul(0x1c410u64);
    let n_1927 = n_162.wrapping_mul(0xfffffffffffe7638u64);
    let n_1945 = n_167.wrapping_mul(0xffffffffffff4a68u64);
    let n_1976 = n_178.wrapping_mul(0xfffffffffffe6d38u64);
    let n_1979 = n_179.wrapping_mul(0x1c410u64);
    let n_1982 = n_162.wrapping_mul(0xffffffffffff22d0u64);
    let n_1985 = n_163.wrapping_mul(0xfffffffffffe7638u64);
    let n_2001 = n_98.wrapping_mul(0x563f0u64);
    let n_2013 = n_227.wrapping_mul(0x2bf20u64);
    // layer 6
    let n_62 = n_58.wrapping_add(n_59);
    let n_65 = n_58.wrapping_sub(n_59);
    let n_71 = n_50.wrapping_sub(n_52);
    let n_72 = n_51.wrapping_sub(n_53);
    let n_485 = n_1861.wrapping_sub(n_1865);
    let n_570 = n_609.wrapping_add(n_1903);
    let n_970 = n_1865.wrapping_add(n_2001);
    let n_1300 = n_1321.wrapping_sub(n_1945);
    let n_1330 = n_1351.wrapping_add(n_1927);
    let n_1591 = n_1939.wrapping_add(n_2013);
    let n_1729 = n_1982.wrapping_add(n_1985);
    let n_1756 = n_1976.wrapping_add(n_1979);
    let n_1857 = n_90.wrapping_mul(0x608f8u64);
    let n_1869 = n_93.wrapping_mul(0x2bcb4u64);
    let n_1951 = n_166.wrapping_mul(0x34dd8u64);
    let n_1957 = n_164.wrapping_mul(0xffffffffffff7148u64);
    let n_1961 = n_90.wrapping_mul(0xfffffffffffb7700u64);
    let n_1963 = n_91.wrapping_mul(0x608f8u64);
    let n_1970 = n_160.wrapping_mul(0xfffffffffffcc698u64);
    let n_1973 = n_161.wrapping_mul(0x8b18u64);
    let n_1988 = n_166.wrapping_mul(0xffffffffffff4a68u64);
    let n_1991 = n_167.wrapping_mul(0x34dd8u64);
    let n_1994 = n_164.wrapping_mul(0xfffffffffffdfe18u64);
    let n_1997 = n_165.wrapping_mul(0xffffffffffff7148u64);
    let n_2007 = n_184.wrapping_mul(0xffffffffffff0150u64);
    let n_2015 = n_98.wrapping_mul(0xfffffffffffe33b4u64);
    let n_2017 = n_99.wrapping_mul(0x563f0u64);
    let n_2020 = n_184.wrapping_mul(0xfffffffffffbe968u64);
    let n_2023 = n_185.wrapping_mul(0xffffffffffff0150u64);
    let n_2026 = n_227.wrapping_mul(0xfffffffffffd4880u64);
    let n_2029 = n_228.wrapping_mul(0x2bf20u64);
    let n_2035 = n_176.wrapping_mul(0xfffffffffffffc60u64);
    // layer 7
    let n_64 = n_62.wrapping_mul(0x801d5u64);
    let n_67 = n_65.wrapping_mul(0xcccbu64);
    let n_454 = n_485.wrapping_sub(n_1869);
    let n_504 = n_570.wrapping_sub(n_1330);
    let n_801 = n_1756.wrapping_sub(n_1729);
    let n_937 = n_970.wrapping_sub(n_1857);
    let n_988 = n_1897.wrapping_sub(n_1921);
    let n_1116 = n_1961.wrapping_add(n_1963);
    let n_1140 = n_2015.wrapping_add(n_2017);
    let n_1276 = n_1300.wrapping_add(n_1951);
    let n_1285 = n_1330.wrapping_add(n_2035);
    let n_1558 = n_1591.wrapping_sub(n_1957);
    let n_1615 = n_1915.wrapping_add(n_2007);
    let n_1705 = n_1988.wrapping_add(n_1991);
    let n_1741 = n_1994.wrapping_add(n_1997);
    let n_1768 = n_1970.wrapping_add(n_1973);
    let n_1774 = n_2026.wrapping_add(n_2029);
    let n_1792 = n_2020.wrapping_add(n_2023);
    let n_1851 = n_71.wrapping_mul(0xffffffffffff9af0u64);
    let n_1853 = n_72.wrapping_mul(0xd29eu64);
    let n_1873 = n_92.wrapping_mul(0xffffffffffff5af8u64);
    let n_1879 = n_160.wrapping_mul(0x8b18u64);
    let n_1958 = n_71.wrapping_mul(0xd29eu64);
    let n_1959 = n_72.wrapping_mul(0xffffffffffff9af0u64);
    let n_1965 = n_92.wrapping_mul(0x2bcb4u64);
    let n_1967 = n_93.wrapping_mul(0xffffffffffff5af8u64);
    // layer 8
    let n_69 = n_64.wrapping_add(n_67);
    let n_70 = n_64.wrapping_sub(n_67);
    let n_397 = n_1851.wrapping_sub(n_1853);
    let n_428 = n_454.wrapping_add(n_1873);
    let n_473 = n_504.wrapping_sub(n_1276);
    let n_702 = n_1958.wrapping_add(n_1959);
    let n_769 = n_801.wrapping_sub(n_1705);
    let n_912 = n_937.wrapping_sub(n_1873);
    let n_955 = n_988.wrapping_sub(n_1945);
    let n_1092 = n_1140.wrapping_sub(n_1116);
    let n_1096 = n_1965.wrapping_add(n_1967);
    let n_1261 = n_1285.wrapping_sub(n_1879);
    let n_1525 = n_1558.wrapping_sub(n_1951);
    let n_1582 = n_1615.wrapping_sub(n_1879);
    let n_1699 = n_1774.wrapping_sub(n_1741);
    let n_1723 = n_1792.wrapping_sub(n_1768);
    // layer 9
    let n_86 = n_69.wrapping_add(n_397);
    let n_87 = n_69.wrapping_sub(n_397);
    let n_88 = n_70.wrapping_add(n_702);
    let n_89 = n_70.wrapping_sub(n_702);
    let n_403 = n_1857.wrapping_sub(n_428);
    let n_444 = n_473.wrapping_add(n_1957);
    let n_708 = n_1116.wrapping_sub(n_1096);
    let n_742 = n_769.wrapping_add(n_1741);
    let n_897 = n_912.wrapping_sub(n_1869);
    let n_931 = n_955.wrapping_add(n_1525);
    let n_1077 = n_1092.wrapping_sub(n_1096);
    let n_1246 = n_1261.wrapping_sub(n_1957);
    let n_1549 = n_1582.wrapping_sub(n_1927);
    let n_1675 = n_1699.wrapping_sub(n_1705);
    let n_1690 = n_1723.wrapping_sub(n_1729);
    // layer 10
    let n_152 = n_86.wrapping_add(n_403);
    let n_153 = n_86.wrapping_sub(n_403);
    let n_154 = n_88.wrapping_add(n_708);
    let n_156 = n_87.wrapping_add(n_897);
    let n_158 = n_89.wrapping_add(n_1077);
    let n_412 = n_1879.wrapping_sub(n_444);
    let n_717 = n_1768.wrapping_sub(n_742);
    let n_906 = n_1549.wrapping_sub(n_931);
    let n_1086 = n_1690.wrapping_sub(n_1675);
    let n_1237 = n_1246.wrapping_sub(n_1276);
    // output
    [
        n_152.wrapping_add(n_412),
        n_154.wrapping_add(n_717),
        n_156.wrapping_add(n_906),
        n_158.wrapping_add(n_1086),
        n_153.wrapping_add(n_1237),
    ]
}

// ===== SIMD VERSION (x2 lanes) =====

#[allow(unused_parens)]
#[rustfmt::skip]
pub fn generated_last_simd_x2(input: &[[u32; 16]; 2]) -> [u64x2; 5] {
    // Load all inputs into SIMD registers
    let input_0_simd = u64x2::from_array([input[0][0] as u64, input[1][0] as u64]);
    let input_1_simd = u64x2::from_array([input[0][1] as u64, input[1][1] as u64]);
    let input_2_simd = u64x2::from_array([input[0][2] as u64, input[1][2] as u64]);
    let input_3_simd = u64x2::from_array([input[0][3] as u64, input[1][3] as u64]);
    let input_4_simd = u64x2::from_array([input[0][4] as u64, input[1][4] as u64]);
    let input_5_simd = u64x2::from_array([input[0][5] as u64, input[1][5] as u64]);
    let input_6_simd = u64x2::from_array([input[0][6] as u64, input[1][6] as u64]);
    let input_7_simd = u64x2::from_array([input[0][7] as u64, input[1][7] as u64]);
    let input_8_simd = u64x2::from_array([input[0][8] as u64, input[1][8] as u64]);
    let input_9_simd = u64x2::from_array([input[0][9] as u64, input[1][9] as u64]);
    let input_10_simd = u64x2::from_array([input[0][10] as u64, input[1][10] as u64]);
    let input_11_simd = u64x2::from_array([input[0][11] as u64, input[1][11] as u64]);
    let input_12_simd = u64x2::from_array([input[0][12] as u64, input[1][12] as u64]);
    let input_13_simd = u64x2::from_array([input[0][13] as u64, input[1][13] as u64]);
    let input_14_simd = u64x2::from_array([input[0][14] as u64, input[1][14] as u64]);
    let input_15_simd = u64x2::from_array([input[0][15] as u64, input[1][15] as u64]);

    // layer 0
    let n_161_simd = (input_1_simd - input_9_simd);
    let n_163_simd = (input_3_simd - input_11_simd);
    let n_165_simd = (input_5_simd - input_13_simd);
    let n_167_simd = (input_7_simd - input_15_simd);
    // layer 1
    let n_177_simd = (n_161_simd + n_165_simd);
    let n_179_simd = (n_163_simd + n_167_simd);
    // layer 2
    let n_34_simd = (input_0_simd + input_8_simd);
    let n_35_simd = (input_1_simd + input_9_simd);
    let n_36_simd = (input_2_simd + input_10_simd);
    let n_37_simd = (input_3_simd + input_11_simd);
    let n_38_simd = (input_4_simd + input_12_simd);
    let n_39_simd = (input_5_simd + input_13_simd);
    let n_40_simd = (input_6_simd + input_14_simd);
    let n_41_simd = (input_7_simd + input_15_simd);
    let n_185_simd = (n_161_simd + n_163_simd);
    let n_271_simd = (n_177_simd + n_179_simd);
    // layer 3
    let n_90_simd = (n_34_simd - n_38_simd);
    let n_91_simd = (n_35_simd - n_39_simd);
    let n_92_simd = (n_36_simd - n_40_simd);
    let n_93_simd = (n_37_simd - n_41_simd);
    let n_162_simd = (input_2_simd - input_10_simd);
    let n_164_simd = (input_4_simd - input_12_simd);
    let n_166_simd = (input_6_simd - input_14_simd);
    let n_228_simd = (n_165_simd + n_167_simd);
    let n_1885_simd = (n_271_simd * u64x2::splat(0xfffffffffff931e8u64));
    let n_1891_simd = (n_177_simd * u64x2::splat(0xfffffffffffac4b0u64));
    let n_1909_simd = (n_185_simd * u64x2::splat(0xfffffffffffbe968u64));
    let n_1915_simd = (n_161_simd * u64x2::splat(0xfffffffffffcc698u64));
    // layer 4
    let n_50_simd = (n_34_simd + n_38_simd);
    let n_51_simd = (n_35_simd + n_39_simd);
    let n_52_simd = (n_36_simd + n_40_simd);
    let n_53_simd = (n_37_simd + n_41_simd);
    let n_98_simd = (n_90_simd + n_92_simd);
    let n_99_simd = (n_91_simd + n_93_simd);
    let n_160_simd = (input_0_simd - input_8_simd);
    let n_178_simd = (n_162_simd + n_166_simd);
    let n_227_simd = (n_164_simd + n_166_simd);
    let n_651_simd = (n_1885_simd - n_1891_simd);
    let n_1360_simd = (n_1909_simd - n_1915_simd);
    let n_1897_simd = (n_179_simd * u64x2::splat(0xfffffffffffe6d38u64));
    let n_1921_simd = (n_163_simd * u64x2::splat(0xffffffffffff22d0u64));
    let n_1933_simd = (n_228_simd * u64x2::splat(0xfffffffffffd4880u64));
    let n_1939_simd = (n_165_simd * u64x2::splat(0xfffffffffffdfe18u64));
    // layer 5
    let n_58_simd = (n_50_simd + n_52_simd);
    let n_59_simd = (n_51_simd + n_53_simd);
    let n_176_simd = (n_160_simd + n_164_simd);
    let n_184_simd = (n_160_simd + n_162_simd);
    let n_609_simd = (n_651_simd - n_1897_simd);
    let n_1321_simd = (n_1933_simd - n_1939_simd);
    let n_1351_simd = (n_1360_simd - n_1921_simd);
    let n_1861_simd = (n_99_simd * u64x2::splat(0xfffffffffffe33b4u64));
    let n_1865_simd = (n_91_simd * u64x2::splat(0xfffffffffffb7700u64));
    let n_1903_simd = (n_178_simd * u64x2::splat(0x1c410u64));
    let n_1927_simd = (n_162_simd * u64x2::splat(0xfffffffffffe7638u64));
    let n_1945_simd = (n_167_simd * u64x2::splat(0xffffffffffff4a68u64));
    let n_1976_simd = (n_178_simd * u64x2::splat(0xfffffffffffe6d38u64));
    let n_1979_simd = (n_179_simd * u64x2::splat(0x1c410u64));
    let n_1982_simd = (n_162_simd * u64x2::splat(0xffffffffffff22d0u64));
    let n_1985_simd = (n_163_simd * u64x2::splat(0xfffffffffffe7638u64));
    let n_2001_simd = (n_98_simd * u64x2::splat(0x563f0u64));
    let n_2013_simd = (n_227_simd * u64x2::splat(0x2bf20u64));
    // layer 6
    let n_62_simd = (n_58_simd + n_59_simd);
    let n_65_simd = (n_58_simd - n_59_simd);
    let n_71_simd = (n_50_simd - n_52_simd);
    let n_72_simd = (n_51_simd - n_53_simd);
    let n_485_simd = (n_1861_simd - n_1865_simd);
    let n_570_simd = (n_609_simd + n_1903_simd);
    let n_970_simd = (n_1865_simd + n_2001_simd);
    let n_1300_simd = (n_1321_simd - n_1945_simd);
    let n_1330_simd = (n_1351_simd + n_1927_simd);
    let n_1591_simd = (n_1939_simd + n_2013_simd);
    let n_1729_simd = (n_1982_simd + n_1985_simd);
    let n_1756_simd = (n_1976_simd + n_1979_simd);
    let n_1857_simd = (n_90_simd * u64x2::splat(0x608f8u64));
    let n_1869_simd = (n_93_simd * u64x2::splat(0x2bcb4u64));
    let n_1951_simd = (n_166_simd * u64x2::splat(0x34dd8u64));
    let n_1957_simd = (n_164_simd * u64x2::splat(0xffffffffffff7148u64));
    let n_1961_simd = (n_90_simd * u64x2::splat(0xfffffffffffb7700u64));
    let n_1963_simd = (n_91_simd * u64x2::splat(0x608f8u64));
    let n_1970_simd = (n_160_simd * u64x2::splat(0xfffffffffffcc698u64));
    let n_1973_simd = (n_161_simd * u64x2::splat(0x8b18u64));
    let n_1988_simd = (n_166_simd * u64x2::splat(0xffffffffffff4a68u64));
    let n_1991_simd = (n_167_simd * u64x2::splat(0x34dd8u64));
    let n_1994_simd = (n_164_simd * u64x2::splat(0xfffffffffffdfe18u64));
    let n_1997_simd = (n_165_simd * u64x2::splat(0xffffffffffff7148u64));
    let n_2007_simd = (n_184_simd * u64x2::splat(0xffffffffffff0150u64));
    let n_2015_simd = (n_98_simd * u64x2::splat(0xfffffffffffe33b4u64));
    let n_2017_simd = (n_99_simd * u64x2::splat(0x563f0u64));
    let n_2020_simd = (n_184_simd * u64x2::splat(0xfffffffffffbe968u64));
    let n_2023_simd = (n_185_simd * u64x2::splat(0xffffffffffff0150u64));
    let n_2026_simd = (n_227_simd * u64x2::splat(0xfffffffffffd4880u64));
    let n_2029_simd = (n_228_simd * u64x2::splat(0x2bf20u64));
    let n_2035_simd = (n_176_simd * u64x2::splat(0xfffffffffffffc60u64));
    // layer 7
    let n_64_simd = (n_62_simd * u64x2::splat(0x801d5u64));
    let n_67_simd = (n_65_simd * u64x2::splat(0xcccbu64));
    let n_454_simd = (n_485_simd - n_1869_simd);
    let n_504_simd = (n_570_simd - n_1330_simd);
    let n_801_simd = (n_1756_simd - n_1729_simd);
    let n_937_simd = (n_970_simd - n_1857_simd);
    let n_988_simd = (n_1897_simd - n_1921_simd);
    let n_1116_simd = (n_1961_simd + n_1963_simd);
    let n_1140_simd = (n_2015_simd + n_2017_simd);
    let n_1276_simd = (n_1300_simd + n_1951_simd);
    let n_1285_simd = (n_1330_simd + n_2035_simd);
    let n_1558_simd = (n_1591_simd - n_1957_simd);
    let n_1615_simd = (n_1915_simd + n_2007_simd);
    let n_1705_simd = (n_1988_simd + n_1991_simd);
    let n_1741_simd = (n_1994_simd + n_1997_simd);
    let n_1768_simd = (n_1970_simd + n_1973_simd);
    let n_1774_simd = (n_2026_simd + n_2029_simd);
    let n_1792_simd = (n_2020_simd + n_2023_simd);
    let n_1851_simd = (n_71_simd * u64x2::splat(0xffffffffffff9af0u64));
    let n_1853_simd = (n_72_simd * u64x2::splat(0xd29eu64));
    let n_1873_simd = (n_92_simd * u64x2::splat(0xffffffffffff5af8u64));
    let n_1879_simd = (n_160_simd * u64x2::splat(0x8b18u64));
    let n_1958_simd = (n_71_simd * u64x2::splat(0xd29eu64));
    let n_1959_simd = (n_72_simd * u64x2::splat(0xffffffffffff9af0u64));
    let n_1965_simd = (n_92_simd * u64x2::splat(0x2bcb4u64));
    let n_1967_simd = (n_93_simd * u64x2::splat(0xffffffffffff5af8u64));
    // layer 8
    let n_69_simd = (n_64_simd + n_67_simd);
    let n_70_simd = (n_64_simd - n_67_simd);
    let n_397_simd = (n_1851_simd - n_1853_simd);
    let n_428_simd = (n_454_simd + n_1873_simd);
    let n_473_simd = (n_504_simd - n_1276_simd);
    let n_702_simd = (n_1958_simd + n_1959_simd);
    let n_769_simd = (n_801_simd - n_1705_simd);
    let n_912_simd = (n_937_simd - n_1873_simd);
    let n_955_simd = (n_988_simd - n_1945_simd);
    let n_1092_simd = (n_1140_simd - n_1116_simd);
    let n_1096_simd = (n_1965_simd + n_1967_simd);
    let n_1261_simd = (n_1285_simd - n_1879_simd);
    let n_1525_simd = (n_1558_simd - n_1951_simd);
    let n_1582_simd = (n_1615_simd - n_1879_simd);
    let n_1699_simd = (n_1774_simd - n_1741_simd);
    let n_1723_simd = (n_1792_simd - n_1768_simd);
    // layer 9
    let n_86_simd = (n_69_simd + n_397_simd);
    let n_87_simd = (n_69_simd - n_397_simd);
    let n_88_simd = (n_70_simd + n_702_simd);
    let n_89_simd = (n_70_simd - n_702_simd);
    let n_403_simd = (n_1857_simd - n_428_simd);
    let n_444_simd = (n_473_simd + n_1957_simd);
    let n_708_simd = (n_1116_simd - n_1096_simd);
    let n_742_simd = (n_769_simd + n_1741_simd);
    let n_897_simd = (n_912_simd - n_1869_simd);
    let n_931_simd = (n_955_simd + n_1525_simd);
    let n_1077_simd = (n_1092_simd - n_1096_simd);
    let n_1246_simd = (n_1261_simd - n_1957_simd);
    let n_1549_simd = (n_1582_simd - n_1927_simd);
    let n_1675_simd = (n_1699_simd - n_1705_simd);
    let n_1690_simd = (n_1723_simd - n_1729_simd);
    // layer 10
    let n_152_simd = (n_86_simd + n_403_simd);
    let n_153_simd = (n_86_simd - n_403_simd);
    let n_154_simd = (n_88_simd + n_708_simd);
    let n_156_simd = (n_87_simd + n_897_simd);
    let n_158_simd = (n_89_simd + n_1077_simd);
    let n_412_simd = (n_1879_simd - n_444_simd);
    let n_717_simd = (n_1768_simd - n_742_simd);
    let n_906_simd = (n_1549_simd - n_931_simd);
    let n_1086_simd = (n_1690_simd - n_1675_simd);
    let n_1237_simd = (n_1246_simd - n_1276_simd);
    // output
    [
        (n_152_simd + n_412_simd),
        (n_154_simd + n_717_simd),
        (n_156_simd + n_906_simd),
        (n_158_simd + n_1086_simd),
        (n_153_simd + n_1237_simd),
    ]
}

// ===== SIMD VERSION (x4 lanes) =====

#[allow(unused_parens)]
#[rustfmt::skip]
pub fn generated_last_simd_x4(input: &[[u32; 16]; 4]) -> [u64x4; 5] {
    // Load all inputs into SIMD registers
    let input_0_simd = u64x4::from_array([input[0][0] as u64, input[1][0] as u64, input[2][0] as u64, input[3][0] as u64]);
    let input_1_simd = u64x4::from_array([input[0][1] as u64, input[1][1] as u64, input[2][1] as u64, input[3][1] as u64]);
    let input_2_simd = u64x4::from_array([input[0][2] as u64, input[1][2] as u64, input[2][2] as u64, input[3][2] as u64]);
    let input_3_simd = u64x4::from_array([input[0][3] as u64, input[1][3] as u64, input[2][3] as u64, input[3][3] as u64]);
    let input_4_simd = u64x4::from_array([input[0][4] as u64, input[1][4] as u64, input[2][4] as u64, input[3][4] as u64]);
    let input_5_simd = u64x4::from_array([input[0][5] as u64, input[1][5] as u64, input[2][5] as u64, input[3][5] as u64]);
    let input_6_simd = u64x4::from_array([input[0][6] as u64, input[1][6] as u64, input[2][6] as u64, input[3][6] as u64]);
    let input_7_simd = u64x4::from_array([input[0][7] as u64, input[1][7] as u64, input[2][7] as u64, input[3][7] as u64]);
    let input_8_simd = u64x4::from_array([input[0][8] as u64, input[1][8] as u64, input[2][8] as u64, input[3][8] as u64]);
    let input_9_simd = u64x4::from_array([input[0][9] as u64, input[1][9] as u64, input[2][9] as u64, input[3][9] as u64]);
    let input_10_simd = u64x4::from_array([input[0][10] as u64, input[1][10] as u64, input[2][10] as u64, input[3][10] as u64]);
    let input_11_simd = u64x4::from_array([input[0][11] as u64, input[1][11] as u64, input[2][11] as u64, input[3][11] as u64]);
    let input_12_simd = u64x4::from_array([input[0][12] as u64, input[1][12] as u64, input[2][12] as u64, input[3][12] as u64]);
    let input_13_simd = u64x4::from_array([input[0][13] as u64, input[1][13] as u64, input[2][13] as u64, input[3][13] as u64]);
    let input_14_simd = u64x4::from_array([input[0][14] as u64, input[1][14] as u64, input[2][14] as u64, input[3][14] as u64]);
    let input_15_simd = u64x4::from_array([input[0][15] as u64, input[1][15] as u64, input[2][15] as u64, input[3][15] as u64]);

    // layer 0
    let n_161_simd = (input_1_simd - input_9_simd);
    let n_163_simd = (input_3_simd - input_11_simd);
    let n_165_simd = (input_5_simd - input_13_simd);
    let n_167_simd = (input_7_simd - input_15_simd);
    // layer 1
    let n_177_simd = (n_161_simd + n_165_simd);
    let n_179_simd = (n_163_simd + n_167_simd);
    // layer 2
    let n_34_simd = (input_0_simd + input_8_simd);
    let n_35_simd = (input_1_simd + input_9_simd);
    let n_36_simd = (input_2_simd + input_10_simd);
    let n_37_simd = (input_3_simd + input_11_simd);
    let n_38_simd = (input_4_simd + input_12_simd);
    let n_39_simd = (input_5_simd + input_13_simd);
    let n_40_simd = (input_6_simd + input_14_simd);
    let n_41_simd = (input_7_simd + input_15_simd);
    let n_185_simd = (n_161_simd + n_163_simd);
    let n_271_simd = (n_177_simd + n_179_simd);
    // layer 3
    let n_90_simd = (n_34_simd - n_38_simd);
    let n_91_simd = (n_35_simd - n_39_simd);
    let n_92_simd = (n_36_simd - n_40_simd);
    let n_93_simd = (n_37_simd - n_41_simd);
    let n_162_simd = (input_2_simd - input_10_simd);
    let n_164_simd = (input_4_simd - input_12_simd);
    let n_166_simd = (input_6_simd - input_14_simd);
    let n_228_simd = (n_165_simd + n_167_simd);
    let n_1885_simd = (n_271_simd * u64x4::splat(0xfffffffffff931e8u64));
    let n_1891_simd = (n_177_simd * u64x4::splat(0xfffffffffffac4b0u64));
    let n_1909_simd = (n_185_simd * u64x4::splat(0xfffffffffffbe968u64));
    let n_1915_simd = (n_161_simd * u64x4::splat(0xfffffffffffcc698u64));
    // layer 4
    let n_50_simd = (n_34_simd + n_38_simd);
    let n_51_simd = (n_35_simd + n_39_simd);
    let n_52_simd = (n_36_simd + n_40_simd);
    let n_53_simd = (n_37_simd + n_41_simd);
    let n_98_simd = (n_90_simd + n_92_simd);
    let n_99_simd = (n_91_simd + n_93_simd);
    let n_160_simd = (input_0_simd - input_8_simd);
    let n_178_simd = (n_162_simd + n_166_simd);
    let n_227_simd = (n_164_simd + n_166_simd);
    let n_651_simd = (n_1885_simd - n_1891_simd);
    let n_1360_simd = (n_1909_simd - n_1915_simd);
    let n_1897_simd = (n_179_simd * u64x4::splat(0xfffffffffffe6d38u64));
    let n_1921_simd = (n_163_simd * u64x4::splat(0xffffffffffff22d0u64));
    let n_1933_simd = (n_228_simd * u64x4::splat(0xfffffffffffd4880u64));
    let n_1939_simd = (n_165_simd * u64x4::splat(0xfffffffffffdfe18u64));
    // layer 5
    let n_58_simd = (n_50_simd + n_52_simd);
    let n_59_simd = (n_51_simd + n_53_simd);
    let n_176_simd = (n_160_simd + n_164_simd);
    let n_184_simd = (n_160_simd + n_162_simd);
    let n_609_simd = (n_651_simd - n_1897_simd);
    let n_1321_simd = (n_1933_simd - n_1939_simd);
    let n_1351_simd = (n_1360_simd - n_1921_simd);
    let n_1861_simd = (n_99_simd * u64x4::splat(0xfffffffffffe33b4u64));
    let n_1865_simd = (n_91_simd * u64x4::splat(0xfffffffffffb7700u64));
    let n_1903_simd = (n_178_simd * u64x4::splat(0x1c410u64));
    let n_1927_simd = (n_162_simd * u64x4::splat(0xfffffffffffe7638u64));
    let n_1945_simd = (n_167_simd * u64x4::splat(0xffffffffffff4a68u64));
    let n_1976_simd = (n_178_simd * u64x4::splat(0xfffffffffffe6d38u64));
    let n_1979_simd = (n_179_simd * u64x4::splat(0x1c410u64));
    let n_1982_simd = (n_162_simd * u64x4::splat(0xffffffffffff22d0u64));
    let n_1985_simd = (n_163_simd * u64x4::splat(0xfffffffffffe7638u64));
    let n_2001_simd = (n_98_simd * u64x4::splat(0x563f0u64));
    let n_2013_simd = (n_227_simd * u64x4::splat(0x2bf20u64));
    // layer 6
    let n_62_simd = (n_58_simd + n_59_simd);
    let n_65_simd = (n_58_simd - n_59_simd);
    let n_71_simd = (n_50_simd - n_52_simd);
    let n_72_simd = (n_51_simd - n_53_simd);
    let n_485_simd = (n_1861_simd - n_1865_simd);
    let n_570_simd = (n_609_simd + n_1903_simd);
    let n_970_simd = (n_1865_simd + n_2001_simd);
    let n_1300_simd = (n_1321_simd - n_1945_simd);
    let n_1330_simd = (n_1351_simd + n_1927_simd);
    let n_1591_simd = (n_1939_simd + n_2013_simd);
    let n_1729_simd = (n_1982_simd + n_1985_simd);
    let n_1756_simd = (n_1976_simd + n_1979_simd);
    let n_1857_simd = (n_90_simd * u64x4::splat(0x608f8u64));
    let n_1869_simd = (n_93_simd * u64x4::splat(0x2bcb4u64));
    let n_1951_simd = (n_166_simd * u64x4::splat(0x34dd8u64));
    let n_1957_simd = (n_164_simd * u64x4::splat(0xffffffffffff7148u64));
    let n_1961_simd = (n_90_simd * u64x4::splat(0xfffffffffffb7700u64));
    let n_1963_simd = (n_91_simd * u64x4::splat(0x608f8u64));
    let n_1970_simd = (n_160_simd * u64x4::splat(0xfffffffffffcc698u64));
    let n_1973_simd = (n_161_simd * u64x4::splat(0x8b18u64));
    let n_1988_simd = (n_166_simd * u64x4::splat(0xffffffffffff4a68u64));
    let n_1991_simd = (n_167_simd * u64x4::splat(0x34dd8u64));
    let n_1994_simd = (n_164_simd * u64x4::splat(0xfffffffffffdfe18u64));
    let n_1997_simd = (n_165_simd * u64x4::splat(0xffffffffffff7148u64));
    let n_2007_simd = (n_184_simd * u64x4::splat(0xffffffffffff0150u64));
    let n_2015_simd = (n_98_simd * u64x4::splat(0xfffffffffffe33b4u64));
    let n_2017_simd = (n_99_simd * u64x4::splat(0x563f0u64));
    let n_2020_simd = (n_184_simd * u64x4::splat(0xfffffffffffbe968u64));
    let n_2023_simd = (n_185_simd * u64x4::splat(0xffffffffffff0150u64));
    let n_2026_simd = (n_227_simd * u64x4::splat(0xfffffffffffd4880u64));
    let n_2029_simd = (n_228_simd * u64x4::splat(0x2bf20u64));
    let n_2035_simd = (n_176_simd * u64x4::splat(0xfffffffffffffc60u64));
    // layer 7
    let n_64_simd = (n_62_simd * u64x4::splat(0x801d5u64));
    let n_67_simd = (n_65_simd * u64x4::splat(0xcccbu64));
    let n_454_simd = (n_485_simd - n_1869_simd);
    let n_504_simd = (n_570_simd - n_1330_simd);
    let n_801_simd = (n_1756_simd - n_1729_simd);
    let n_937_simd = (n_970_simd - n_1857_simd);
    let n_988_simd = (n_1897_simd - n_1921_simd);
    let n_1116_simd = (n_1961_simd + n_1963_simd);
    let n_1140_simd = (n_2015_simd + n_2017_simd);
    let n_1276_simd = (n_1300_simd + n_1951_simd);
    let n_1285_simd = (n_1330_simd + n_2035_simd);
    let n_1558_simd = (n_1591_simd - n_1957_simd);
    let n_1615_simd = (n_1915_simd + n_2007_simd);
    let n_1705_simd = (n_1988_simd + n_1991_simd);
    let n_1741_simd = (n_1994_simd + n_1997_simd);
    let n_1768_simd = (n_1970_simd + n_1973_simd);
    let n_1774_simd = (n_2026_simd + n_2029_simd);
    let n_1792_simd = (n_2020_simd + n_2023_simd);
    let n_1851_simd = (n_71_simd * u64x4::splat(0xffffffffffff9af0u64));
    let n_1853_simd = (n_72_simd * u64x4::splat(0xd29eu64));
    let n_1873_simd = (n_92_simd * u64x4::splat(0xffffffffffff5af8u64));
    let n_1879_simd = (n_160_simd * u64x4::splat(0x8b18u64));
    let n_1958_simd = (n_71_simd * u64x4::splat(0xd29eu64));
    let n_1959_simd = (n_72_simd * u64x4::splat(0xffffffffffff9af0u64));
    let n_1965_simd = (n_92_simd * u64x4::splat(0x2bcb4u64));
    let n_1967_simd = (n_93_simd * u64x4::splat(0xffffffffffff5af8u64));
    // layer 8
    let n_69_simd = (n_64_simd + n_67_simd);
    let n_70_simd = (n_64_simd - n_67_simd);
    let n_397_simd = (n_1851_simd - n_1853_simd);
    let n_428_simd = (n_454_simd + n_1873_simd);
    let n_473_simd = (n_504_simd - n_1276_simd);
    let n_702_simd = (n_1958_simd + n_1959_simd);
    let n_769_simd = (n_801_simd - n_1705_simd);
    let n_912_simd = (n_937_simd - n_1873_simd);
    let n_955_simd = (n_988_simd - n_1945_simd);
    let n_1092_simd = (n_1140_simd - n_1116_simd);
    let n_1096_simd = (n_1965_simd + n_1967_simd);
    let n_1261_simd = (n_1285_simd - n_1879_simd);
    let n_1525_simd = (n_1558_simd - n_1951_simd);
    let n_1582_simd = (n_1615_simd - n_1879_simd);
    let n_1699_simd = (n_1774_simd - n_1741_simd);
    let n_1723_simd = (n_1792_simd - n_1768_simd);
    // layer 9
    let n_86_simd = (n_69_simd + n_397_simd);
    let n_87_simd = (n_69_simd - n_397_simd);
    let n_88_simd = (n_70_simd + n_702_simd);
    let n_89_simd = (n_70_simd - n_702_simd);
    let n_403_simd = (n_1857_simd - n_428_simd);
    let n_444_simd = (n_473_simd + n_1957_simd);
    let n_708_simd = (n_1116_simd - n_1096_simd);
    let n_742_simd = (n_769_simd + n_1741_simd);
    let n_897_simd = (n_912_simd - n_1869_simd);
    let n_931_simd = (n_955_simd + n_1525_simd);
    let n_1077_simd = (n_1092_simd - n_1096_simd);
    let n_1246_simd = (n_1261_simd - n_1957_simd);
    let n_1549_simd = (n_1582_simd - n_1927_simd);
    let n_1675_simd = (n_1699_simd - n_1705_simd);
    let n_1690_simd = (n_1723_simd - n_1729_simd);
    // layer 10
    let n_152_simd = (n_86_simd + n_403_simd);
    let n_153_simd = (n_86_simd - n_403_simd);
    let n_154_simd = (n_88_simd + n_708_simd);
    let n_156_simd = (n_87_simd + n_897_simd);
    let n_158_simd = (n_89_simd + n_1077_simd);
    let n_412_simd = (n_1879_simd - n_444_simd);
    let n_717_simd = (n_1768_simd - n_742_simd);
    let n_906_simd = (n_1549_simd - n_931_simd);
    let n_1086_simd = (n_1690_simd - n_1675_simd);
    let n_1237_simd = (n_1246_simd - n_1276_simd);
    // output
    [
        (n_152_simd + n_412_simd),
        (n_154_simd + n_717_simd),
        (n_156_simd + n_906_simd),
        (n_158_simd + n_1086_simd),
        (n_153_simd + n_1237_simd),
    ]
}

// ===== SIMD VERSION (x8 lanes) =====

#[allow(unused_parens)]
#[rustfmt::skip]
pub fn generated_last_simd_x8(input: &[[u32; 16]; 8]) -> [u64x8; 5] {
    // Load all inputs into SIMD registers
    let input_0_simd = u64x8::from_array([input[0][0] as u64, input[1][0] as u64, input[2][0] as u64, input[3][0] as u64, input[4][0] as u64, input[5][0] as u64, input[6][0] as u64, input[7][0] as u64]);
    let input_1_simd = u64x8::from_array([input[0][1] as u64, input[1][1] as u64, input[2][1] as u64, input[3][1] as u64, input[4][1] as u64, input[5][1] as u64, input[6][1] as u64, input[7][1] as u64]);
    let input_2_simd = u64x8::from_array([input[0][2] as u64, input[1][2] as u64, input[2][2] as u64, input[3][2] as u64, input[4][2] as u64, input[5][2] as u64, input[6][2] as u64, input[7][2] as u64]);
    let input_3_simd = u64x8::from_array([input[0][3] as u64, input[1][3] as u64, input[2][3] as u64, input[3][3] as u64, input[4][3] as u64, input[5][3] as u64, input[6][3] as u64, input[7][3] as u64]);
    let input_4_simd = u64x8::from_array([input[0][4] as u64, input[1][4] as u64, input[2][4] as u64, input[3][4] as u64, input[4][4] as u64, input[5][4] as u64, input[6][4] as u64, input[7][4] as u64]);
    let input_5_simd = u64x8::from_array([input[0][5] as u64, input[1][5] as u64, input[2][5] as u64, input[3][5] as u64, input[4][5] as u64, input[5][5] as u64, input[6][5] as u64, input[7][5] as u64]);
    let input_6_simd = u64x8::from_array([input[0][6] as u64, input[1][6] as u64, input[2][6] as u64, input[3][6] as u64, input[4][6] as u64, input[5][6] as u64, input[6][6] as u64, input[7][6] as u64]);
    let input_7_simd = u64x8::from_array([input[0][7] as u64, input[1][7] as u64, input[2][7] as u64, input[3][7] as u64, input[4][7] as u64, input[5][7] as u64, input[6][7] as u64, input[7][7] as u64]);
    let input_8_simd = u64x8::from_array([input[0][8] as u64, input[1][8] as u64, input[2][8] as u64, input[3][8] as u64, input[4][8] as u64, input[5][8] as u64, input[6][8] as u64, input[7][8] as u64]);
    let input_9_simd = u64x8::from_array([input[0][9] as u64, input[1][9] as u64, input[2][9] as u64, input[3][9] as u64, input[4][9] as u64, input[5][9] as u64, input[6][9] as u64, input[7][9] as u64]);
    let input_10_simd = u64x8::from_array([input[0][10] as u64, input[1][10] as u64, input[2][10] as u64, input[3][10] as u64, input[4][10] as u64, input[5][10] as u64, input[6][10] as u64, input[7][10] as u64]);
    let input_11_simd = u64x8::from_array([input[0][11] as u64, input[1][11] as u64, input[2][11] as u64, input[3][11] as u64, input[4][11] as u64, input[5][11] as u64, input[6][11] as u64, input[7][11] as u64]);
    let input_12_simd = u64x8::from_array([input[0][12] as u64, input[1][12] as u64, input[2][12] as u64, input[3][12] as u64, input[4][12] as u64, input[5][12] as u64, input[6][12] as u64, input[7][12] as u64]);
    let input_13_simd = u64x8::from_array([input[0][13] as u64, input[1][13] as u64, input[2][13] as u64, input[3][13] as u64, input[4][13] as u64, input[5][13] as u64, input[6][13] as u64, input[7][13] as u64]);
    let input_14_simd = u64x8::from_array([input[0][14] as u64, input[1][14] as u64, input[2][14] as u64, input[3][14] as u64, input[4][14] as u64, input[5][14] as u64, input[6][14] as u64, input[7][14] as u64]);
    let input_15_simd = u64x8::from_array([input[0][15] as u64, input[1][15] as u64, input[2][15] as u64, input[3][15] as u64, input[4][15] as u64, input[5][15] as u64, input[6][15] as u64, input[7][15] as u64]);

    // layer 0
    let n_161_simd = (input_1_simd - input_9_simd);
    let n_163_simd = (input_3_simd - input_11_simd);
    let n_165_simd = (input_5_simd - input_13_simd);
    let n_167_simd = (input_7_simd - input_15_simd);
    // layer 1
    let n_177_simd = (n_161_simd + n_165_simd);
    let n_179_simd = (n_163_simd + n_167_simd);
    // layer 2
    let n_34_simd = (input_0_simd + input_8_simd);
    let n_35_simd = (input_1_simd + input_9_simd);
    let n_36_simd = (input_2_simd + input_10_simd);
    let n_37_simd = (input_3_simd + input_11_simd);
    let n_38_simd = (input_4_simd + input_12_simd);
    let n_39_simd = (input_5_simd + input_13_simd);
    let n_40_simd = (input_6_simd + input_14_simd);
    let n_41_simd = (input_7_simd + input_15_simd);
    let n_185_simd = (n_161_simd + n_163_simd);
    let n_271_simd = (n_177_simd + n_179_simd);
    // layer 3
    let n_90_simd = (n_34_simd - n_38_simd);
    let n_91_simd = (n_35_simd - n_39_simd);
    let n_92_simd = (n_36_simd - n_40_simd);
    let n_93_simd = (n_37_simd - n_41_simd);
    let n_162_simd = (input_2_simd - input_10_simd);
    let n_164_simd = (input_4_simd - input_12_simd);
    let n_166_simd = (input_6_simd - input_14_simd);
    let n_228_simd = (n_165_simd + n_167_simd);
    let n_1885_simd = (n_271_simd * u64x8::splat(0xfffffffffff931e8u64));
    let n_1891_simd = (n_177_simd * u64x8::splat(0xfffffffffffac4b0u64));
    let n_1909_simd = (n_185_simd * u64x8::splat(0xfffffffffffbe968u64));
    let n_1915_simd = (n_161_simd * u64x8::splat(0xfffffffffffcc698u64));
    // layer 4
    let n_50_simd = (n_34_simd + n_38_simd);
    let n_51_simd = (n_35_simd + n_39_simd);
    let n_52_simd = (n_36_simd + n_40_simd);
    let n_53_simd = (n_37_simd + n_41_simd);
    let n_98_simd = (n_90_simd + n_92_simd);
    let n_99_simd = (n_91_simd + n_93_simd);
    let n_160_simd = (input_0_simd - input_8_simd);
    let n_178_simd = (n_162_simd + n_166_simd);
    let n_227_simd = (n_164_simd + n_166_simd);
    let n_651_simd = (n_1885_simd - n_1891_simd);
    let n_1360_simd = (n_1909_simd - n_1915_simd);
    let n_1897_simd = (n_179_simd * u64x8::splat(0xfffffffffffe6d38u64));
    let n_1921_simd = (n_163_simd * u64x8::splat(0xffffffffffff22d0u64));
    let n_1933_simd = (n_228_simd * u64x8::splat(0xfffffffffffd4880u64));
    let n_1939_simd = (n_165_simd * u64x8::splat(0xfffffffffffdfe18u64));
    // layer 5
    let n_58_simd = (n_50_simd + n_52_simd);
    let n_59_simd = (n_51_simd + n_53_simd);
    let n_176_simd = (n_160_simd + n_164_simd);
    let n_184_simd = (n_160_simd + n_162_simd);
    let n_609_simd = (n_651_simd - n_1897_simd);
    let n_1321_simd = (n_1933_simd - n_1939_simd);
    let n_1351_simd = (n_1360_simd - n_1921_simd);
    let n_1861_simd = (n_99_simd * u64x8::splat(0xfffffffffffe33b4u64));
    let n_1865_simd = (n_91_simd * u64x8::splat(0xfffffffffffb7700u64));
    let n_1903_simd = (n_178_simd * u64x8::splat(0x1c410u64));
    let n_1927_simd = (n_162_simd * u64x8::splat(0xfffffffffffe7638u64));
    let n_1945_simd = (n_167_simd * u64x8::splat(0xffffffffffff4a68u64));
    let n_1976_simd = (n_178_simd * u64x8::splat(0xfffffffffffe6d38u64));
    let n_1979_simd = (n_179_simd * u64x8::splat(0x1c410u64));
    let n_1982_simd = (n_162_simd * u64x8::splat(0xffffffffffff22d0u64));
    let n_1985_simd = (n_163_simd * u64x8::splat(0xfffffffffffe7638u64));
    let n_2001_simd = (n_98_simd * u64x8::splat(0x563f0u64));
    let n_2013_simd = (n_227_simd * u64x8::splat(0x2bf20u64));
    // layer 6
    let n_62_simd = (n_58_simd + n_59_simd);
    let n_65_simd = (n_58_simd - n_59_simd);
    let n_71_simd = (n_50_simd - n_52_simd);
    let n_72_simd = (n_51_simd - n_53_simd);
    let n_485_simd = (n_1861_simd - n_1865_simd);
    let n_570_simd = (n_609_simd + n_1903_simd);
    let n_970_simd = (n_1865_simd + n_2001_simd);
    let n_1300_simd = (n_1321_simd - n_1945_simd);
    let n_1330_simd = (n_1351_simd + n_1927_simd);
    let n_1591_simd = (n_1939_simd + n_2013_simd);
    let n_1729_simd = (n_1982_simd + n_1985_simd);
    let n_1756_simd = (n_1976_simd + n_1979_simd);
    let n_1857_simd = (n_90_simd * u64x8::splat(0x608f8u64));
    let n_1869_simd = (n_93_simd * u64x8::splat(0x2bcb4u64));
    let n_1951_simd = (n_166_simd * u64x8::splat(0x34dd8u64));
    let n_1957_simd = (n_164_simd * u64x8::splat(0xffffffffffff7148u64));
    let n_1961_simd = (n_90_simd * u64x8::splat(0xfffffffffffb7700u64));
    let n_1963_simd = (n_91_simd * u64x8::splat(0x608f8u64));
    let n_1970_simd = (n_160_simd * u64x8::splat(0xfffffffffffcc698u64));
    let n_1973_simd = (n_161_simd * u64x8::splat(0x8b18u64));
    let n_1988_simd = (n_166_simd * u64x8::splat(0xffffffffffff4a68u64));
    let n_1991_simd = (n_167_simd * u64x8::splat(0x34dd8u64));
    let n_1994_simd = (n_164_simd * u64x8::splat(0xfffffffffffdfe18u64));
    let n_1997_simd = (n_165_simd * u64x8::splat(0xffffffffffff7148u64));
    let n_2007_simd = (n_184_simd * u64x8::splat(0xffffffffffff0150u64));
    let n_2015_simd = (n_98_simd * u64x8::splat(0xfffffffffffe33b4u64));
    let n_2017_simd = (n_99_simd * u64x8::splat(0x563f0u64));
    let n_2020_simd = (n_184_simd * u64x8::splat(0xfffffffffffbe968u64));
    let n_2023_simd = (n_185_simd * u64x8::splat(0xffffffffffff0150u64));
    let n_2026_simd = (n_227_simd * u64x8::splat(0xfffffffffffd4880u64));
    let n_2029_simd = (n_228_simd * u64x8::splat(0x2bf20u64));
    let n_2035_simd = (n_176_simd * u64x8::splat(0xfffffffffffffc60u64));
    // layer 7
    let n_64_simd = (n_62_simd * u64x8::splat(0x801d5u64));
    let n_67_simd = (n_65_simd * u64x8::splat(0xcccbu64));
    let n_454_simd = (n_485_simd - n_1869_simd);
    let n_504_simd = (n_570_simd - n_1330_simd);
    let n_801_simd = (n_1756_simd - n_1729_simd);
    let n_937_simd = (n_970_simd - n_1857_simd);
    let n_988_simd = (n_1897_simd - n_1921_simd);
    let n_1116_simd = (n_1961_simd + n_1963_simd);
    let n_1140_simd = (n_2015_simd + n_2017_simd);
    let n_1276_simd = (n_1300_simd + n_1951_simd);
    let n_1285_simd = (n_1330_simd + n_2035_simd);
    let n_1558_simd = (n_1591_simd - n_1957_simd);
    let n_1615_simd = (n_1915_simd + n_2007_simd);
    let n_1705_simd = (n_1988_simd + n_1991_simd);
    let n_1741_simd = (n_1994_simd + n_1997_simd);
    let n_1768_simd = (n_1970_simd + n_1973_simd);
    let n_1774_simd = (n_2026_simd + n_2029_simd);
    let n_1792_simd = (n_2020_simd + n_2023_simd);
    let n_1851_simd = (n_71_simd * u64x8::splat(0xffffffffffff9af0u64));
    let n_1853_simd = (n_72_simd * u64x8::splat(0xd29eu64));
    let n_1873_simd = (n_92_simd * u64x8::splat(0xffffffffffff5af8u64));
    let n_1879_simd = (n_160_simd * u64x8::splat(0x8b18u64));
    let n_1958_simd = (n_71_simd * u64x8::splat(0xd29eu64));
    let n_1959_simd = (n_72_simd * u64x8::splat(0xffffffffffff9af0u64));
    let n_1965_simd = (n_92_simd * u64x8::splat(0x2bcb4u64));
    let n_1967_simd = (n_93_simd * u64x8::splat(0xffffffffffff5af8u64));
    // layer 8
    let n_69_simd = (n_64_simd + n_67_simd);
    let n_70_simd = (n_64_simd - n_67_simd);
    let n_397_simd = (n_1851_simd - n_1853_simd);
    let n_428_simd = (n_454_simd + n_1873_simd);
    let n_473_simd = (n_504_simd - n_1276_simd);
    let n_702_simd = (n_1958_simd + n_1959_simd);
    let n_769_simd = (n_801_simd - n_1705_simd);
    let n_912_simd = (n_937_simd - n_1873_simd);
    let n_955_simd = (n_988_simd - n_1945_simd);
    let n_1092_simd = (n_1140_simd - n_1116_simd);
    let n_1096_simd = (n_1965_simd + n_1967_simd);
    let n_1261_simd = (n_1285_simd - n_1879_simd);
    let n_1525_simd = (n_1558_simd - n_1951_simd);
    let n_1582_simd = (n_1615_simd - n_1879_simd);
    let n_1699_simd = (n_1774_simd - n_1741_simd);
    let n_1723_simd = (n_1792_simd - n_1768_simd);
    // layer 9
    let n_86_simd = (n_69_simd + n_397_simd);
    let n_87_simd = (n_69_simd - n_397_simd);
    let n_88_simd = (n_70_simd + n_702_simd);
    let n_89_simd = (n_70_simd - n_702_simd);
    let n_403_simd = (n_1857_simd - n_428_simd);
    let n_444_simd = (n_473_simd + n_1957_simd);
    let n_708_simd = (n_1116_simd - n_1096_simd);
    let n_742_simd = (n_769_simd + n_1741_simd);
    let n_897_simd = (n_912_simd - n_1869_simd);
    let n_931_simd = (n_955_simd + n_1525_simd);
    let n_1077_simd = (n_1092_simd - n_1096_simd);
    let n_1246_simd = (n_1261_simd - n_1957_simd);
    let n_1549_simd = (n_1582_simd - n_1927_simd);
    let n_1675_simd = (n_1699_simd - n_1705_simd);
    let n_1690_simd = (n_1723_simd - n_1729_simd);
    // layer 10
    let n_152_simd = (n_86_simd + n_403_simd);
    let n_153_simd = (n_86_simd - n_403_simd);
    let n_154_simd = (n_88_simd + n_708_simd);
    let n_156_simd = (n_87_simd + n_897_simd);
    let n_158_simd = (n_89_simd + n_1077_simd);
    let n_412_simd = (n_1879_simd - n_444_simd);
    let n_717_simd = (n_1768_simd - n_742_simd);
    let n_906_simd = (n_1549_simd - n_931_simd);
    let n_1086_simd = (n_1690_simd - n_1675_simd);
    let n_1237_simd = (n_1246_simd - n_1276_simd);
    // output
    [
        (n_152_simd + n_412_simd),
        (n_154_simd + n_717_simd),
        (n_156_simd + n_906_simd),
        (n_158_simd + n_1086_simd),
        (n_153_simd + n_1237_simd),
    ]
}
