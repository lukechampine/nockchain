#[allow(unused_parens)]
// generated with mds-codegen
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

#[allow(unused_parens)]
// Adapted from the generated function and removed all calculations related to
//  the first 10 output variables.
pub const fn generated_intermediate(input: &[u32; 16]) -> [u64; 6] {
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
    let n_702 = n_1958.wrapping_add(n_1959);
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
    let n_708 = n_1116.wrapping_sub(n_1096);
    let n_897 = n_912.wrapping_sub(n_1869);
    let n_931 = n_955.wrapping_add(n_1525);
    let n_1077 = n_1092.wrapping_sub(n_1096);
    let n_1246 = n_1261.wrapping_sub(n_1957);
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

#[allow(unused_parens)]
// Adapted from the generated function and removed all calculations related to
//  the last 11 output variables.
pub const fn generated_last(input: &[u32; 16]) -> [u64; 5] {
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
    let n_1582 = n_1615.wrapping_sub(n_1879);
    let n_1591 = n_1939.wrapping_add(n_2013);
    let n_1729 = n_1982.wrapping_add(n_1985);
    let n_1756 = n_1976.wrapping_add(n_1979);
    let n_1768 = n_1970.wrapping_add(n_1973);
    let n_1792 = n_2020.wrapping_add(n_2023);
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
    let n_1549 = n_1582.wrapping_sub(n_1927);
    let n_1558 = n_1591.wrapping_sub(n_1957);
    let n_1705 = n_1988.wrapping_add(n_1991);
    let n_1723 = n_1792.wrapping_sub(n_1768);
    let n_1741 = n_1994.wrapping_add(n_1997);
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
    let n_1525 = n_1558.wrapping_sub(n_1951);
    let n_1690 = n_1723.wrapping_sub(n_1729);
    let n_1699 = n_1774.wrapping_sub(n_1741);
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
    let n_1675 = n_1699.wrapping_sub(n_1705);
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
