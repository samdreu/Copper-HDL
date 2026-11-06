# Semantics review — the 2026-11-03 baseline

Three read-only inventories taken on 2026-11-03 at commit `ea54d26` (branch `paper`),
before the specification in `../SPEC.md` was started. They are the baseline the review
runs against: every clause packet in `../SPEC_DECISIONS.md` cites them by their item
numbers, and the conformance ledger in `../SPEC.md` is built from them.

| file | what it inventories |
|---|---|
| `INVENTORY_DOCS.md` | every semantic clause stated in `design_docs/*.md` (254), the contradictions between documents (28), and the gaps the documents raise but do not answer |
| `INVENTORY_CODE.md` | every point where the implementation decides meaning (~200 behaviors, by file:line), and the 36 that look like semantic decisions no document states |
| `INVENTORY_RULES.md` | the 16 refusal rules as the code implements them, the probe battery's verdict ledger (159 modules), the `build.rs` tables, and every open or pending item found by grep |
| `MEASUREMENTS.md` | the measurements Phase A owed before the first clause packet: the `probe` fixture, the pre-loop-statement extraction question, and the anchor-debt list |

They are snapshots. They are not updated as the review proceeds; `SPEC.md` and the
decision log are.
