# Semantics review — the baseline, the packets and the log

The review that produced `../SPEC.md` (normative since 2026-11-09, D-31). Three read-only
inventories were taken on 2026-11-03 at commit `ea54d26` (branch `paper`), before the
specification was started. Every clause packet in `../SPEC_DECISIONS.md` cites them by item
number. The conformance ledger is SPEC §III.3 itself (`tests/conformance_ledger.rs` reads
only that); the inventories were the baseline it was written against.

| file | what it holds | kind |
|---|---|---|
| `INVENTORY_DOCS.md` | every semantic clause stated in `design_docs/*.md` (254), the contradictions between documents (28), and the gaps the documents raise but do not answer | snapshot, 2026-11-03 |
| `INVENTORY_CODE.md` | every point where the implementation decides meaning (~200 behaviors, by file:line), and the 36 that look like semantic decisions no document states | snapshot, 2026-11-03 |
| `INVENTORY_RULES.md` | the 16 refusal rules as the code implemented them, the probe battery's verdict ledger (159 modules), the `build.rs` tables, and every open or pending item found by grep | snapshot, 2026-11-03 |
| `DECISION_AUDIT.md` | D-1…D-7 sorted against `SPEC_OVERVIEW.md`, and re-read after D-8 | record, 2026-11-04/05 |
| `Q1_TRADEOFFS.md` | the two-jobs-or-one packet that D-8 decided | record, 2026-11-04 |
| `MEASUREMENTS.md` | the measurement log: Phase A's owed measurements (§A–§C), then every Phase C step and later batch, dated (§D–§AH) | append-only log |
| `DOCS_AUDIT.md` | the 2026-11-09 audit of every doc against the tree, with what phase 2 changed | record, 2026-11-09 |

The snapshots are not updated; `SPEC.md`, the decision log and `MEASUREMENTS.md` are.
