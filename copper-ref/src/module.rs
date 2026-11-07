//! [`Module`]: a `#[hardware(sequential)]` module instance run under the
//! one-resumption protocol (SPEC §I.0.2). See the crate docs for the protocol and
//! `README.md` for how to read a disagreement.

use crate::interp::{collect_tick_sites, Env, Exec, Flow, Program, TickSite};
use crate::types::{parse_ty, ConstEval};
use crate::value::{Ty, Value};
use crate::RefError;
use copper_core::Logic;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use syn::visit::Visit;
use syn::{Expr, Item, ItemFn, Pat, Stmt};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PortKind {
    In,
    RegOut,
}

#[derive(Clone, Debug)]
pub struct Port {
    pub name: String,
    pub ty: Ty,
    pub kind: PortKind,
}

#[derive(Clone, Debug, PartialEq)]
enum State {
    /// Before the first resumption: the entry of I.9.2.
    Entry,
    /// Parked at a tick site.
    Parked(TickSite),
}

/// A module instance: the kernel, its environment, its parked site and its ports.
pub struct Module {
    item: ItemFn,
    prog: Program,
    env: Env,
    state: State,
    committed: Vec<(String, Value)>,
    pending: HashMap<String, Value>,
    reached: BTreeSet<TickSite>,
    resumptions: usize,
    poison: Option<HashSet<String>>,
    mode: String,
}

impl std::fmt::Debug for Module {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Module")
            .field("name", &self.item.sig.ident.to_string())
            .field("state", &self.state)
            .field("resumptions", &self.resumptions)
            .finish()
    }
}

struct ProgEval<'a>(&'a Program);
impl<'a> ConstEval for ProgEval<'a> {
    fn eval_usize(&self, e: &Expr) -> Result<usize, RefError> {
        Exec::eval_const(self.0, e)?.as_index()
    }
}

/// The mode word of `#[hardware(mode[, flags])]` — read off the first token of the
/// attribute's list, never through `parse_args::<Ident>()`, which fails outright
/// once a flag is present (the CLAUDE.md gotcha).
pub fn hardware_mode(f: &ItemFn) -> Option<String> {
    for a in &f.attrs {
        if !a.path().segments.last().is_some_and(|s| s.ident == "hardware") {
            continue;
        }
        let syn::Meta::List(l) = &a.meta else { return Some(String::new()) };
        return match l.tokens.clone().into_iter().next() {
            Some(proc_macro2::TokenTree::Ident(i)) => Some(i.to_string()),
            _ => Some(String::new()),
        };
    }
    None
}

