use copper_core::ClockDomain;
use copper_core::port::{In, Out};
use copper_macros::hardware;

// Should fail (SPEC I.14.3(c), D-32 (2)): `to_le` is the identity on a little-endian host and
// `swap_bytes` on a big-endian one; SystemVerilog has no byte order. Refused by both front-ends
// with one message (`copper_analysis::check_target_byte_order`), in a combinational body too.
#[hardware(combinational)]
fn target_byte_order_to_le<D: ClockDomain>(a: In<u16, D>, q: Out<u16, D>) {
    q.write(a.read().to_le());
}

fn main() {}
