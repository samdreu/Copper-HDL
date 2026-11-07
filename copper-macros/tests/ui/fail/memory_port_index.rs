use copper_core::{Bits, Clock, ClockDomain, Memory};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail (SPEC I.14.6(c), D-19 6b-A; Phase C step 8): a memory with one read port
// has no `read_port::<1>()`. The port index is known at the instance, so it is a shape
// rule, refused by both front-ends with one message
// (`copper_analysis::check_memory_port_index`); before, the simulator panicked at run
// time indexing its port array.
#[hardware(sequential)]
async fn memory_port_index(
    clk: Clock<MainClk>,
    a: In<Bits<2>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mem = Memory::<Bits<8>, 1, 1, MainClk, 1, 1>::new(clk.clone(), 4);
    loop {
        if mem.read_port::<0>().is_ready() {
            q.write(mem.read_port::<0>().data());
        }
        mem.read_port::<1>().read(a.read().as_usize());
        clk.tick().await;
    }
}

fn main() {}