impl Module {
    /// Parse `src`, find the `#[hardware]` fn named `module`, bind its const
    /// generics from `params`, and check the module is one this interpreter gives a
    /// meaning to.
    pub fn new(src: &str, module: &str, params: &[(&str, u64)]) -> Result<Module, RefError> {
        let file = syn::parse_file(src).map_err(|e| RefError::Parse(format!("{e}")))?;
        let mut item: Option<ItemFn> = None;
        let mut prog = Program {
            consts: HashMap::new(),
            const_vals: RefCell::new(HashMap::new()),
            enums: HashMap::new(),
            structs: HashMap::new(),
            fns: HashMap::new(),
            methods: HashMap::new(),
            generics: HashMap::new(),
            clocks: Vec::new(),
            inputs: HashMap::new(),
            outputs: Vec::new(),
        };
        for it in &file.items {
            match it {
                Item::Fn(f) if f.sig.ident == module && hardware_mode(f).is_some() => {
                    if item.is_some() {
                        return Err(RefError::Parse(format!("two `#[hardware]` fns named `{module}`")));
                    }
                    item = Some(f.clone());
                }
                other => collect_item(other, &mut prog)?,
            }
        }
        let item = item.ok_or_else(|| {
            RefError::Parse(format!("no `#[hardware]` fn named `{module}` in the source"))
        })?;
        let mode = hardware_mode(&item).unwrap_or_default();
        match mode.as_str() {
            "sequential" | "synchronizer" => {}
            "combinational" => {
                return Err(RefError::Deferred(
                    "a `combinational` module: one evaluation per settle, SPEC I.11 (TBD, Round 3)".into(),
                ))
            }
            "structural" => {
                return Err(RefError::Deferred(
                    "a `structural` module: a wiring language, SPEC I.13 (TBD, Round 3)".into(),
                ))
            }
            other => return Err(RefError::Parse(format!("unknown #[hardware] mode `{other}`"))),
        }
        // Body-local items (a `const`, an `enum`, a helper `fn`).
        for s in &item.block.stmts {
            if let Stmt::Item(it) = s {
                collect_item(it, &mut prog)?;
            }
        }
        // Const generics.
        for g in &item.sig.generics.params {
            if let syn::GenericParam::Const(c) = g {
                let n = c.ident.to_string();
                let v = params
                    .iter()
                    .find(|(k, _)| *k == n)
                    .map(|(_, v)| *v)
                    .ok_or_else(|| RefError::Parse(format!("const generic `{n}` has no value in params")))?;
                prog.generics.insert(n, v);
            }
        }
        // Ports.
        let mut ports: Vec<Port> = Vec::new();
        for arg in &item.sig.inputs {
            let syn::FnArg::Typed(pt) = arg else {
                return Err(RefError::Parse("a `self` parameter on a module".into()));
            };
            let name = match &*pt.pat {
                Pat::Ident(pi) => pi.ident.to_string(),
                _ => return Err(RefError::Parse("a pattern parameter on a module".into())),
            };
            let syn::Type::Path(tp) = &*pt.ty else {
                return Err(RefError::Parse(format!("parameter `{name}` is not a port type")));
            };
            let seg = tp.path.segments.last().unwrap();
            let kind = seg.ident.to_string();
            let payload = || -> Result<Ty, RefError> {
                let syn::PathArguments::AngleBracketed(a) = &seg.arguments else {
                    return Err(RefError::Parse(format!("port `{name}` has no payload type")));
                };
                let Some(syn::GenericArgument::Type(t)) = a.args.first() else {
                    return Err(RefError::Parse(format!("port `{name}` has no payload type")));
                };
                let ty = parse_ty(t, &ProgEval(&prog))?;
                if ty.bit_width().is_none() {
                    return Err(RefError::Unsupported(format!(
                        "port `{name}` payload `{ty:?}` is not `Logic`, `Bits<N>` or an array of them (I.14)"
                    )));
                }
                Ok(ty)
            };
            match kind.as_str() {
                "Clock" => prog.clocks.push(name),
                "In" => {
                    let ty = payload()?;
                    prog.inputs.insert(name.clone(), ty.clone());
                    ports.push(Port { name, ty, kind: PortKind::In });
                }
                "RegOut" => {
                    let ty = payload()?;
                    prog.outputs.push((name.clone(), ty.clone()));
                    ports.push(Port { name, ty, kind: PortKind::RegOut });
                }
                "Out" => {
                    return Err(RefError::Deferred(format!(
                        "plain `Out` port `{name}` on a clocked module: a sequential module has no plain \
                         `Out` under the one-job body (D-8, SPEC I.2.2); the module awaits the Phase C \
                         rewrite to `RegOut`"
                    )))
                }
                "Memory" => {
                    return Err(RefError::Deferred(format!(
                        "`Memory` parameter `{name}`: memories are SPEC I.10 (TBD, Round 2)"
                    )))
                }
                other => {
                    return Err(RefError::Unsupported(format!(
                        "parameter `{name}: {other}<…>` is not a port kind of I.2.2"
                    )))
                }
            }
        }
        if prog.clocks.len() > 1 {
            return Err(RefError::Deferred(format!(
                "{} clock parameters: per-domain gating is SPEC I.12 (TBD, Round 3)",
                prog.clocks.len()
            )));
        }
        if prog.clocks.is_empty() {
            return Err(RefError::Parse("a clocked module with no `Clock` parameter".into()));
        }
        if prog.outputs.is_empty() {
            return Err(RefError::Parse("a module with no output (I.2.2: at least one output)".into()));
        }
        // Shape of the body: `s_pre ; loop { s }`, the loop last (I.2.2).
        let loop_idx = item
            .block
            .stmts
            .iter()
            .position(|s| matches!(s, Stmt::Expr(Expr::Loop(_), _)))
            .ok_or_else(|| RefError::Parse("no top-level `loop` in the module body (I.2.2)".into()))?;
        if loop_idx + 1 != item.block.stmts.len() {
            return Err(RefError::Unsupported("statements after the module loop (I.2.2: the loop is last)".into()));
        }
        let mut scan = Scan { prog: &prog, pre_loop: true, err: None };
        for (i, s) in item.block.stmts.iter().enumerate() {
            scan.pre_loop = i < loop_idx;
            scan.visit_stmt(s);
            if let Some(e) = scan.err.take() {
                return Err(e);
            }
        }
        let committed = prog
            .outputs
            .iter()
            .map(|(n, t)| Ok((n.clone(), Value::zero_of(t)?)))
            .collect::<Result<Vec<_>, RefError>>()?;
        Ok(Module {
            item,
            prog,
            env: Env::default(),
            state: State::Entry,
            committed,
            pending: HashMap::new(),
            reached: BTreeSet::new(),
            resumptions: 0,
            poison: None,
            mode,
        })
    }

