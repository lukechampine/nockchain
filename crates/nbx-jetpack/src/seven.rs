use zkvm_jetpack::form::mary::MarySlice;

use crate::utils::xeb;

pub const fn height_mary(p: MarySlice) -> u32 {
    // |=  p=mary
    // ^-  @
    // ~+
    // =/  len  len.array.p
    // ?:  =(len 0)  0
    // (bex (xeb (dec len)))
    1 << xeb((p.len - 1) as usize)
}
