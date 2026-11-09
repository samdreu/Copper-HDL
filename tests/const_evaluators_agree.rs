//! D-21 B2 (2026-11-07): one constant grammar (SPEC I.14.4), two evaluators,
//! pinned against each other on every corpus constant.
//!
//! The interpreter's evaluator (`copper_ref::ConstScope`, the same `Exec::eval_const`
//! a module's consts and widths go through) stays independent of the front-ends'
//! (`copper_analysis::consts::ConstEnv`, which the transpiler's register fold,
//! counted-`for` bounds and constant indices use) — that independence is what lets
//! the reference leg attribute a disagreement (SPEC III.2). This test is the price:
//! it evaluates, over every file of `examples/`, `src/` and `tests/` that defines a
//! `#[hardware]` fn,
//!
//! 1. **every `const` item** (file scope, `impl` blocks as `T::K`, fn bodies) both
//!    ways;
//! 2. **every pre-loop `let` initializer** of a clocked module: against the
//!    transpiler's *stated* power-on value (the SHIR register's `init`, what lands in
//!    the SV's `initial` block — the fold's output) when the binding is a register,
//!    and against `ConstEnv` when it evaluates the initializer;
//! 3. **every counted-`for` bound and every index** (`x[e]`, `x.get(e)`, `x.set(e, _)`)
//!    of every `#[hardware]` fn, both ways.
//!
//! When both evaluate, the values must be equal (a DISAGREE fails the test). When
//! only one does, the expression is an **asymmetry** — a grammar the two do not
//! share — and must be listed in [`ASYMMETRIC`] with a reason (exact set: a new one or
//! a stale one fails). When neither does, it is not a constant of the shared grammar
//! (a local, a `&str`) and is counted. The test fails if it compares nothing in any
//! of the three classes, and writes its coverage line to `COPPER_CONST_EVAL_REPORT`
//! for `tools/regression.sh`'s G-J.
//!
//! The sweep pins both against rustc as well: the simulator runs rustc's value, the
//! interpreter its own, the SV the transpiler's (G-G).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use copper_core::chir::{CHIRType, Width};
use copper_core::shir::SHIRBody;
use copper_ref::{ConstScope, Value};
use quote::ToTokens;
use syn::visit::Visit;
use syn::{Expr, Item, ItemFn, Stmt};

