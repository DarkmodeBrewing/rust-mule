#![no_main]

use libfuzzer_sys::fuzz_target;
use rust_mule::kad::wire::{
    KadPacket, decode_kad2_bootstrap_res, decode_kad2_publish_key_req,
    decode_kad2_publish_key_req_lenient, decode_kad2_req, decode_kad2_res, decode_kad2_search_res,
    decode_kad2_search_key_req, decode_kad2_search_source_req,
};

fuzz_target!(|data: &[u8]| {
    let _ = KadPacket::decode(data).map(|pkt| {
        let p = pkt.payload.as_slice();
        let _ = decode_kad2_bootstrap_res(p);
        let _ = decode_kad2_req(p);
        let _ = decode_kad2_res(p);
        let _ = decode_kad2_search_source_req(p);
        let _ = decode_kad2_search_key_req(p);
        let _ = decode_kad2_publish_key_req(p);
        let _ = decode_kad2_publish_key_req_lenient(p);
        let _ = decode_kad2_search_res(p);
    });
});