    /// The poison run (SPEC §I.0.5(a), §I.7.7): after every suspension, overwrite
    /// each named local with a poison value; a read of one before its
    /// reassignment fails the next `resume` with [`RefError::Poisoned`].
    pub fn with_poison(mut self, targets: HashSet<String>) -> Self {
        self.poison = Some(targets);
        self
    }

    pub fn item_fn(&self) -> &ItemFn {
        &self.item
    }

    pub fn mode(&self) -> &str {
        &self.mode
    }

    pub fn ports(&self) -> Vec<Port> {
        let mut out = Vec::new();
        for arg in &self.item.sig.inputs {
            if let syn::FnArg::Typed(pt) = arg {
                if let Pat::Ident(pi) = &*pt.pat {
                    let n = pi.ident.to_string();
                    if let Some(t) = self.prog.inputs.get(&n) {
                        out.push(Port { name: n, ty: t.clone(), kind: PortKind::In });
                    } else if let Some((_, t)) = self.prog.outputs.iter().find(|(o, _)| *o == n) {
                        out.push(Port { name: n, ty: t.clone(), kind: PortKind::RegOut });
                    }
                }
            }
        }
        out
    }

    pub fn input_ports(&self) -> Vec<Port> {
        self.ports().into_iter().filter(|p| p.kind == PortKind::In).collect()
    }

    pub fn output_ports(&self) -> Vec<Port> {
        self.ports().into_iter().filter(|p| p.kind == PortKind::RegOut).collect()
    }

    /// Every `tick` site of the kernel, statically (the states of H(M), I.0.4).
    pub fn tick_sites(&self) -> BTreeSet<TickSite> {
        let mut out = BTreeSet::new();
        collect_tick_sites(&self.item.block.stmts, &mut Vec::new(), &mut out);
        out
    }

    /// The sites some resumption so far has parked at.
    pub fn reached_sites(&self) -> &BTreeSet<TickSite> {
        &self.reached
    }

    /// The site the coroutine is parked at; `None` before the first resumption.
    pub fn parked_at(&self) -> Option<&TickSite> {
        match &self.state {
            State::Entry => None,
            State::Parked(s) => Some(s),
        }
    }

    pub fn resumptions(&self) -> usize {
        self.resumptions
    }

    /// The locals currently alive in the coroutine's environment.
    pub fn live_locals(&self) -> BTreeSet<String> {
        self.env.live_names()
    }

    /// The names assigned inside the module loop — SPEC I.7.1 (i): a `let`, an
    /// assignment target (through index, field or tuple), a `for` binding, a
    /// `Bits::set` receiver. Match-arm bindings are not counted (they are the
    /// arm's wires, as the inference's `DefinedInLoop` also excludes them).
    pub fn assigned_in_loop(&self) -> BTreeSet<String> {
        let mut v = Assigned { out: BTreeSet::new() };
        for s in &self.item.block.stmts {
            if let Stmt::Expr(Expr::Loop(l), _) = s {
                v.visit_block(&l.body);
            }
        }
        v.out
    }