/// `(site, reason)`: expressions only one evaluator evaluates. A site is
/// `<file>::<what>` as the test prints it. Empty since Phase C step 9: its seven
/// rows were the interpreter's `Self { .. }` gap (the `bubble()` initializers of
/// `struct_pipeline_dut` and the pipelined CPU), closed then, and each is now
/// compared both ways (MEASUREMENTS §O.2). Since §U.2 (2026-11-08) the rows are the first
/// non-integer consts of the corpus, the const-pattern fixture's.
const ASYMMETRIC: &[(&str, &str)] = &[
    ("tests/fixtures/const_pattern_dut.rs::const CP_ONE", "a `Logic`/`Bits` const: `ConstEnv` is integer-valued (SPEC I.14.4's evaluator over integers), so only the interpreter evaluates it; the transpiler folds a pattern naming it with `fold_const_init`, and the sweep checks that fold against rustc (`const_pattern_dut`, MEASUREMENTS §U.2)"),
    ("tests/fixtures/const_pattern_dut.rs::const CP_ZERO", "a `Logic`/`Bits` const: `ConstEnv` is integer-valued (SPEC I.14.4's evaluator over integers), so only the interpreter evaluates it; the transpiler folds a pattern naming it with `fold_const_init`, and the sweep checks that fold against rustc (`const_pattern_dut`, MEASUREMENTS §U.2)"),
    ("tests/fixtures/const_pattern_dut.rs::const CP_TWO", "a `Logic`/`Bits` const: `ConstEnv` is integer-valued (SPEC I.14.4's evaluator over integers), so only the interpreter evaluates it; the transpiler folds a pattern naming it with `fold_const_init`, and the sweep checks that fold against rustc (`const_pattern_dut`, MEASUREMENTS §U.2)"),
    ("tests/fixtures/const_pattern_dut.rs::const CP_THREE", "a `Logic`/`Bits` const: `ConstEnv` is integer-valued (SPEC I.14.4's evaluator over integers), so only the interpreter evaluates it; the transpiler folds a pattern naming it with `fold_const_init`, and the sweep checks that fold against rustc (`const_pattern_dut`, MEASUREMENTS §U.2)"),
    ("tests/fixtures/parity_battery_dut.rs::const PB_MASK", "a `Logic`/`Bits` const: `ConstEnv` is integer-valued (SPEC I.14.4's evaluator over integers), so only the interpreter evaluates it; the transpiler states it as a sized `localparam` of the value `fold_const_init` folds it to, and the sweep checks that fold against rustc (`parity_battery_dut`, MEASUREMENTS §Z.8, line `II.3 found bits-const`)"),
    ("tests/fixtures/parity_battery_dut.rs::const PB_ONE", "a `Logic`/`Bits` const: `ConstEnv` is integer-valued (SPEC I.14.4's evaluator over integers), so only the interpreter evaluates it; the transpiler states it as a sized `localparam` of the value `fold_const_init` folds it to, and the sweep checks that fold against rustc (`parity_battery_dut`, MEASUREMENTS §Z.8, line `II.3 found bits-const`)"),
    ("tests/fixtures/parity_battery_dut.rs::pbt_bits_const::const PB_LOCAL", "a `Logic`/`Bits` const: `ConstEnv` is integer-valued (SPEC I.14.4's evaluator over integers), so only the interpreter evaluates it; the transpiler states it as a sized `localparam` of the value `fold_const_init` folds it to, and the sweep checks that fold against rustc (`parity_battery_dut`, MEASUREMENTS §Z.8, line `II.3 found bits-const`)"),
    ("tests/fixtures/const_pattern_dut.rs::cp_tuple_expr::const BODY_TWO", "a `Logic`/`Bits` const: `ConstEnv` is integer-valued (SPEC I.14.4's evaluator over integers), so only the interpreter evaluates it; the transpiler folds a pattern naming it with `fold_const_init`, and the sweep checks that fold against rustc (`const_pattern_dut`, MEASUREMENTS §U.2)"),
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target" || n == "obj_dir" || n == "old") {
                continue;
            }
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

fn hardware_mode(f: &ItemFn) -> Option<String> {
    f.attrs.iter().find_map(|a| {
        if !a.path().segments.last().is_some_and(|s| s.ident == "hardware") {
            return None;
        }
        let text = a.meta.require_list().ok()?.tokens.to_string();
        Some(text.split(',').next()?.trim().to_string())
    })
}

fn hardware_fns(items: &[Item], out: &mut Vec<ItemFn>) {
    for it in items {
        match it {
            Item::Fn(f) if hardware_mode(f).is_some() => out.push(f.clone()),
            Item::Mod(m) => {
                if let Some((_, items)) = &m.content {
                    hardware_fns(items, out);
                }
            }
            _ => {}
        }
    }
}

/// The item texts the transpiler hands `ConstEnv` (`parser.rs`'s `const_source`).
fn const_source(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .filter(|it| match it {
            Item::Const(_) => true,
            // As `copper-codegen`'s `capture_file_scope` (2026-11-08, MEASUREMENTS §V.4):
            // an immutable `static` and a tuple struct are constants of I.14.4 too.
            Item::Static(st) => matches!(st.mutability, syn::StaticMutability::None),
            Item::Struct(st) => matches!(st.fields, syn::Fields::Unnamed(_)),
            Item::Fn(f) => f.sig.constness.is_some(),
            Item::Impl(imp) => imp.trait_.is_none(),
            _ => false,
        })
        .map(|it| it.to_token_stream().to_string())
        .collect()
}

