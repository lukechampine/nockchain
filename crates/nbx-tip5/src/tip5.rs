#[cfg(target_arch = "x86_64")]
pub mod avx512;
mod mds_generated;
pub mod scalar;
pub mod simd;
pub mod simd_x2;
pub mod simd_x8;
pub mod test_cases;

use crate::base::*;
use crate::melt::*;

pub const DIGEST_LENGTH: usize = 5;
pub const STATE_SIZE: usize = 16;
pub const NUM_SPLIT_AND_LOOKUP: usize = 4;
pub const LOG2_STATE_SIZE: usize = 4;
pub const CAPACITY: usize = 6;
pub const RATE: usize = 10;
pub const NUM_ROUNDS: usize = 7;
pub const R: u128 = 18446744073709551616;

pub const LOOKUP_TABLE: [u8; 256] = [
    0, 7, 26, 63, 124, 215, 85, 254, 214, 228, 45, 185, 140, 173, 33, 240, 29, 177, 176, 32, 8,
    110, 87, 202, 204, 99, 150, 106, 230, 14, 235, 128, 213, 239, 212, 138, 23, 130, 208, 6, 44,
    71, 93, 116, 146, 189, 251, 81, 199, 97, 38, 28, 73, 179, 95, 84, 152, 48, 35, 119, 49, 88,
    242, 3, 148, 169, 72, 120, 62, 161, 166, 83, 175, 191, 137, 19, 100, 129, 112, 55, 221, 102,
    218, 61, 151, 237, 68, 164, 17, 147, 46, 234, 203, 216, 22, 141, 65, 57, 123, 12, 244, 54, 219,
    231, 96, 77, 180, 154, 5, 253, 133, 165, 98, 195, 205, 134, 245, 30, 9, 188, 59, 142, 186, 197,
    181, 144, 92, 31, 224, 163, 111, 74, 58, 69, 113, 196, 67, 246, 225, 10, 121, 50, 60, 157, 90,
    122, 2, 250, 101, 75, 178, 159, 24, 36, 201, 11, 243, 132, 198, 190, 114, 233, 39, 52, 21, 209,
    108, 238, 91, 187, 18, 104, 194, 37, 153, 34, 200, 143, 126, 155, 236, 118, 64, 80, 172, 89,
    94, 193, 135, 183, 86, 107, 252, 13, 167, 206, 136, 220, 207, 103, 171, 160, 76, 182, 227, 217,
    158, 56, 174, 4, 66, 109, 139, 162, 184, 211, 249, 47, 125, 232, 117, 43, 16, 42, 127, 20, 241,
    25, 149, 105, 156, 51, 53, 168, 145, 247, 223, 79, 78, 226, 15, 222, 82, 115, 70, 210, 27, 41,
    1, 170, 40, 131, 192, 229, 248, 255,
];