    /// ONE resumption (SPEC I.0.2 step 1): from the entry on the first call, else
    /// from the parked site, to the next `tick`. Every `In::read()` yields the
    /// value given here — `x_{N+1}`. Returns the site parked at.
    pub fn resume(&mut self, inputs: &[(&str, Value)]) -> Result<TickSite, RefError> {
        let mut map = HashMap::new();
        for (name, ty) in &self.prog.inputs {
            let v = inputs
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.clone())
                .ok_or_else(|| RefError::Runtime(format!("no value for input `{name}` this resumption")))?;
            map.insert(name.clone(), v.coerce_to(ty)?);
        }
        let resume_path: Option<Vec<usize>> = match &self.state {
            State::Entry => None,
            State::Parked(s) => Some(s.0.clone()),
        };
        let flow = {
            let mut ex = Exec::new(&self.prog, &mut self.env, &map, &mut self.pending);
            ex.exec_block(&self.item.block.stmts, 0, resume_path.as_deref(), Vec::new())?
        };
        self.resumptions += 1;
        match flow {
            Flow::Suspend(p) => {
                let site = TickSite(p);
                self.reached.insert(site.clone());
                self.state = State::Parked(site.clone());
                if let Some(t) = &self.poison {
                    self.env.poison(t);
                }
                Ok(site)
            }
            Flow::Done => Err(RefError::Unsupported(
                "the module body returned: a module never returns (I.2.5(d))".into(),
            )),
            Flow::Break => Err(RefError::Unsupported(
                "`break` of the module loop (I.2.5(d): OUTSIDE; the simulator panics)".into(),
            )),
            Flow::Continue => Err(RefError::Runtime("`continue` escaped the module loop".into())),
        }
    }

    /// `resume` with the inputs as bit vectors (LSB first, element-major), the
    /// harness's layout.
    pub fn resume_bits(&mut self, inputs: &[(&str, &[Logic])]) -> Result<TickSite, RefError> {
        let mut vals = Vec::new();
        for (name, bits) in inputs {
            let Some(ty) = self.prog.inputs.get(*name) else {
                return Err(RefError::Runtime(format!("`{name}` is not an input port")));
            };
            vals.push((*name, Value::from_bits(ty, bits)?));
        }
        self.resume(&vals)
    }

    /// The edge (SPEC I.0.2 step 2, I.6.1): every pending `RegOut` value takes
    /// effect; the locals — the registers — are already their new values.
    pub fn edge(&mut self) {
        for (name, v) in self.pending.drain() {
            if let Some(slot) = self.committed.iter_mut().find(|(n, _)| *n == name) {
                slot.1 = v;
            }
        }
    }

    /// Every `RegOut`'s committed value, in signature order: `obs_N` after the
    /// N-th edge; `init(q)` before the first (I.6.6, I.9.4).
    pub fn observe(&self) -> Vec<(String, Value)> {
        self.committed.clone()
    }

    pub fn observe_bits(&self) -> Vec<(String, Vec<Logic>)> {
        self.committed
            .iter()
            .map(|(n, v)| (n.clone(), v.to_bits().expect("a RegOut holds a port value")))
            .collect()
    }
}