/// A body's own items as `Item`s, and the `const` items of its nested blocks (a loop body's
/// `const`): both evaluators read a nested block's consts since 2026-11-08 (the transpiler
/// hoists them, the interpreter collects them; MEASUREMENTS §Z.3).
fn body_items(f: &ItemFn) -> Vec<Item> {
    struct V(Vec<Item>, usize);
    impl<'ast> syn::visit::Visit<'ast> for V {
        fn visit_block(&mut self, b: &'ast syn::Block) {
            self.1 += 1;
            syn::visit::visit_block(self, b);
            self.1 -= 1;
        }
        fn visit_item_const(&mut self, c: &'ast syn::ItemConst) {
            if self.1 > 1 {
                self.0.push(Item::Const(c.clone()));
            }
        }
        fn visit_item_fn(&mut self, _: &'ast ItemFn) {}
    }
    let mut out: Vec<Item> =
        f.block.stmts.iter().filter_map(|s| if let Stmt::Item(it) = s { Some(it.clone()) } else { None }).collect();
    let mut v = V(Vec::new(), 0);
    syn::visit::Visit::visit_block(&mut v, &f.block);
    out.extend(v.0);
    out
}

/// The interpreter's value as an integer, when it is one (an `X` or a non-scalar is
/// `None`).
fn ref_int(v: &Value) -> Option<i128> {
    match v {
        Value::Int(i, _) => Some(*i),
        Value::Bool(b) => Some(*b as i128),
        Value::Lit(u) => i128::try_from(*u).ok(),
        Value::Bits(b) => b.as_uint().and_then(|u| i128::try_from(u).ok()),
        // A packed vector register (`[Logic; N]`, an array of `Bits`): element 0 in the low
        // bits, the layout the transpiler states it in (`fold_const_init`'s `[elem; n]`). Reached
        // since MEASUREMENTS §U.3, when a `[Logic; N]` register's bit writes began to transpile.
        Value::Array(vs) => {
            let mut acc: u128 = 0;
            let mut off: u32 = 0;
            for v in vs {
                let Value::Bits(b) = v else { return None };
                acc |= b.as_uint()?.checked_shl(off)?;
                off = off.checked_add(b.width)?;
                if off > 127 {
                    return None;
                }
            }
            i128::try_from(acc).ok()
        }
        _ => None,
    }
}

fn width_of(ty: &CHIRType) -> Option<usize> {
    match ty {
        CHIRType::Bool => Some(1),
        CHIRType::UInt { width: Width::Concrete(w) } | CHIRType::SInt { width: Width::Concrete(w) } => Some(*w),
        _ => None,
    }
}

fn mask(v: u128, w: usize) -> u128 {
    if w >= 128 { v } else { v & ((1u128 << w) - 1) }
}

#[derive(Default)]
struct Tally {
    compared: usize,
    neither: usize,
}

struct Run {
    disagree: Vec<String>,
    asymmetric: BTreeMap<String, String>,
    items: Tally,
    inits_reg: Tally,
    inits_env: Tally,
    inits_not_compared: BTreeMap<&'static str, Vec<String>>,
    inits_generic: usize,
    bounds: Tally,
    indices: Tally,
    outside_items: Vec<String>,
}

impl Run {
    /// Both-ways comparison of one integer expression.
    fn both(&mut self, which: &'static str, site: String, a: Option<i128>, b: Result<Option<i128>, String>) {
        let tally = match which {
            "item" => &mut self.items,
            "bound" => &mut self.bounds,
            "index" => &mut self.indices,
            _ => &mut self.inits_env,
        };
        match (a, b) {
            (Some(x), Ok(Some(y))) => {
                tally.compared += 1;
                if x != y {
                    self.disagree.push(format!("{site}: copper_analysis {x}, copper-ref {y}"));
                }
            }
            (None, Ok(None)) | (None, Err(_)) => {
                tally.neither += 1;
                if which == "item" {
                    self.outside_items.push(site);
                }
            }
            (Some(x), r) => {
                self.asymmetric.insert(site, format!("copper_analysis {x}, copper-ref {r:?}"));
            }
            (None, Ok(Some(y))) => {
                self.asymmetric.insert(site, format!("copper_analysis none, copper-ref {y}"));
            }
        }
    }
}

/// The counted-`for` bounds and the index expressions of a body.
#[derive(Default)]
struct Sites {
    bounds: Vec<Expr>,
    indices: Vec<Expr>,
}

impl<'ast> Visit<'ast> for Sites {
    fn visit_expr_for_loop(&mut self, f: &'ast syn::ExprForLoop) {
        if let Expr::Range(r) = &*f.expr {
            self.bounds.extend(r.start.iter().map(|e| (**e).clone()));
            self.bounds.extend(r.end.iter().map(|e| (**e).clone()));
        }
        syn::visit::visit_expr_for_loop(self, f);
    }
    fn visit_expr_index(&mut self, i: &'ast syn::ExprIndex) {
        if !matches!(&*i.index, Expr::Range(_)) {
            self.indices.push((*i.index).clone());
        }
        syn::visit::visit_expr_index(self, i);
    }
    fn visit_expr_method_call(&mut self, m: &'ast syn::ExprMethodCall) {
        if (m.method == "get" && m.args.len() == 1) || (m.method == "set" && m.args.len() == 2) {
            self.indices.push(m.args[0].clone());
        }
        syn::visit::visit_expr_method_call(self, m);
    }
}

/// Every container of `const` items below the file scope — a `mod`'s items, a
/// block's statement items (a fn body, a test fn's) — as `(label, items)`.
#[derive(Default)]
struct Containers {
    out: Vec<(String, Vec<Item>)>,
    path: Vec<String>,
}

impl<'ast> Visit<'ast> for Containers {
    fn visit_item_mod(&mut self, m: &'ast syn::ItemMod) {
        self.path.push(format!("mod {}", m.ident));
        if let Some((_, items)) = &m.content {
            if items.iter().any(|i| matches!(i, Item::Const(_))) {
                self.out.push((self.path.join("::"), items.clone()));
            }
        }
        syn::visit::visit_item_mod(self, m);
        self.path.pop();
    }
    fn visit_item_fn(&mut self, f: &'ast ItemFn) {
        self.path.push(f.sig.ident.to_string());
        syn::visit::visit_item_fn(self, f);
        self.path.pop();
    }
    fn visit_impl_item_fn(&mut self, f: &'ast syn::ImplItemFn) {
        self.path.push(f.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, f);
        self.path.pop();
    }
    fn visit_block(&mut self, b: &'ast syn::Block) {
        let items: Vec<Item> =
            b.stmts.iter().filter_map(|s| if let Stmt::Item(it) = s { Some(it.clone()) } else { None }).collect();
        if items.iter().any(|i| matches!(i, Item::Const(_))) {
            self.out.push((self.path.join("::"), items));
        }
        syn::visit::visit_block(self, b);
    }
}

fn text(e: &impl ToTokens) -> String {
    e.to_token_stream().to_string()
}

#[test]
fn the_two_constant_evaluators_agree_on_every_corpus_constant() {
    let root = repo_root();
    let mut files = Vec::new();
    for dir in ["examples", "src", "tests"] {
        rs_files(&root.join(dir), &mut files);
    }
    files.sort();

    let mut run = Run {
        disagree: Vec::new(),
        asymmetric: BTreeMap::new(),
        items: Tally::default(),
        inits_reg: Tally::default(),
        inits_env: Tally::default(),
        inits_not_compared: BTreeMap::new(),
        inits_generic: 0,
        bounds: Tally::default(),
        indices: Tally::default(),
        outside_items: Vec::new(),
    };
    let mut hw_files = 0usize;
    let mut not_transpiled: BTreeSet<String> = BTreeSet::new();

    for path in &files {
        let Ok(src) = fs::read_to_string(path) else { continue };
        let Ok(file) = syn::parse_file(&src) else { continue };
        let mut fns = Vec::new();
        hardware_fns(&file.items, &mut fns);
        if fns.is_empty() {
            continue;
        }
        hw_files += 1;
        let rel = path.strip_prefix(&root).unwrap_or(path).display().to_string();
        let file_texts = const_source(&file.items);
        let env = copper_analysis::consts::ConstEnv::from_items(file_texts.iter().map(String::as_str));
        let scope = ConstScope::new(&file, None, &[]).expect("the interpreter collects the file's items");

        // 1. File-scope const items (`K`, `T::K`).
        for name in scope.const_names() {
            let b = scope.eval_const_named(&name).map(|v| ref_int(&v)).map_err(|e| format!("{e:?}"));
            run.both("item", format!("{rel}::const {name}"), env.get(&name), b);
        }

        // 1b. Nested containers' const items (a `mod`, a fn body — the hardware fns'
        // and the harness's), each over the file's items plus its own.
        let mut cont = Containers::default();
        cont.visit_file(&file);
        for (label, items) in &cont.out {
            let mut texts = file_texts.clone();
            texts.extend(const_source(items));
            let cenv = copper_analysis::consts::ConstEnv::from_items(texts.iter().map(String::as_str));
            let mut synth = file.clone();
            synth.items.extend(items.iter().cloned());
            let cscope = ConstScope::new(&synth, None, &[]).expect("the interpreter collects the container's items");
            for it in items {
                if let Item::Const(c) = it {
                    let name = c.ident.to_string();
                    let b = cscope.eval_const_named(&name).map(|v| ref_int(&v)).map_err(|e| format!("{e:?}"));
                    run.both("item", format!("{rel}::{label}::const {name}"), cenv.get(&name), b);
                }
            }
        }

        for f in &fns {
            let fname = f.sig.ident.to_string();
            let locals_items = body_items(f);
            // Body-local items join the file's for both evaluators.
            let (fenv, fscope) = if locals_items.is_empty() {
                (None, None)
            } else {
                let mut texts = file_texts.clone();
                texts.extend(const_source(&locals_items));
                (
                    Some(copper_analysis::consts::ConstEnv::from_items(texts.iter().map(String::as_str))),
                    Some(ConstScope::new(&file, Some(f), &[]).expect("body items collect")),
                )
            };
            let env = fenv.as_ref().unwrap_or(&env);
            let fs = fscope.as_ref().unwrap_or(&scope);

            // 3. Bounds and indices, every mode.
            let mut sites = Sites::default();
            sites.visit_block(&f.block);
            let none = HashMap::new();
            for (which, list) in [("bound", &sites.bounds), ("index", &sites.indices)] {
                for e in list {
                    let a = env.eval_expr(e, &none);
                    let b = fs.eval(e).map(|v| ref_int(&v)).map_err(|e| format!("{e:?}"));
                    run.both(which, format!("{rel}::{fname}::{which} `{}`", text(e)), a, b);
                }
            }

            // 2. Pre-loop initializers of a clocked module.
            if !matches!(hardware_mode(f).as_deref(), Some("sequential") | Some("synchronizer")) {
                continue;
            }
            let pre: Vec<&syn::Local> = f
                .block
                .stmts
                .iter()
                .take_while(|s| !matches!(s, Stmt::Expr(Expr::Loop(_), _)))
                .filter_map(|s| if let Stmt::Local(l) = s { Some(l) } else { None })
                .filter(|l| l.init.is_some())
                .collect();
            if pre.is_empty() {
                continue;
            }
            let shir = match copper_codegen::lower_source_module(&src, &fname) {
                Ok(low) => match low.shir.body {
                    SHIRBody::Sequential(s) => Some(s),
                    _ => None,
                },
                Err(_) => {
                    not_transpiled.insert(format!("{rel}::{fname}"));
                    None
                }
            };
            for l in pre {
                let (name, ty) = match &l.pat {
                    syn::Pat::Ident(pi) => (pi.ident.to_string(), None),
                    syn::Pat::Type(pt) => match &*pt.pat {
                        syn::Pat::Ident(pi) => (pi.ident.to_string(), Some((*pt.ty).clone())),
                        _ => continue,
                    },
                    _ => continue,
                };
                let init = &*l.init.as_ref().unwrap().expr;
                let site = format!("{rel}::{fname}::let {name} = {}", text(init));
                let rv = match &ty {
                    Some(t) => fs.eval_as(init, t),
                    None => fs.eval(init),
                };
                // (a) Against ConstEnv, wherever it evaluates the initializer.
                let a = env.eval_expr(init, &none);
                if a.is_some() {
                    run.both("init", site.clone(), a, rv.as_ref().map(ref_int).map_err(|e| format!("{e:?}")));
                }
                // (b) Against the transpiler's stated power-on value.
                let Some(shir) = &shir else {
                    if a.is_none() {
                        run.inits_not_compared.entry("the module does not transpile").or_default().push(site);
                    }
                    continue;
                };
                // The SHIR registers this binding became: itself, or one per field.
                let regs: Vec<&copper_core::shir::SHIRReg> = shir
                    .registers
                    .iter()
                    .filter(|r| r.name == name || r.name.starts_with(&format!("{name}_")))
                    .collect();
                let own = regs.iter().find(|r| r.name == name);
                // A const generic as the power-on value: the SV states it as the
                // parameter (`init_param`), not a number — no value to compare.
                if own.is_some_and(|r| r.init_param.is_some()) {
                    run.inits_generic += 1;
                    continue;
                }
                match (own, &rv) {
                    (Some(r), Ok(v)) => {
                        let Some(w) = width_of(&r.ty) else {
                            run.inits_not_compared.entry("a register with no scalar width (an array)").or_default().push(site);
                            continue;
                        };
                        let theirs = r.init.as_ref().map(|l| mask(l.value, w));
                        let ours = match v {
                            Value::Enum(en, var) => fs.enum_index(en, var).map(|i| i as u128),
                            Value::LitX => None,
                            Value::Bits(b) if !b.is_valid() => None,
                            other => ref_int(other).map(|i| mask(i as u128, w)),
                        };
                        // An X start states nothing in the SV; the interpreter has X.
                        let x_start = matches!(v, Value::LitX) || matches!(v, Value::Bits(b) if !b.is_valid());
                        if theirs.is_none() && x_start {
                            run.inits_reg.compared += 1;
                        } else if theirs.is_some() && theirs == ours {
                            run.inits_reg.compared += 1;
                        } else {
                            run.disagree.push(format!("{site}: transpiler states {theirs:?}, copper-ref {ours:?} ({v:?})"));
                        }
                    }
                    (None, Ok(Value::Struct(_, fields))) if !regs.is_empty() => {
                        for (fname_, fv) in fields {
                            let Some(r) = regs.iter().find(|r| r.name == format!("{name}_{fname_}")) else { continue };
                            let Some(w) = width_of(&r.ty) else {
                                run.inits_not_compared
                                    .entry("a register with no scalar width (an array)")
                                    .or_default()
                                    .push(format!("{site}.{fname_}"));
                                continue;
                            };
                            let theirs = r.init.as_ref().map(|l| mask(l.value, w));
                            let ours = ref_int(fv).map(|i| mask(i as u128, w));
                            if theirs == ours {
                                run.inits_reg.compared += 1;
                            } else {
                                run.disagree.push(format!("{site}.{fname_}: transpiler states {theirs:?}, copper-ref {ours:?}"));
                            }
                        }
                    }
                    (Some(_), Err(e)) => {
                        run.disagree.push(format!("{site}: a stated register the interpreter cannot evaluate: {e:?}"));
                    }
                    // A struct register the transpiler states field by field and the
                    // interpreter cannot evaluate: one evaluator only.
                    (None, Err(e)) if !regs.is_empty() => {
                        run.asymmetric.insert(
                            site,
                            format!("the transpiler states {} field register(s); copper-ref {e:?}", regs.len()),
                        );
                    }
                    _ => {
                        if a.is_none() {
                            let why = if rv.is_ok() {
                                "not a register (no stated value) and outside ConstEnv's integer grammar"
                            } else {
                                "not a constant (the interpreter refuses it in a const context)"
                            };
                            run.inits_not_compared.entry(why).or_default().push(site);
                        }
                    }
                }
            }
        }
    }

    // The pinned asymmetries, exact set.
    let expected: BTreeMap<String, String> = ASYMMETRIC.iter().map(|(s, r)| (s.to_string(), r.to_string())).collect();
    let new: Vec<String> = run
        .asymmetric
        .iter()
        .filter(|(s, _)| !expected.contains_key(*s))
        .map(|(s, v)| format!("{s} — {v}"))
        .collect();
    let stale: Vec<&String> = expected.keys().filter(|s| !run.asymmetric.contains_key(*s)).collect();

    let summary = format!(
        "constant evaluators: {hw_files} hardware files; const items {} compared / {} outside both; \
         pre-loop initializers {} stated registers vs the transpiler + {} vs ConstEnv, {} a const generic \
         (stated as the parameter), {} not compared; \
         counted-for bounds {} compared / {} not constants; indices {} compared / {} not constants; \
         {} asymmetric (pinned), {} DISAGREE",
        run.items.compared,
        run.items.neither,
        run.inits_reg.compared,
        run.inits_env.compared,
        run.inits_generic,
        run.inits_not_compared.values().map(Vec::len).sum::<usize>(),
        run.bounds.compared,
        run.bounds.neither,
        run.indices.compared,
        run.indices.neither,
        run.asymmetric.len(),
        run.disagree.len()
    );
    println!("{summary}");
    for (why, sites) in &run.inits_not_compared {
        println!("  initializers not compared — {why}: {}", sites.len());
        for s in sites {
            println!("    {s}");
        }
    }
    for s in &run.outside_items {
        println!("  const item outside both evaluators (not an integer): {s}");
    }
    for (s, why) in ASYMMETRIC {
        println!("  asymmetric (pinned): {s} — {why}");
    }
    println!("  clocked modules with pre-loop initializers the transpiler refuses (not compared): {not_transpiled:?}");

    assert!(run.disagree.is_empty(), "the two evaluators DISAGREE:\n  {}", run.disagree.join("\n  "));
    assert!(new.is_empty(), "unpinned asymmetries (one evaluator only):\n  {}", new.join("\n  "));
    assert!(stale.is_empty(), "stale ASYMMETRIC rows (both or neither evaluate now): {stale:?}");
    assert!(run.items.compared > 0, "no const item compared — the walk found no constants");
    assert!(run.inits_reg.compared > 0, "no register initializer compared against the transpiler");
    assert!(run.bounds.compared > 0, "no counted-for bound compared");
    assert!(run.indices.compared > 0, "no constant index compared");
    if let Ok(p) = std::env::var("COPPER_CONST_EVAL_REPORT") {
        fs::write(&p, format!("{summary}\n")).expect("write the constant-evaluator report");
    }
}

/// The grammar widened on 2026-11-08 (line `II.3 found const-call`, MEASUREMENTS §V.4),
/// against rustc: each item below was compiled by rustc 1.92 as a `const` (or `static`)
/// and its value printed; both evaluators must give rustc's value. `min`/`max` are not
/// here — rustc refuses them in a constant (E0658, `Ord` is not a const trait), and both
/// evaluators leave them outside.
#[test]
fn the_widened_constant_grammar_agrees_with_rustc() {
    let src = "struct P(u8, u8);
        const A: u8 = 250u8.wrapping_add(10);
        const B: u8 = 250u8.wrapping_sub(10);
        const C: u8 = 250u8.wrapping_mul(3);
        const D: i8 = (-128i8).wrapping_neg();
        const E: u16 = 250u16.wrapping_shl(9);
        const F: i16 = (-4i16).wrapping_shr(1);
        const G: u8 = 250u8.saturating_add(10);
        const H: u8 = 5u8.saturating_sub(10);
        const I: u8 = 3u8.pow(4);
        const J: u32 = 250u8.count_ones();
        const K: u32 = 250u8.count_zeros();
        const L: u32 = 1u8.leading_zeros();
        const M: u32 = 8u8.trailing_zeros();
        const N: bool = 8u8.is_power_of_two();
        const O: u8 = 5u8.next_power_of_two();
        const Q: i8 = (-5i8).abs();
        const R: u8 = P(3, 4).1;
        static SS: u8 = 7;
        const T: u8 = SS;
        const PK: P = P(9, 2);
        const U: u8 = PK.0.wrapping_add(PK.1);
        const MN: u8 = 3u8.min(4);";
    // rustc 1.92's values (`println!` of each), recorded.
    let rustc: &[(&str, i128)] = &[
        ("A", 4), ("B", 240), ("C", 238), ("D", -128), ("E", 62464), ("F", -2), ("G", 255), ("H", 0),
        ("I", 81), ("J", 6), ("K", 2), ("L", 7), ("M", 3), ("N", 1), ("O", 8), ("Q", 5), ("R", 4),
        ("T", 7), ("U", 11),
    ];
    let file: syn::File = syn::parse_str(src).unwrap();
    let env = copper_analysis::consts::ConstEnv::from_items(const_source(&file.items).iter().map(String::as_str));
    let scope = ConstScope::new(&file, None, &[]).unwrap();
    for (name, v) in rustc {
        assert_eq!(env.get(name), Some(*v), "ConstEnv, {name}");
        let r = scope.eval_const_named(name).ok().and_then(|x| ref_int(&x));
        assert_eq!(r, Some(*v), "the interpreter, {name}");
    }
    assert_eq!(env.get("MN"), None, "ConstEnv admits `min`, which rustc refuses in a constant");
    assert!(scope.eval_const_named("MN").is_err(), "the interpreter admits `min`, which rustc refuses in a constant");
}
