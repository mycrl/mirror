#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::approx_constant)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::redundant_static_lifetimes)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

#[cfg(any(
    feature = "avcodec",
    feature = "avdevice",
    feature = "avfilter",
    feature = "avformat",
    feature = "avutil",
    feature = "swresample",
    feature = "swresample",
    feature = "swscale"
))]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use std::ffi::c_int;

#[inline(always)]
pub unsafe fn av_make_q(num: c_int, den: c_int) -> AVRational {
    AVRational { num, den }
}