const ROUND_CONSTANTS: [u64; NUM_ROUNDS * STATE_SIZE] = [
    // 1st round constants
    1332676891236936200, 16607633045354064669, 12746538998793080786, 15240351333789289931,
    10333439796058208418, 986873372968378050, 153505017314310505, 703086547770691416,
    8522628845961587962, 1727254290898686320, 199492491401196126, 2969174933639985366,
    1607536590362293391, 16971515075282501568, 15401316942841283351, 14178982151025681389,
    // 2nd round constants
    2916963588744282587, 5474267501391258599, 5350367839445462659, 7436373192934779388,
    12563531800071493891, 12265318129758141428, 6524649031155262053, 1388069597090660214,
    3049665785814990091, 5225141380721656276, 10399487208361035835, 6576713996114457203,
    12913805829885867278, 10299910245954679423, 12980779960345402499, 593670858850716490,
    // 3rd round constants
    12184128243723146967, 1315341360419235257, 9107195871057030023, 4354141752578294067,
    8824457881527486794, 14811586928506712910, 7768837314956434138, 2807636171572954860,
    9487703495117094125, 13452575580428891895, 14689488045617615844, 16144091782672017853,
    15471922440568867245, 17295382518415944107, 15054306047726632486, 5708955503115886019,
    // 4th round constants
    9596017237020520842, 16520851172964236909, 8513472793890943175, 8503326067026609602,
    9402483918549940854, 8614816312698982446, 7744830563717871780, 14419404818700162041,
    8090742384565069824, 15547662568163517559, 17314710073626307254, 10008393716631058961,
    14480243402290327574, 13569194973291808551, 10573516815088946209, 15120483436559336219,
    // 5th round constants
    3515151310595301563, 1095382462248757907, 5323307938514209350, 14204542692543834582,
    12448773944668684656, 13967843398310696452, 14838288394107326806, 13718313940616442191,
    15032565440414177483, 13769903572116157488, 17074377440395071208, 16931086385239297738,
    8723550055169003617, 590842605971518043, 16642348030861036090, 10708719298241282592,
    // 6th round constants
    12766914315707517909, 11780889552403245587, 113183285481780712, 9019899125655375514,
    3300264967390964820, 12802381622653377935, 891063765000023873, 15939045541699412539,
    3240223189948727743, 4087221142360949772, 10980466041788253952, 18199914337033135244,
    7168108392363190150, 16860278046098150740, 13088202265571714855, 4712275036097525581,
    // 7th round constants
    16338034078141228133, 1455012125527134274, 5024057780895012002, 9289161311673217186,
    9401110072402537104, 11919498251456187748, 4173156070774045271, 15647643457869530627,
    15642078237964257476, 1405048341078324037, 3059193199283698832, 1605012781983592984,
    7134876918849821827, 5796994175286958720, 7251651436095127661, 4565856221886323991,
];

const MDS_MATRIX_I64: [[i64; STATE_SIZE]; STATE_SIZE] = [
    [
        61402, 17845, 26798, 59689, 12021, 40901, 41351, 27521, 56951, 12034, 53865, 43244, 7454,
        33823, 28750, 1108,
    ],
    [
        1108, 61402, 17845, 26798, 59689, 12021, 40901, 41351, 27521, 56951, 12034, 53865, 43244,
        7454, 33823, 28750,
    ],
    [
        28750, 1108, 61402, 17845, 26798, 59689, 12021, 40901, 41351, 27521, 56951, 12034, 53865,
        43244, 7454, 33823,
    ],
    [
        33823, 28750, 1108, 61402, 17845, 26798, 59689, 12021, 40901, 41351, 27521, 56951, 12034,
        53865, 43244, 7454,
    ],
    [
        7454, 33823, 28750, 1108, 61402, 17845, 26798, 59689, 12021, 40901, 41351, 27521, 56951,
        12034, 53865, 43244,
    ],
    [
        43244, 7454, 33823, 28750, 1108, 61402, 17845, 26798, 59689, 12021, 40901, 41351, 27521,
        56951, 12034, 53865,
    ],
    [
        53865, 43244, 7454, 33823, 28750, 1108, 61402, 17845, 26798, 59689, 12021, 40901, 41351,
        27521, 56951, 12034,
    ],
    [
        12034, 53865, 43244, 7454, 33823, 28750, 1108, 61402, 17845, 26798, 59689, 12021, 40901,
        41351, 27521, 56951,
    ],
    [
        56951, 12034, 53865, 43244, 7454, 33823, 28750, 1108, 61402, 17845, 26798, 59689, 12021,
        40901, 41351, 27521,
    ],
    [
        27521, 56951, 12034, 53865, 43244, 7454, 33823, 28750, 1108, 61402, 17845, 26798, 59689,
        12021, 40901, 41351,
    ],
    [
        41351, 27521, 56951, 12034, 53865, 43244, 7454, 33823, 28750, 1108, 61402, 17845, 26798,
        59689, 12021, 40901,
    ],
    [
        40901, 41351, 27521, 56951, 12034, 53865, 43244, 7454, 33823, 28750, 1108, 61402, 17845,
        26798, 59689, 12021,
    ],
    [
        12021, 40901, 41351, 27521, 56951, 12034, 53865, 43244, 7454, 33823, 28750, 1108, 61402,
        17845, 26798, 59689,
    ],
    [
        59689, 12021, 40901, 41351, 27521, 56951, 12034, 53865, 43244, 7454, 33823, 28750, 1108,
        61402, 17845, 26798,
    ],
    [
        26798, 59689, 12021, 40901, 41351, 27521, 56951, 12034, 53865, 43244, 7454, 33823, 28750,
        1108, 61402, 17845,
    ],
    [
        17845, 26798, 59689, 12021, 40901, 41351, 27521, 56951, 12034, 53865, 43244, 7454, 33823,
        28750, 1108, 61402,
    ],
];