fn collect_item(it: &Item, prog: &mut Program) -> Result<(), RefError> {
    match it {
        Item::Const(c) => {
            prog.consts.insert(c.ident.to_string(), (Some((*c.ty).clone()), (*c.expr).clone()));
        }
        Item::Enum(e) => {
            let mut vs = Vec::new();
            for v in &e.variants {
                if !matches!(v.fields, syn::Fields::Unit) {
                    // A variant with fields is outside the value sets of I.14;
                    // refused where it is used, not here (the file may hold a
                    // model enum beside the hardware).
                    continue;
                }
                vs.push(v.ident.to_string());
            }
            prog.enums.insert(e.ident.to_string(), vs);
        }
        Item::Struct(s) => {
            let fields = match &s.fields {
                syn::Fields::Named(n) => n.named.iter().filter_map(|f| f.ident.as_ref().map(|i| i.to_string())).collect(),
                _ => Vec::new(),
            };
            prog.structs.insert(s.ident.to_string(), fields);
        }
        Item::Fn(f) => {
            if hardware_mode(f).is_none() {
                prog.fns.insert(f.sig.ident.to_string(), f.clone());
            }
        }
        Item::Impl(im) => {
            if im.trait_.is_some() {
                return Ok(());
            }
            let syn::Type::Path(tp) = &*im.self_ty else { return Ok(()) };
            let ty = tp.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
            for ii in &im.items {
                if let syn::ImplItem::Fn(f) = ii {
                    prog.methods.insert((ty.clone(), f.sig.ident.to_string()), f.clone());
                }
                // An associated const is a const of I.14.4 named `T::K` (Phase C
                // step 5, II.3 (d), MEASUREMENTS §J.13).
                if let syn::ImplItem::Const(c) = ii {
                    prog.consts.insert(format!("{ty}::{}", c.ident), (Some(c.ty.clone()), c.expr.clone()));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// The eager refusals: shapes outside the kernel that the macro admits today
/// (II.1's parity list), refused at construction so a module is refused whether
/// or not the stimulus reaches the construct.
struct Scan<'a> {
    prog: &'a Program,
    pre_loop: bool,
    err: Option<RefError>,
}

impl<'a> Scan<'a> {
    fn fail(&mut self, e: RefError) {
        if self.err.is_none() {
            self.err = Some(e);
        }
    }
}

impl<'ast, 'a> Visit<'ast> for Scan<'a> {
    fn visit_expr(&mut self, e: &'ast Expr) {
        match e {
            Expr::Await(a) => {
                let is_tick = matches!(&*a.base, Expr::MethodCall(mc)
                    if mc.method == "tick" && matches!(&*mc.receiver, Expr::Path(p)
                        if p.path.segments.len() == 1 && self.prog.clocks.iter().any(|c| p.path.segments[0].ident == c)));
                if !is_tick {
                    self.fail(RefError::Unsupported(
                        "an `await` that is not `clk.tick().await` (II.1 R1)".into(),
                    ));
                } else if self.pre_loop {
                    self.fail(RefError::Unsupported(
                        "a `tick` before the module loop (II.1 R21: `s_pre` is tick-free; II.3 row (f6))".into(),
                    ));
                }
            }
            Expr::Macro(m) => {
                let name = m.mac.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
                if name != "matches" {
                    self.fail(RefError::Unsupported(format!(
                        "macro `{name}!` (II.1 R13/R14: statement macros and reads inside macro tokens are refused)"
                    )));
                }
            }
            Expr::Return(_) => self.fail(RefError::Unsupported("`return` (I.2.5(d): OUTSIDE)".into())),
            Expr::Break(b) if b.label.is_some() || b.expr.is_some() => {
                self.fail(RefError::Unsupported("a labelled or valued `break` (II.1 R12)".into()))
            }
            Expr::Continue(c) if c.label.is_some() => {
                self.fail(RefError::Unsupported("a labelled `continue` (II.1 R12)".into()))
            }
            Expr::Closure(_) => self.fail(RefError::Unsupported(
                "a closure (I.2.4: OUTSIDE outside a `Memory` preload)".into(),
            )),
            Expr::Unsafe(_) | Expr::Async(_) | Expr::Yield(_) | Expr::Try(_) | Expr::TryBlock(_) => {
                self.fail(RefError::Unsupported("`unsafe` / nested `async` / `yield` / `?` (I.2.5)".into()))
            }
            Expr::Path(p) => {
                let first = p.path.segments.first().map(|s| s.ident.to_string()).unwrap_or_default();
                if first == "Memory" {
                    self.fail(RefError::Deferred(
                        "a `Memory` binding in the body: memories are SPEC I.10 (TBD, Round 2)".into(),
                    ));
                }
            }
            Expr::MethodCall(mc) if matches!(mc.method.to_string().as_str(), "read_port" | "write_port") => {
                self.fail(RefError::Deferred(
                    "a memory port access: memories are SPEC I.10 (TBD, Round 2)".into(),
                ));
            }
            _ => {}
        }
        syn::visit::visit_expr(self, e);
    }

    fn visit_stmt_macro(&mut self, m: &'ast syn::StmtMacro) {
        let name = m.mac.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
        self.fail(RefError::Unsupported(format!(
            "statement macro `{name}!` (II.1 R13: admitted by the macro with unrewritten reads, refused by the transpiler)"
        )));
    }

    fn visit_local(&mut self, l: &'ast syn::Local) {
        if let Some(init) = &l.init {
            if init.diverge.is_some() {
                self.fail(RefError::Unsupported("`let … else` (I.2.5: OUTSIDE)".into()));
            }
        }
        syn::visit::visit_local(self, l);
    }

    fn visit_expr_if(&mut self, i: &'ast syn::ExprIf) {
        if let Expr::Let(_) = &*i.cond {
            self.fail(RefError::Unsupported("`if let` (I.2.5: OUTSIDE)".into()));
        }
        syn::visit::visit_expr_if(self, i);
    }
}

/// Names assigned inside the loop (SPEC I.7.1 (i)).
struct Assigned {
    out: BTreeSet<String>,
}

fn place_base(e: &Expr) -> Option<String> {
    match e {
        Expr::Path(p) if p.path.segments.len() == 1 => Some(p.path.segments[0].ident.to_string()),
        Expr::Index(ix) => place_base(&ix.expr),
        Expr::Field(f) => place_base(&f.base),
        Expr::Paren(p) => place_base(&p.expr),
        Expr::Unary(u) => place_base(&u.expr),
        _ => None,
    }
}

fn pat_names(p: &Pat, out: &mut BTreeSet<String>) {
    match p {
        Pat::Ident(pi) => {
            out.insert(pi.ident.to_string());
        }
        Pat::Tuple(t) => t.elems.iter().for_each(|e| pat_names(e, out)),
        Pat::Type(pt) => pat_names(&pt.pat, out),
        Pat::Paren(pp) => pat_names(&pp.pat, out),
        _ => {}
    }
}

impl<'ast> Visit<'ast> for Assigned {
    fn visit_local(&mut self, l: &'ast syn::Local) {
        pat_names(&l.pat, &mut self.out);
        syn::visit::visit_local(self, l);
    }
    fn visit_expr_assign(&mut self, a: &'ast syn::ExprAssign) {
        match &*a.left {
            Expr::Tuple(t) => {
                for e in &t.elems {
                    if let Some(n) = place_base(e) {
                        self.out.insert(n);
                    }
                }
            }
            other => {
                if let Some(n) = place_base(other) {
                    self.out.insert(n);
                }
            }
        }
        syn::visit::visit_expr_assign(self, a);
    }
    fn visit_expr_binary(&mut self, b: &'ast syn::ExprBinary) {
        let compound = matches!(
            b.op,
            syn::BinOp::AddAssign(_)
                | syn::BinOp::SubAssign(_)
                | syn::BinOp::MulAssign(_)
                | syn::BinOp::DivAssign(_)
                | syn::BinOp::RemAssign(_)
                | syn::BinOp::BitXorAssign(_)
                | syn::BinOp::BitAndAssign(_)
                | syn::BinOp::BitOrAssign(_)
                | syn::BinOp::ShlAssign(_)
                | syn::BinOp::ShrAssign(_)
        );
        if compound {
            if let Some(n) = place_base(&b.left) {
                self.out.insert(n);
            }
        }
        syn::visit::visit_expr_binary(self, b);
    }
    fn visit_expr_for_loop(&mut self, f: &'ast syn::ExprForLoop) {
        pat_names(&f.pat, &mut self.out);
        syn::visit::visit_expr_for_loop(self, f);
    }
    fn visit_expr_method_call(&mut self, mc: &'ast syn::ExprMethodCall) {
        if mc.method == "set" {
            if let Some(n) = place_base(&mc.receiver) {
                self.out.insert(n);
            }
        }
        syn::visit::visit_expr_method_call(self, mc);
    }
    // Arm bindings are the arm's wires: not counted.
    fn visit_arm(&mut self, a: &'ast syn::Arm) {
        if let Some((_, g)) = &a.guard {
            self.visit_expr(g);
        }
        self.visit_expr(&a.body);
    }
}
