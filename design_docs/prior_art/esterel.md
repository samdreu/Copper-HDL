# Esterel (Berry & Gonthier, Sci. Comput. Program. 19(2):87–152, 1992) — how the paper specifies semantics

Source: `paper/references/esterel_scp1992.pdf` (scan; journal page N is PDF page
N−86). Section numbers are the paper's; page numbers are journal pages. Rules
were checked against the typeset PDF pages (116–122, 127–137), not the OCR.

## Headline

The paper gives **three** things for one kernel language ("basic Esterel"):
(1) an *intuitive* semantics in prose by "instruction duration" (§3.3);
(2) a **behavioral semantics** — Plotkin-style structural rules, big-step per
reaction, labelled transition over (statement, memory) pairs, with a
**non-effective fixpoint** in the local-signal rule (§6); this is declared the
formal definition ("any operational semantics should agree with it as far as
input-output behavior is concerned", §6); (3) an **execution (computational)
semantics** — small-step *microsteps* with interleaved parallel branches, a
signal memory with a four-status read/write discipline, a static *potential*
analysis, and an *expansion step* between reactions (§8). The two are related
by Theorem 3 (correctness and determinism): for a *causally correct* program
and input, every halted execution yields the same result and it is the unique
behavioral derivative. A *denotational* semantics (output history as a function
of input history) exists but is only cited ([24], §1.6). Compilation to
automata is Brzozowski-style derivative enumeration (§9), correct by
construction. **The circuit translation is not in this paper** (it is Berry's
later "Esterel on hardware" / constructive-semantics work); the only "circuit"
content here is the analogy of causality paradoxes to feedback gates (§7).
Proofs are "omitted and not even sketched" (§8), living in Gonthier's thesis.

## 1. Object; grammar

- The object is **basic Esterel**, a kernel of 14 statements (§3.2, p.100).
  **Plain Esterel** (user-level: `await`, `every`, `loop … each`, `upto`,
  `repeat`, timeouts, occurrence counts, `immediate`, valued exceptions with
  handlers, `copymodule`, sensors, `inputoutput` signals) is defined by
  **macro-expansion** into basic Esterel (§4; expansions given for `watching
  … timeout`, `watching immediate`, `upto`, `await`, `every`, `loop … each`).
  Data (types, constants, functions, procedures) are abstract, implemented in
  a host language (§2.1); Esterel is "a program generator used to program
  reactive kernels in the same way as YACC is used to program parsers" (§1.1).
- Grammar, as the paper lists it (not BNF, but a closed list; §3.2):
  ```
  stat ::= nothing
        |  halt
        |  X := exp
        |  call P (variable-list) (expression-list)
        |  emit S(exp)
        |  stat1 ; stat2
        |  loop stat end
        |  if exp then stat1 else stat2 end
        |  present S then stat1 else stat2 end
        |  do stat watching S
        |  stat1 || stat2
        |  trap T in stat end
        |  exit T
        |  var X : type in stat end
        |  signal S (combine type with comb) in stat end
  exp  ::= constants | X | ?S | op/function applications        (§3.1)
  ```
  Priority: `;` binds tighter than `||`; brackets `[ … ]` group (§3.2).
  Module: `module M : declarations body .` with `input S(type)`,
  `output S(combine type with comb)`, `relation S1 # S2`, `relation S1 => S2`
  (§2). Basic-Esterel restrictions (§3.2): inputs cannot be emitted
  internally; `present S` and `?S` not allowed on output signals; variables
  cannot be shared across `||`; a loop body "cannot terminate instantly when
  started (this is detected at compile time)" (§3.4).
- The semantics slightly **extends the kernel syntax** to carry state in the
  program text: `var X=[v] in stat end` and `signal S=[v] in stat end`, where
  "[·]" ("the □ operator transforms any semantic value into an Esterel
  constant", §6.5.14), and in §8 `signal S^x=[v] in stat end` with status
  x ∈ {⊥, †, +, −}.

## 2. Styles of semantics and how they relate

1. **Intuitive semantics** (§3.3, p.101–103): prose, structural over
   statements, in terms of four notions — when a statement *starts*, when it
   *terminates*, when it *exits a trap*, when it is *killed*; "a statement
   that does not terminate nor exit a trap instantly is said to take time";
   "Almost everything in Esterel is instantaneous"; only `halt` takes time.
2. **Denotational semantics** (cited only, [24]; §1.6): "globally defines the
   output sequence of a program as a function of a timed input sequence … it
   is not detailed here since it is useless for compiling algorithms."
3. **Behavioral semantics** (§6): "Plotkin-style rewrite rules"; per
   reaction, a program P with complete input event Î rewrites to a
   *derivative* P′ emitting output O: `P —O/Î↦ P′` (§6.2, p.115–116); deduced
   from a statement-level relation (§6.3) with more components. The
   local-signal rule is a fixpoint: "we require the body to work in the
   environment that it builds itself" (§6.5.15). Not executable ("the
   treatment of the sharing law involves an 'uneffective' fixpoint operation",
   §6). Serves as *the* definition.
4. **Execution semantics** (§8): microsteps `⟨stat, ρ, θ⟩ → ⟨stat′, ρ′, θ′⟩`
   on a sequential machine; "The parallel operator interleaves the actions of
   its branches, as in usual asynchronous models"; signals are "controlled
   shared variables" with the discipline "a signal cannot be read until it
   can no longer be written" (§1.6); auxiliary termination function 𝒯,
   potential function π̂, expansion ℰ. Effective: basis of interpreters (v2.2,
   v3).
5. **Translation to automata** (§9): derivatives enumerated à la Brzozowski;
   states = program terms (derivatives), transitions = action sequences;
   Theorem 4 finiteness for pure-signal programs; memory actions kept formal
   (§9.2). v3 uses a lower-level intermediate code and an abstract execution
   machine ([24], §9.4) — not described here.

Relations: 1 → 3 is informal ("The intuitive semantics … can be turned into a
formal denotational semantics", §1.6); 3 ↔ 4 by **Theorem 3** (§8.5); 4 → 5 by
construction (§9.1: "Given a current state P′ and an input Î, the automaton
emits an output event O and goes to state P″ iff P′ —O/Î↦ P″"). The paper
also reports that the first published semantics [5] and the v2 system treated
the simultaneous-emission-in-different-scopes example (§3.5, p.107)
incorrectly — a reminder that the rules were revised against examples.

## 3. Time model

- **Instants = input events.** "A sequence of input events is called an input
  history; the events define the instants of the history. Reactions only occur
  on input events; the underlying execution machine is inactive between input
  events." (§3.3). Synchrony hypothesis: "each reaction is assumed to be
  instantaneous—and therefore atomic in any possible sense" (§1.4); outputs
  are synchronous with inputs.
- Formal: event `E = S1(v1)·…·Sn(vn)` (set of emitted signals with values);
  synchronous product `E1 * E2` combines values of a signal in both via its
  `comb` (`*_S`), with `E * E = E` (§6.1). **Complete event** Ê over a sort 𝒮:
  each S ∈ 𝒮 appears as `S⁺(v)` (emitted with v) or `S⁻(v)` (absent, current
  value v). History `H = E0, E1, …`; complete history Ĥ respects the
  **persistence law**:
  ```
  S⁻ ∈ Ê0        implies  Ê0(S) = ⊥
  S⁻(v) ∈ Ê_{n+1} implies  Ê_n(S) = v
  ```
  Completion of H: `S⁻(⊥) ∈ Ê0 iff S ∉ E0`; `S⁺(v) ∈ Ê_n iff S(v) ∈ E_n`;
  `S⁻(v) ∈ Ê_{n+1} iff S ∉ E_{n+1} and Ê_n(S) = v`. "The current value of a
  signal S at the nth step in a history H is thus Ĥ_n(S)." (§6.1, p.115)
- **Unit of observation: one status (present/absent) and one value per signal
  per instant** — the *sharing law* (§3.3): "A signal has a fixed status in
  each reaction … A signal has a unique current value ?S in each reaction. If a
  signal is present … its value is … the combination of all the emitted values
  … If a signal is absent, its current value is the same as in the previous
  reaction."
- **Micro-steps**: in the behavioral semantics there are none — one big-step
  derivation per reaction, with the signal environment Ê guessed and checked
  by the local-signal rule. In the execution semantics, a reaction is "a
  sequence of elementary actions (also called microsteps as in [28]), followed
  by an expansion step" (§8); the order of microsteps is nondeterministic but
  immaterial (Theorem 1/Corollary 2). Variables are updated *in program order*
  inside the instant ("x := 1; x := x+1 … the final value of X is 2", §3.4);
  signal emission order is immaterial.
- **Instant boundary = `halt`** (under `watching`): `await S` ≡
  `do halt watching S`. Note the two footnotes (3, 4; p.102, 109): `watching S`
  does **not** count an S present in the starting instant ("watching next S"
  in older versions, now the default); `watching immediate S` expands to
  `present S else do stat watching S end` and does. Older versions had the
  opposite default. This is Esterel's own "which edge does the test anchor to"
  decision, made syntactically explicit.
- Derivative sequence (§6.2): `P = P0 —O0/Î0↦ P1 —O1/Î1↦ P2 …`; "The
  derivative technique transforms a temporal problem into two instantaneous
  ones: find the instantaneous reaction on an input and find the derivative."

## 4. State

- Two kinds of data: **variables** (local to a statement, not shared across
  `||`, "can be updated several times within a single reaction", initial ⊥)
  and **signals** (status + persistent value, shared by instantaneous
  broadcast, initial value ⊥ for local/output). Inputs: "the value of a signal
  can change only when a tick occurs" (§2.2).
- **The program term is the state.** The derivative P′ "has the same
  declarations as P_n, but a different body" (§6.2). Control state = where the
  `halt`s are in the residual term; data state = boxed values written back
  into the declarations: rule 6.5.14 (variables) "allocates the variable with
  the currently saved value, executes the body, and saves the new value for
  the next step"; rule 6.5.15 (signals) likewise stores v′. So "what persists
  across instants" is decided structurally: exactly the declarations that
  enclose a non-terminated (paused) body survive into the derivative, and
  carry their values. There is no register/wire distinction; a variable whose
  scope terminates within the instant simply disappears from the derivative.
- Expansion step (§8.4) resets local signal status to ⊥, rewrites `do stat
  watching S` into `present S else do ℰ(stat) watching S end`, and cleans
  `nothing; halt` → `halt`.
- Initial values: ⊥ for variables and local/output signals (§3.3); `S⁻(⊥) ∈ Ê0`.

## 5. Intra-instant ordering, causality, forwarding, multiple emissions

- **Sharing law / forwarding is total within the instant**: a reader sees the
  final status and combined value of the instant, whatever the textual or
  parallel position of the emitters. §3.5 example: `emit S(2); Y := ?S ||
  emit S(1); present S then X := ?S end` — "the two emissions are
  simultaneous, the present statement sees S as present, and both X and Y
  receive the value ?S equal to 3 = 1+2." So "emit then test the same signal in
  the same instant" sees the emission — and so does "test, in parallel with an
  emit that is textually later".
- **Multiple emissions per instant** are legal and combined by the signal's
  declared associative-commutative `comb` ("collision", §2.2.1); a signal
  declared without `comb` is checked by the compiler never to collide (§2.2.2).
  Two emissions of a local signal in *different instances* of its declaration
  (loop restarted in the same instant) are *not* combined (§3.5, p.107).
- **Variables** are ordered by sequence; across `||` they are not shared, so
  no ordering question arises (`merge(ρ, ρ1′, ρ2′)`, §6.5.11).
- **Causality**: the behavioral fixpoint "does not respect the intuitive
  sequentiality constraints"; programs P1–P6 (§7, p.125–126) have 0 or 2
  solutions:
  ```
  P1: signal S in present S then emit S end end        -- two solutions
  P2: signal S in present S else emit S end end        -- no solution (NOT gate fed back)
  P3/P4: two-signal mutual versions                    -- two solutions each
  P5: signal S(integer) in emit S(?S+C) end            -- S = S + c, none/∞
  P6: P5 at type triv                                   -- unique, still rejected
  ```
  Criterion (§7): "a signal should not be read if it can still be written … a
  program is correct (with respect to some input) if it can be correctly
  executed." Explicitly *not* "read before written" (too weak for P1, P3, P4),
  and explicitly *not* "has a unique solution" (P6 has one and is rejected,
  because "the semantics of Esterel programs should not depend on type
  implementation details"). This is **not** a fixpoint iteration and **not**
  the 1999 constructive semantics; cyclic dependencies are *rejected*, by one
  of two sufficient conditions: a static signal-dependency graph must be
  acyclic (v2.2, proved in [23]; "sometimes rejects correct programs"), or the
  potential-based read/write discipline (v3, §8). Correctness is per reaction:
  *locally correct* = correct for all inputs now; *globally correct* = every
  reachable derivative is locally correct (§7, p.127).
- **The four cell statuses** (§8.1, p.128) are the mechanism:
  ```
  (S⊥ = v): not yet written this reaction (v is the previous value)
  (S† = v): written, may still be written; cannot be read
  (S⁺ = v): written and can no longer be written; readable, present
  (S⁻ = v): not written and can no longer be written; readable, absent
  ```
  Writes: `θ.(S⊥=v)[S←v′] = θ.(S†=v′)`; `θ.(S†=v)[S←v′] = θ.(S†= v *_S v′)`;
  no write to S⁺/S⁻. Read `θ(S)` only when status ∈ {+, −}. Status moves
  † → + or ⊥ → − exactly when S ∉ π(stat) for the enclosing local-signal body
  (operation ±, §8.2.1). Output signals never get + or − "but this does not
  matter, since it is never read internally" (§8.1).

## 6. Properties stated

- **Theorem 1 (Strong confluence)** (§8.3, p.135): for any two distinct
  actions `⟨stat,ρ,θ⟩ → ⟨stat1′,ρ1′,θ1′⟩` and `⟨stat,ρ,θ⟩ → ⟨stat2′,ρ2′,θ2′⟩`
  there exist stat″, ρ″, θ″ with `⟨stat1′,ρ1′,θ1′⟩ → ⟨stat″,ρ″,θ″⟩` and
  `⟨stat2′,ρ2′,θ2′⟩ → ⟨stat″,ρ″,θ″⟩`. (One-step diamond.)
- **Corollary 2 (Global confluence)**: "If ⟨stat, φ, θ⟩ has a halted
  execution sequence, then it has no infinite execution sequence, all its
  maximal sequences are halted, and they all yield the same result." (Church–
  Rosser + strong normalization, §1.6.)
- **Theorem 3 (Correctness and determinism)** (§8.5, p.137): "Let P be a
  program, let Î be a complete input event such that P is causally correct in
  θ_Î. There exist a unique program P′ and a unique output event O such that
  `P —O/Î↦ P′` and `P ⇒O/Î P′`." I.e. the execution result (after expansion)
  is a behavioral derivative, and the behavioral derivative is unique.
- **Theorem 4** (§9.1, p.140): "Any pure synchronization program has only
  finitely many derivatives." (Hence a finite automaton.)
- Compile-time checks mentioned: instantaneous loop bodies rejected (§3.4);
  collision-freedom for signals without `comb` (§2.2.2); dependency-graph
  acyclicity (v2.2, [23]).
- Proof status: "The proofs are omitted and not even sketched; they require
  the introduction of many technical concepts that are outside the scope of
  this paper. See [24] for complete proofs." (§8). Paper proofs in a thesis;
  nothing mechanized; no testing claims about the semantics (benchmarks are
  compile time and automaton size, §9.3–9.4).

## 7. Semantics vs. generated code

- Automata: the automaton is *defined* as the reaction graph of derivatives
  (§9.1), so its correctness is by construction relative to the execution
  semantics (and hence, via Theorem 3, the behavioral one). Run time keeps
  only "actions that must be done at run-time"; "the synchronization needed
  for the internal communication of values is simply implemented by the order
  of the assignments in the transitions; thus it generates no code" (§9.2).
  Local pure signals vanish ("A local signal acts as an auxiliary nonterminal
  in a parser generator", §9.1). Appendix B shows the reflex game's 6-state
  automaton as action tables and `if … goto` transitions.
- Circuits: **not treated**. The 1992 paper's circuit content is limited to
  the physical analogies for causality paradoxes (NOT gate with output to
  input; amplifier positive feedback; "races can appear whenever sequential
  propagation of electrical currents is not compatible with the topological
  structure", §7) and the remark that VLSI's "all reactions take one clock
  cycle" is "a similar but weaker synchrony hypothesis … it lacks good
  compositionality properties and cannot be used as the basis of a general
  reactive programming language" (§1.4). The hardware translation and its
  relation to the behavioral semantics are in later work (Berry, "Esterel on
  hardware", 1991/92; the constructive semantics book, 1999), not here.

## 8. Exact locations and transcribed rules

Locations: statements list §3.2 (p.100); intuitive semantics §3.3 (p.101–103);
plain→basic expansions §4 (p.107–111); events/complete events/histories §6.1
(p.114–115); derivatives §6.2 (p.115–116); transition form and memories §6.3
(p.116–117); expression rules §6.4 (p.118); statement rules §6.5.1–6.5.15
(p.118–122); worked derivation §6.6 (p.123–124); paradoxes and correctness
criterion §7 (p.124–127); signal memory §8.1 (p.128–129); action rules §8.2.1
(p.130–131); expression/termination/potential rules §8.2.2–8.2.4 (p.132–134);
Theorem 1, Corollary 2 §8.3 (p.135–136); expansion §8.4 (p.136); Definition +
Theorem 3 §8.5 (p.137); worked execution §8.6 (p.137–140); automata §9,
Theorem 4 (p.140).

### Behavioral semantics (§6)

Transition form (§6.3):
```
⟨stat, ρ⟩ ——E′, b, T——→_Ê ⟨stat′, ρ′⟩
```
ρ, ρ′ memories of free variables; Ê complete event (inputs + visible local
signals); E′ emitted event (outputs + visible locals); b ∈ {tt, ff}
termination; T set of exited trap labels. Program level: `P —E′/Ê↦ P′ iff
⟨stat, φ⟩ —E′,ff,∅→_Ê ⟨stat′, φ⟩` (body assumed followed by `halt`, so never
terminates).

Expressions (§6.4): `⟨C,ρ⟩ →_Ê c`; `⟨X,ρ⟩ →_Ê ρ(X)`; `⟨?S,ρ⟩ →_Ê Ê(S)`.

Axioms (§6.5.1–2, 6.5.13):
```
⟨nothing, ρ⟩ —ε,tt,∅→_Ê ⟨nothing, ρ⟩
⟨halt, ρ⟩    —ε,ff,∅→_Ê ⟨halt, ρ⟩
⟨exit T, ρ⟩  —ε,ff,{T}→_Ê ⟨halt, ρ⟩
```

Assignment (§6.5.3) and emit (§6.5.5):
```
        ⟨exp, ρ⟩ →_Ê v                              ⟨exp, ρ⟩ →_Ê v
—————————————————————————————————        ———————————————————————————————————
⟨X := exp, ρ⟩ —ε,tt,∅→_Ê ⟨nothing, ρ[X←v]⟩   ⟨emit S(exp), ρ⟩ —S(v),tt,∅→_Ê ⟨nothing, ρ⟩
```

Sequence (§6.5.6) — two rules, the first is the "pause inside stat1" case:
```
      ⟨stat1, ρ⟩ —E1′,ff,T1→_Ê ⟨stat1′, ρ1′⟩
——————————————————————————————————————————————
⟨stat1; stat2, ρ⟩ —E1′,ff,T1→_Ê ⟨stat1′; stat2, ρ1′⟩

⟨stat1, ρ⟩ —E1′,tt,∅→_Ê ⟨stat1′, ρ1′⟩     ⟨stat2, ρ1′⟩ —E2′,b2,T2→_Ê ⟨stat2′, ρ2′⟩
———————————————————————————————————————————————————————————————————————————
          ⟨stat1; stat2, ρ⟩ —E1′ * E2′, b2, T2→_Ê ⟨stat2′, ρ2′⟩
```
(Note: stat2 runs in ρ1′ — variables are sequential — but in the *same* Ê —
signals are shared; and the derivative drops stat1′ entirely.)

Loop (§6.5.7): `⟨stat; loop stat end, ρ⟩ —E′,ff,T→_Ê ⟨stat′, ρ′⟩` ⟹
`⟨loop stat end, ρ⟩ —E′,ff,T→_Ê ⟨stat′, ρ′⟩` (instantaneous unfolding).

Present (§6.5.9):
```
S⁺ ∈ Ê    ⟨stat1, ρ⟩ —E1′,b1,T1→_Ê ⟨stat1′, ρ1′⟩
————————————————————————————————————————————————————————
⟨present S then stat1 else stat2 end, ρ⟩ —E1′,b1,T1→_Ê ⟨stat1′, ρ1′⟩
```
(symmetric with S⁻ ∈ Ê for the else branch).

Watching (§6.5.10) — the temporal guard does not fire in the starting instant:
```
             ⟨stat, ρ⟩ —E′,b,T→_Ê ⟨stat′, ρ′⟩
————————————————————————————————————————————————————————————————
⟨do stat watching S, ρ⟩ —E′,b,T→_Ê ⟨present S else do stat′ watching S end, ρ′⟩
```

Parallel (§6.5.11):
```
⟨stat1, ρ⟩ —E1′,b1,T1→_Ê ⟨stat1′, ρ1′⟩    ⟨stat2, ρ⟩ —E2′,b2,T2→_Ê ⟨stat2′, ρ2′⟩
————————————————————————————————————————————————————————————————————————
⟨stat1 || stat2, ρ⟩ —E1′ * E2′, b1 ∧ b2, T1 ∪ T2→_Ê ⟨stat1′ || stat2′, merge(ρ, ρ1′, ρ2′)⟩
```
with merge: ρ′(X) = ρ(X) if neither changed it, else the one that changed it.

Trap (§6.5.12):
```
⟨stat, ρ⟩ —E′,b,T→_Ê ⟨stat′, ρ′⟩    b = tt or T = {T}
————————————————————————————————————————————————————
⟨trap T in stat end, ρ⟩ —E′,tt,∅→_Ê ⟨nothing, ρ′⟩

⟨stat, ρ⟩ —E′,ff,T→_Ê ⟨stat′, ρ′⟩                 (otherwise)
——————————————————————————————————————————————————————————
⟨trap T in stat end, ρ⟩ —E′,ff,T−{T}→_Ê ⟨trap T in stat′ end, ρ′⟩
```

Local variable (§6.5.14) — the value persists in the term:
```
      ⟨stat, ρ.(X = v)⟩ —E′,b,T→_Ê ⟨stat′, ρ′.(X = v′)⟩
——————————————————————————————————————————————————————————
⟨var X=[v] in stat end, ρ⟩ —E′,b,T→_Ê ⟨var X=[v′] in stat′ end, ρ′⟩
```

Local signal (§6.5.15) — THE fixpoint rules (sharing law):
```
⟨stat, ρ⟩ —E′ * S(v′), b, T→_{(Ê\S) * S⁺(v′)} ⟨stat′, ρ′⟩      S ∉ E′
——————————————————————————————————————————————————————————————————
⟨signal S=[v] in stat end, ρ⟩ —E′,b,T→_Ê ⟨signal S=[v′] in stat′ end, ρ′⟩

⟨stat, ρ⟩ —E′, b, T→_{(Ê\S) * S⁻(v)} ⟨stat′, ρ′⟩               S ∉ E′
——————————————————————————————————————————————————————————————————
⟨signal S=[v] in stat end, ρ⟩ —E′,b,T→_Ê ⟨signal S=[v] in stat′ end, ρ′⟩
```
First rule: the body is run in an environment where S is present with the
very value v′ it emits (guess-and-check); the new value is saved. Second: S
not emitted, seen absent with the retained value v.

### Execution semantics (§8)

Action form: `⟨stat, ρ, θ⟩ → ⟨stat′, ρ′, θ′⟩`, θ a signal memory of cells
`(S^x = v)`, x ∈ {⊥, †, +, −}. Initial memory θ_Î for input event Î: `(S^x = v)
for all S^x(v) ∈ Î`, `(S⊥ = ⊥)` for every output S. Final output `O_θ` = all
`S(v)` with `(S† = v)` ∈ θ.

Selected action rules (§8.2.1):
```
         ⟨exp, ρ, θ⟩ → v
—————————————————————————————————————————
⟨emit S(exp), ρ, θ⟩ → ⟨nothing, ρ, θ[S←v]⟩

⟨stat1, ρ, θ⟩ → ⟨stat1′, ρ1′, θ1′⟩                 𝒯(stat1) = ⟨tt, ∅⟩   ⟨stat2, ρ, θ⟩ → ⟨stat2′, ρ2′, θ2′⟩
————————————————————————————————————————       ————————————————————————————————————————————————————
⟨stat1; stat2, ρ, θ⟩ → ⟨stat1′; stat2, ρ1′, θ1′⟩   ⟨stat1; stat2, ρ, θ⟩ → ⟨stat2′, ρ2′, θ2′⟩

      S⁺ ∈ θ                                                  S⁻ ∈ θ
——————————————————————————————————————————      ——————————————————————————————————————————
⟨present S then stat1 else stat2 end, ρ, θ⟩      ⟨present S then stat1 else stat2 end, ρ, θ⟩
      → ⟨stat1, ρ, θ⟩                                   → ⟨stat2, ρ, θ⟩

⟨stat1, ρ, θ⟩ → ⟨stat1′, ρ1′, θ1′⟩          ⟨stat2, ρ, θ⟩ → ⟨stat2′, ρ2′, θ2′⟩
—————————————————————————————————————       —————————————————————————————————————
⟨stat1 || stat2, ρ, θ⟩ → ⟨stat1′ || stat2, ρ1′, θ1′⟩    ⟨stat1 || stat2, ρ, θ⟩ → ⟨stat1 || stat2′, ρ2′, θ2′⟩

         ⟨stat, ρ, θ⟩ → ⟨stat′, ρ′, θ′⟩
————————————————————————————————————————————————————————
⟨do stat watching S, ρ, θ⟩ → ⟨do stat′ watching S, ρ′, θ′⟩     (guard not yet armed)
```
Value access (§8.2.2):
```
S⁻ ∈ θ  or  S⁺ ∈ θ
——————————————————
⟨?S, ρ, θ⟩ → θ(S)
```
Local signal with potentials (§8.2.1, p.131):
```
±(S, stat, x) = +  if x = † and S ∉ π(stat)
              = −  if x = ⊥ and S ∉ π(stat)
              = x  otherwise

y = ±(S, stat, x)     ⟨stat, ρ, θ.(S^y = v)⟩ → ⟨stat′, ρ′, θ′.(S^z = v′)⟩
—————————————————————————————————————————————————————————————————————
⟨signal S^x=[v] in stat end, ρ, θ⟩ → ⟨signal S^z=[v′] in stat′ end, ρ′, θ′⟩
```
Termination 𝒯 (§8.2.3), partial, only on action-less terms: 𝒯(nothing) =
⟨tt,∅⟩; 𝒯(halt) = ⟨ff,∅⟩; 𝒯(stat1; stat2) = ⟨ff,T⟩ if 𝒯(stat1) = ⟨ff,T⟩, =
⟨b,T⟩ if 𝒯(stat1) = ⟨tt,∅⟩ and 𝒯(stat2) = ⟨b,T⟩; 𝒯(stat1 || stat2) =
⟨b1 ∧ b2, T1 ∪ T2⟩; 𝒯(trap T in stat end) = ⟨b ∨ (T = {T}), T − {T}⟩;
𝒯(exit T) = ⟨ff, {T}⟩; declarations propagate. *halted* = ⟨ff, ∅⟩.

Potentials π̂ (§8.2.4) — a static over-approximation ⟨π, b, T⟩ of (signals
emittable, can terminate, can exit): π̂(emit S(exp)) = ⟨{S}, tt, ∅⟩;
π̂(stat1; stat2) = ⟨π1, ff, T1⟩ if b1 = ff, else ⟨π1 ∪ π2, b2, T1 ∪ T2⟩;
π̂(if/present) = ⟨π1 ∪ π2, b1 ∨ b2, T1 ∪ T2⟩; π̂(||) = ⟨π1 ∪ π2, b1 ∧ b2, T1 ∪
T2⟩; π̂(loop) = ⟨π, ff, T⟩; π̂(signal S in stat end) = ⟨π \ S, b, T⟩.

Execution/halted (§8.3): an execution `⟨stat,ρ,θ⟩ → … → ⟨stat_n,ρ_n,θ_n⟩` is
*maximal* if no further action, *halted* if 𝒯(stat_n) = ⟨ff, ∅⟩.

Expansion ℰ (§8.4): ℰ(stat1; stat2) = ℰ(stat2) if 𝒯(stat1) = ⟨tt,∅⟩ else
ℰ(stat1); stat2; ℰ(loop stat end) = ℰ(stat); loop stat end; ℰ(do stat
watching S) = present S else do ℰ(stat) watching S end; ℰ(trap T in stat end)
= nothing if 𝒯 = ⟨tt,∅⟩ else trap T in ℰ(stat) end; ℰ(signal S^x=[v] in stat
end) = signal S⊥=[v] in ℰ(stat) end.

Causal correctness (§8.5, Definition): P is causally correct w.r.t. Î, written
`P ⇒O/Î P′`, if stat has a halted execution `⟨stat, φ, θ_Î⟩ → ⟨stat′, φ, θ′⟩`
with body(P′) = ℰ(stat′) and O = O_θ′.

## 9. Transfer to Copper — concrete and critical

Reading key: Copper "cycle" ≈ Esterel "instant/reaction"; Copper
`clk.tick().await` ≈ a paused `halt` (`await`); Copper "segment between ticks"
≈ the actions of one reaction; Copper local ≈ Esterel *variable*; Copper port ≈
Esterel *signal* (with caveats below); Copper `control_extract` FSM ≈ the set
of derivatives.

### What maps well

1. **Pause/tick ↔ halt/await, segment ↔ reaction.** Yes, structurally. The
   sequence rule's two cases are exactly "the segment continues past stat1"
   (b = tt) vs "the segment ends inside stat1, and stat1′; stat2 is the
   residue" (b = ff). Copper's segment is the maximal prefix of the body that
   executes before the next `tick`; Esterel's is the set of actions before all
   branches reach `halt`. The *termination flag b* is the device Copper lacks
   in writing: a per-statement boolean "did this complete within the cycle",
   compositional through sequence (b2 only if b1), loop (never), parallel
   (∧), trap (∨ exited). Copper's `Cfg` with `E_comb`/`E_tick` edges is the
   graph form of the same thing; a rule-based presentation would phrase
   `check_reachability` as "the loop body has b = ff on every path" — which is
   literally Esterel's compile-time restriction "the body of a loop cannot
   terminate instantly when started" (§3.4). Exact analogue, worth citing.

2. **Registers by liveness ↔ boxed values in the derivative.** Esterel's
   `var X=[v]` rule is a syntactic rendering of "a value live across a pause
   is state": only a declaration enclosing a paused body survives into the
   derivative, and it carries its value. Copper's `infer_registers` ("defined-
   in-loop ∧ live-across-a-tick") is the dataflow version of the same
   criterion; the Esterel presentation suggests a clean way to *define* it
   without liveness analysis — state the derivative, then observe which
   locals the derivative mentions. For a paper, a derivative-style definition
   ("the residue of the coroutine after one cycle, with live values written
   into it") is more declarative than "what the liveness pass computes", and
   the liveness pass becomes an implementation of it.

3. **FSM extraction ↔ derivative enumeration.** `control_extract`'s
   `match pc` FSM is Brzozowski derivative enumeration specialised to a single
   sequential body: states = residues after each tick, transitions = the
   segment code. Theorem 4's finiteness argument (pure programs have finitely
   many derivatives) is trivial for Copper (one coroutine, finitely many
   await sites), but the *correctness-by-construction* framing (§9.1: the
   automaton transitions are the reactions) is the shape Copper could use to
   state "the SV FSM agrees with the simulator": both are defined as computing
   the same derivative. What Copper has instead is a measured sweep; the
   derivative framing is how to turn "agree on 4 seeds" into a proposition.

4. **Persistence law ↔ `Out` holds when unwritten.** `S⁻(v) ∈ Ê_{n+1} implies
   Ê_n(S) = v` is a one-line statement of Copper's enabled-register idiom
   (memory note `copper_out_hold_semantics`): an unwritten `Out` in a cycle
   carries the previous cycle's value. The complete-event notation `S⁺(v)` /
   `S⁻(v)` (written-this-cycle vs held) is a ready-made notation for the
   "one value per signal per cycle" denotation with the write/hold
   distinction made explicit per cycle. Copper could index this further by
   anchor: `S⁺ₒ(v)` (opening drive) vs `S⁺꜀(v)` (closing drive) — Esterel has
   no need for that split, see below.

5. **Causality criterion ↔ the pre-tick alignment family.** "A signal should
   not be read if it can still be written" is the principle behind
   `multi_phase_out_write`, D1 mixed anchors, `trailing_out_reads_input`,
   `linear_out_write_anchor_mismatch`: the hazard in every one of them is a
   value being *observed* (by the SV's continuous assign / flop, or by the
   simulator's immediate read) at a point where the segment could still
   change it. Esterel resolves it dynamically per reaction (statuses †→+ via
   potentials) and offers the static dependency-graph check as the
   conservative alternative; Copper's rules are all static and shape-based
   (like v2.2), and Esterel's own remark applies — "it sometimes rejects
   correct programs" — which is what the D1/multi_phase narrowing history is
   about. The *potential* function π̂ (set of signals a residue can still
   emit, computed structurally with (π, b, T)) is a concrete device Copper
   could adopt verbatim as "the set of ports a segment suffix can still
   write", and define "a drive is closing iff an `In` read precedes it" in
   those terms.

6. **Watching vs watching immediate ↔ the anchor of a wait-loop's test.** The
   footnoted history (default flipped between versions; `await S` ignores an
   S present in the starting instant; `await immediate S` does not) is the
   same off-by-one that Copper's opening/closing anchoring names. Esterel's
   fix was *two syntactic forms*, not analysis — a possible design option for
   Copper's `In` reads (an explicit "read as of this cycle's opening" vs "as
   of the closing") rather than inferring the anchor from where the read
   sits. Worth weighing against Copper's current inference.

7. **Theorem shape.** Theorem 3 is the template for what Copper would want:
   (i) an uneffective reference (behavioral/denotational: one value per
   signal per cycle), (ii) an effective execution model (the simulator's
   phases and levelized order), (iii) a correctness/determinism theorem that
   every execution yields the reference result. Copper's "simulation must be
   independent of poll order" is Corollary 2 (all maximal executions halt
   and give the same result) across modules; Copper's levelized scheduler is
   the deterministic *choice* of one such execution, and the fixpoint
   scheduler + poll-order fuzzer is the empirical form of Theorem 1.

### What does not map, or maps misleadingly

1. **Esterel has one observation point per instant; Copper has two.** The
   sharing law makes every reader see the *final* value of the instant.
   Copper's cycle has pre-edge settle → edge → post-edge settle → post-edge
   observation, and the whole D1/D2/multi-phase family exists because an `In`
   read in the simulator can be *immediate* (the current wire) while a port
   drive is realised at the edge. In Esterel that situation is by definition
   a causality error (read of S while S† — "can still be written"), not a
   semantic choice. Berry's §1.4 remark that circuit synchrony ("all reactions
   take one clock cycle") is a "weaker" hypothesis with poor compositionality
   is precisely the gap: Copper's opening/closing anchor distinction is a
   feature of the *clocked-circuit* model (closer to Lustre's pre/→ or a
   Mealy/Moore split), and Esterel's apparatus does not express it. So: the
   tick/pause mapping holds for *control*; the data model inside the instant
   does not transfer without adding the phase structure.

2. **Emit-then-test is stronger in Esterel than Copper's forwarding.** Copper
   forwards a local write to *later* reads in program order within the
   segment (a sequential variable discipline); Esterel's signal forwarding is
   order-independent across parallel branches (a read sees emissions textually
   after it, in other branches). Copper has no intra-module parallelism, so
   its locals are Esterel *variables* ("x := 1; x := x+1 → 2"), not signals.
   The closest Esterel signal analogue to a Copper `Out` is a single-emitter
   signal with persistence; the combination function (`comb`) and collision
   handling have no counterpart (Copper: single writer, last write wins within
   a segment — which is the *variable* rule, not the signal rule). Do not
   claim Copper's forwarding "is" Esterel's sharing law; it is the sequential
   half of it.

3. **Causality resolution.** Esterel 1992 *rejects* cyclic dependencies (no
   fixpoint iteration; constructive semantics came later). Copper's levelized
   scheduler accepts and iterates *convergent* comb SCCs and panics on
   non-convergent ones (`LEVELIZED_SCHEDULING_SCOPE.md`). That is closer to
   the later constructive/Kleene-iteration view than to this paper. The 1992
   paper's P6 argument (reject even when the solution is unique, because
   acceptance must not depend on data details) is a point *against* Copper's
   "iterate and see if it converges": convergence can depend on widths and
   values. Worth a sentence in Copper's doc either way.

4. **No `In` sampling model.** Esterel inputs are events with persistence
   (value changes only on a tick). Copper's `In` is a wire whose value the
   environment may change between phases; the paper has nothing on sampling
   an input at an edge vs reading it combinationally.

5. **Traps, preemption, parallel, interleaving + confluence.** Most of the
   paper's technical weight (T sets, merge of memories, strong confluence
   over interleavings, potentials over `||`) exists because of `||`, `trap`,
   and `watching`. Copper has none within a module; a Copper semantics can be
   a *deterministic* big-step per cycle for one coroutine, and the
   parallelism lives between modules where the comb-dataflow DAG, not
   interleaving, decides order. Borrow the *form* (labelled big-step per
   cycle with a termination flag and a residue), drop the machinery.

6. **The derivative stores state in the program text.** Elegant for a
   definition; for Copper's SV emission the state must be a vector of
   registers plus `pc`. The gap between "residual term" and "register
   vector" is exactly what Copper's VLIR lowering fills, and it is where
   Copper's register-reconciliation audit lives; Esterel's paper does not
   address the analogous automaton-encoding question beyond "state numbers"
   (§9.1) and "an object code variable is allocated for each explicitly
   declared variable or valued signal" (§9.2).

7. **Proof culture.** Even the Esterel paper omits all proofs ("not even
   sketched") and points to a thesis; nothing is mechanized. A Copper
   "formal-esque" definition can honestly follow the same pattern — rules +
   theorem statements + the sweep as evidence — without pretending to more,
   as long as it does not call measured agreement a theorem (contrast Arch).