pub const MDS_MATRIX_MONT: [[Melt; STATE_SIZE]; STATE_SIZE] = const {
    let mut i = 0;
    let mut ret = [[Melt(0); STATE_SIZE]; STATE_SIZE];
    while i < ret.len() {
        let ret = &mut ret[i];
        let mut j = 0;
        while j < ret.len() {
            ret[j] = Melt(montify(MDS_MATRIX_I64[i][j] as u64));
            j += 1;
        }
        i += 1;
    }
    ret
};

pub const ROUND_CONSTANTS2: [Melt; NUM_ROUNDS * STATE_SIZE] = const {
    let mut i = 0;
    let mut ret = [Melt(0); NUM_ROUNDS * STATE_SIZE];
    while i < ret.len() {
        ret[i] = Melt((((ROUND_CONSTANTS[i] as u128) * R) % PRIME_128) as u64);
        i += 1;
    }
    ret
};

pub const MELT_ONE_POW_7: Melt = {
    let s1 = 4294967295;
    let s2 = montiply_ser(s1, s1);
    let s4 = montiply_ser(s2, s2);
    Melt(montiply_ser(montiply_ser(s1, s2), s4))
};

pub fn permute(sponge: &mut [Melt; 16]) {
    #[cfg(target_arch = "x86_64")]
    {
        simd::permute(sponge);
        return;
    }

    scalar::permute(sponge);
}

pub fn permute_intermediate(sponge: &mut [Melt; 16]) {
    #[cfg(target_arch = "x86_64")]
    {
        simd::permute_intermediate(sponge);
        return;
    }

    scalar::permute_intermediate(sponge);
}

#[inline(always)]
pub fn permute_last(sponge: [Melt; 16]) -> [Melt; 5] {
    #[cfg(target_arch = "x86_64")]
    {
        return simd::permute_last(sponge);
    }

    scalar::permute_last(sponge)
}

#[inline(always)]
pub fn permute_fixed(input: &[Melt; 10]) -> [Melt; 5] {
    #[cfg(target_arch = "x86_64")]
    {
        return simd::permute_fixed(input);
    }

    scalar::permute_fixed(input)
}

#[inline(always)]
pub fn permute_fixed_x2(input: &[Melt; 10], other_input: &[Melt; 10]) -> ([Melt; 5], [Melt; 5]) {
    #[cfg(target_arch = "x86_64")]
    {
        return simd_x2::permute_fixed_x2(input, other_input);
    }

    (scalar::permute_fixed(input), scalar::permute_fixed(input))
}

#[inline(always)]
pub fn permute_intermediate_x2(input: &mut [Melt; 16], other_input: &mut [Melt; 16]) {
    #[cfg(target_arch = "x86_64")]
    {
        simd_x2::permute_intermediate_x2(input, other_input);
        return;
    }

    scalar::permute_intermediate(input);
    scalar::permute_intermediate(input);
}

#[inline(always)]
pub fn permute_last_x2(input: [Melt; 16], other_input: [Melt; 16]) -> ([Melt; 5], [Melt; 5]) {
    #[cfg(target_arch = "x86_64")]
    {
        return simd_x2::permute_last_x2(input, other_input);
    }

    (
        scalar::permute_last(input),
        scalar::permute_last(other_input),
    )
}

#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx512f",
    target_feature = "avx512bw",
    target_feature = "avx512vl"
))]
mod tests {
    use super::*;
    use crate::tip5::test_cases::INSTANCES;

    #[test]
    fn test_avx512_permute() {
        for input in INSTANCES {
            let mut scalar_sponge = input;
            let mut avx512_sponge = input;
            scalar::permute(&mut scalar_sponge);
            unsafe {
                avx512::permute(&mut avx512_sponge);
            }

            assert_eq!(avx512_sponge, scalar_sponge);
            assert_ne!(avx512_sponge, input);
        }
    }

