// Independent hardware reference for `tests/fixtures/write_first_ram_dut.rs::ram_read_first`.
//
// PROVENANCE: hand-written for this repository. It is therefore evidence that the
// emitted SystemVerilog does what a hardware engineer would write by hand for a
// read-first 1r1w synchronous RAM — a different and weaker claim than the
// third-party anchoring in `examples/basejump/sv/`, which is independent of anyone
// here. A misconception shared between this file and the Copper module would
// survive. Read `bsg_mem_1r1w_sync` (BaseJump STL) if you want the stronger form.
//
// READ-FIRST is the whole point of the module under test: the read captures the
// value held BEFORE the edge, so a write to the same address on the same edge is
// not visible to it. In SystemVerilog that falls out of non-blocking assignment —
// both statements below read pre-edge state regardless of the order they appear
// in — which is precisely why a reference written this way is worth having: it
// arrives at the behaviour from the language's own semantics rather than from the
// transpiler's enable/address nets.
//
// POWER-ON STATE is stated, not assumed. Copper's simulator starts an unpreloaded
// `Memory` at `T::default()` (zero) and a registered port at the wire's zero, and
// since 2026-10-30 the transpiler states both in an `initial` block. This file does
// the same so the two agree under randomized power-on as well as under Verilator's
// default zero-fill (`COPPER_VERILATOR_X_INITIAL=1` randomizes it; without the
// blocks below the reference read random words while the simulator and the
// transpiled SV agreed on zero). A preloaded memory is a separate fixture.

module ram_read_first (
    input  logic       clk,
    input  logic [3:0] waddr,
    input  logic [7:0] wdata,
    input  logic       we,
    input  logic [3:0] raddr,
    output logic [7:0] data
);

    logic [7:0] mem [0:15];

    initial begin
        data = 8'd0;
        for (int i = 0; i < 16; i++) begin
            mem[i] = 8'd0;
        end
    end

    always_ff @(posedge clk) begin
        data <= mem[raddr];
        if (we) begin
            mem[waddr] <= wdata;
        end
    end

endmodule
