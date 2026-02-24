#![no_main]

use libfuzzer_sys::fuzz_target;
use rust_mule::kad::packed::inflate_zlib;

fuzz_target!(|data: &[u8]| {
    // Keep output cap modest to avoid memory pressure during mutation.
    let _ = inflate_zlib(data, 256 * 1024);
});