    #[test]
    fn test_avx512_last() {
        for input in INSTANCES {
            let scalar_result = scalar::permute_last(input);
            let avx512_result = unsafe { avx512::permute_last(input) };

            assert_eq!(scalar_result, avx512_result);
        }
    }

    #[test]
    fn test_avx512_fixed() {
        for input in INSTANCES {
            let input = input[..10].try_into().unwrap();

            let scalar_result = scalar::permute_fixed(input);
            let avx512_result = unsafe { avx512::permute_fixed(input) };

            assert_eq!(scalar_result, avx512_result);
        }
    }
}

mod test_simd {
    use super::*;
    use crate::tip5::test_cases::{
        get_fixed_instance, get_fixed_instances_x2, get_fixed_instances_x8, get_instance,
        get_instances_x2, get_instances_x8, INSTANCES,
    };

    #[test]
    fn test_permute_fixed() {
        for index in 0..INSTANCES.len() {
            let input = get_fixed_instance(index);

            let simd = simd::permute_fixed(&input);
            assert_eq!(simd, scalar::permute_fixed(&input));
        }
    }

    #[test]
    fn test_permute_intermediate() {
        for index in 0..INSTANCES.len() {
            let mut simd_input = get_instance(index);
            simd::permute_intermediate(&mut simd_input);

            let mut scalar_input = get_instance(index);
            scalar::permute_intermediate(&mut scalar_input);

            assert_eq!(simd_input, scalar_input);
        }
    }

    #[test]
    fn test_permute_fixed_x2() {
        for index in 0..INSTANCES.len() {
            let (a, b) = get_fixed_instances_x2(index);

            let (simd_a, simd_b) = simd_x2::permute_fixed_x2(&a, &b);
            assert_eq!(simd_a, scalar::permute_fixed(&a));
            assert_eq!(simd_b, scalar::permute_fixed(&b));
        }
    }

    #[test]
    fn test_permute_fixed_x8() {
        for index in 0..INSTANCES.len() {
            let input = get_fixed_instances_x8(index);

            let simd_result = simd_x8::permute_fixed_x8(input);

            for (i, input) in input.iter().enumerate() {
                let scalar_result = scalar::permute_fixed(input);
                assert_eq!(simd_result[i], scalar_result);
            }
        }
    }

    #[test]
    fn test_permute_intermediate_x2() {
        for index in 0..INSTANCES.len() {
            let (mut a, mut b) = get_instances_x2(index);
            simd_x2::permute_intermediate_x2(&mut a, &mut b);

            let (mut scalar_a, mut scalar_b) = get_instances_x2(index);
            scalar::permute_intermediate(&mut scalar_a);
            scalar::permute_intermediate(&mut scalar_b);

            assert_eq!(a, scalar_a);
            assert_eq!(b, scalar_b);
        }
    }

    #[test]
    fn test_permute_intermediate_x8() {
        for index in 0..INSTANCES.len() {
            let mut simd = get_instances_x8(index);
            simd_x8::permute_intermediate_x8(&mut simd);

            for (i, scalar) in get_instances_x8(index).iter_mut().enumerate() {
                scalar::permute_intermediate(scalar);
                assert_eq!(simd[i], *scalar);
            }
        }
    }

    #[test]
    fn test_permute_last_x2() {
        for index in 0..INSTANCES.len() {
            let (a, b) = get_instances_x2(index);

            let (simd_a, simd_b) = simd_x2::permute_last_x2(a, b);
            assert_eq!(simd_a, scalar::permute_last(a));
            assert_eq!(simd_b, scalar::permute_last(b));
        }
    }

    #[test]
    fn test_permute_last_x8() {
        for index in 0..INSTANCES.len() {
            let input = get_instances_x8(index);

            let result = simd_x8::permute_last_x8(input);

            for (index, input) in input.into_iter().enumerate() {
                assert_eq!(result[index], scalar::permute_last(input));
            }
        }
    }
}
