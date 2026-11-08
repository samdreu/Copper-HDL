// `()` is not a clock domain (SPEC I.12.6 (4), D-17 A1; D-22, Phase C step 11): a port
// type bounds its domain `D: ClockDomain` and has no default, so a testbench wire in
// the unit domain is a type error. On a `#[hardware]` module the shared front-end rule
// refuses it first (`copper-macros/tests/ui/fail/unit_domain_port.rs`).

use copper_core::port::wire;
use copper_core::Logic;

fn main() {
    let (_out, _in) = wire::<Logic, ()>(Logic::Zero);
}
