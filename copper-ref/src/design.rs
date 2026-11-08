//! The design leg (SPEC §I.13, D-18): a `#[hardware(structural)]` module is no
//! coroutine and no function but the **design** it names (I.13.4(a)) — a set of
//! interpreter instances over shared wires — so it is run by composing its
//! children, never by executing its body.
//!
//! | clause        | here                                                               |
//! |---------------|--------------------------------------------------------------------|
//! | I.13.1        | [`Design::new`]: instances and wires; one driver per wire (R22), every net driven, the payload type checked on both ends |
//! | I.13.4(b)     | the body read as a wiring language: net declarations and instantiations, nothing else |
//! | I.13.4(c)     | a net carries no value of its own: its value is its driver's       |
//! | I.13.2(a)     | [`Design::settle`]: every `combinational` instance evaluated once, in a topological order of the network, from the sources (the committed `RegOut` wires and the environment's inputs) |
//! | I.11.4, D-14 A4 | a combinational cycle is OUTSIDE: refused at construction, naming its instances |
//! | I.13.2(b), I.12.1 | [`Design::pre_edge`] / [`Design::post_edge`]: settle on `(q_N, x_{N+1})`, resume every coroutine of the domain, the edge (its pending values and memories), settle on `(q_{N+1}, x_{N+1})` |
//!
//! A domain is named by the parent's `Clock` parameter the child receives; a child
//! in another domain is not resumed (I.12.1: only `D`'s coroutines). A child's
//! `In` reads the settled wire at its resumption, whatever domain drives it — the
//! one crossing being a `synchronizer`'s foreign `In` (I.12.4(a)), which the
//! front-ends check, not this leg.

use crate::module::{hardware_mode, Module, PortKind};
use crate::types::{parse_ty, ConstEval};
use crate::value::{Ty, Value};
use crate::{ConstScope, RefError};
use std::collections::BTreeMap;
use syn::{Expr, Item, ItemFn, Pat, Stmt};

/// Who drives a wire (I.13.1 (i)–(iii)).
#[derive(Clone, Debug, PartialEq)]
enum Driver {
    /// The environment: a parent `In` port.
    Env,
    /// Output `port` of instance `inst`.
    Inst(usize, String),
}

#[derive(Clone, Debug)]
struct Wire {
    name: String,
    ty: Ty,
    driver: Option<Driver>,
    value: Value,
}

struct Inst {
    label: String,
    module: Module,
    /// The parent `Clock` parameter this child runs on; `None` for a function.
    clock: Option<String>,
    /// Child `In` port → wire.
    reads: Vec<(String, usize)>,
    /// Child output port → wire.
    drives: Vec<(String, usize)>,
}

/// A design: the instances a structural module names, over its wires.
pub struct Design {
    name: String,
    insts: Vec<Inst>,
    wires: Vec<Wire>,
    /// Parent `In` ports → wire, `Out`/`RegOut` ports → wire, in signature order.
    inputs: Vec<(String, usize)>,
    outputs: Vec<(String, usize)>,
    clocks: Vec<String>,
    /// The combinational instances in a topological order of the network.
    comb_order: Vec<usize>,
}

struct ScopeEval<'a>(&'a ConstScope);
impl<'a> ConstEval for ScopeEval<'a> {
    fn eval_usize(&self, e: &Expr) -> Result<usize, RefError> {
        self.0.eval(e)?.as_index()
    }
}

fn unsup(s: impl Into<String>) -> RefError {
    RefError::Unsupported(s.into())
}

/// `Kind<T, D>` of a parameter type: the kind word and the payload type.
fn port_shape(t: &syn::Type) -> Option<(String, Option<syn::Type>)> {
    let syn::Type::Path(tp) = t else { return None };
    let seg = tp.path.segments.last()?;
    let payload = match &seg.arguments {
        syn::PathArguments::AngleBracketed(a) => match a.args.first() {
            Some(syn::GenericArgument::Type(t)) => Some(t.clone()),
            _ => None,
        },
        _ => None,
    };
    Some((seg.ident.to_string(), payload))
}

impl Design {
    /// Parse `src`, find the `#[hardware(structural)]` fn `top` and build the design
    /// it names, each child a [`Module`] of the same source file.
    pub fn new(src: &str, top: &str) -> Result<Design, RefError> {
        let file = syn::parse_file(src).map_err(|e| RefError::Parse(format!("{e}")))?;
        let fns: BTreeMap<String, &ItemFn> = file
            .items
            .iter()
            .filter_map(|it| match it {
                Item::Fn(f) if hardware_mode(f).is_some() => Some((f.sig.ident.to_string(), f)),
                _ => None,
            })
            .collect();
        let item = *fns.get(top).ok_or_else(|| RefError::Parse(format!("no `#[hardware]` fn named `{top}`")))?;
        if hardware_mode(item).as_deref() != Some("structural") {
            return Err(RefError::Parse(format!("`{top}` is not a `#[hardware(structural)]` module")));
        }
        if !item.sig.generics.params.is_empty() {
            return Err(unsup("a generic structural module (no instance in the corpus)"));
        }
        let scope = ConstScope::new(&file, None, &[])?;
        let ev = ScopeEval(&scope);
        let mut d = Design {
            name: top.to_string(),
            insts: Vec::new(),
            wires: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            clocks: Vec::new(),
            comb_order: Vec::new(),
        };
        // Parent ports: a clock, an environment-driven input wire, or an output wire
        // some child drives.
        let mut names: BTreeMap<String, usize> = BTreeMap::new();
        for arg in &item.sig.inputs {
            let syn::FnArg::Typed(pt) = arg else { return Err(RefError::Parse("`self` on a module".into())) };
            let Pat::Ident(pi) = &*pt.pat else { return Err(RefError::Parse("a pattern parameter".into())) };
            let n = pi.ident.to_string();
            let (kind, payload) = port_shape(&pt.ty).ok_or_else(|| RefError::Parse(format!("parameter `{n}`")))?;
            match kind.as_str() {
                "Clock" => d.clocks.push(n),
                "In" | "Out" | "RegOut" => {
                    let ty = parse_ty(&payload.ok_or_else(|| RefError::Parse(format!("`{n}` payload")))?, &ev)?;
                    let w = d.wires.len();
                    let driver = (kind == "In").then_some(Driver::Env);
                    d.wires.push(Wire { name: n.clone(), value: Value::zero_of(&ty)?, ty, driver });
                    names.insert(n.clone(), w);
                    if kind == "In" {
                        d.inputs.push((n, w));
                    } else {
                        d.outputs.push((n, w));
                    }
                }
                other => {
                    return Err(unsup(format!(
                        "a `{other}` parameter `{n}` on a structural module (I.13.4(e): a `Memory` is owned by the one child it reaches; no corpus instance)"
                    )))
                }
            }
        }
        // The body: nets and instantiations (I.13.4(b)).
        for s in &item.block.stmts {
            match s {
                Stmt::Local(l) => {
                    let Pat::Ident(pi) = &l.pat else { return Err(unsup("a net bound by a pattern")) };
                    let n = pi.ident.to_string();
                    let init = l.init.as_ref().ok_or_else(|| unsup(format!("`let {n};` in a structural body")))?;
                    let ty = net_type(&init.expr, &ev)
                        .ok_or_else(|| unsup(format!("`let {n} = …` that is not a net declaration (I.13.4(b))")))??;
                    let w = d.wires.len();
                    d.wires.push(Wire { name: n.clone(), value: Value::zero_of(&ty)?, ty, driver: None });
                    names.insert(n, w);
                }
                Stmt::Expr(Expr::Call(c), _) => d.instantiate(c, src, &fns, &names)?,
                Stmt::Item(_) => {}
                _ => {
                    return Err(unsup(
                        "a statement of a structural body that is neither a net nor an instantiation (I.13.4(b))",
                    ))
                }
            }
        }
        // I.13.1: exactly one driver per wire — every net and every parent output driven.
        for w in &d.wires {
            if w.driver.is_none() {
                return Err(unsup(format!(
                    "wire `{}` of `{top}` has no driver (I.13.1: exactly one driver; I.13.4(d): every net driven)",
                    w.name
                )));
            }
        }
        d.comb_order = d.topological_order()?;
        Ok(d)
    }

    fn instantiate(
        &mut self,
        c: &syn::ExprCall,
        src: &str,
        fns: &BTreeMap<String, &ItemFn>,
        names: &BTreeMap<String, usize>,
    ) -> Result<(), RefError> {
        let Expr::Path(fp) = &*c.func else { return Err(unsup("an instantiation through a non-path callee")) };
        if fp.path.segments.len() != 1 || !fp.path.segments[0].arguments.is_empty() {
            return Err(unsup("an instantiation with a path or turbofish (no corpus instance)"));
        }
        let child = fp.path.segments[0].ident.to_string();
        let f = *fns
            .get(&child)
            .ok_or_else(|| unsup(format!("`{child}(…)` does not name a `#[hardware]` module (I.13.4(d))")))?;
        if child == self.name {
            return Err(unsup("a structural module instantiating itself (I.13.4(d): no recursion)"));
        }
        if hardware_mode(f).as_deref() == Some("structural") {
            return Err(unsup(format!("a structural child `{child}` (a nested design; no corpus instance)")));
        }
        let module = Module::new(src, &child, &[])?;
        let params: Vec<&syn::FnArg> = f.sig.inputs.iter().collect();
        if params.len() != c.args.len() {
            return Err(unsup(format!("`{child}` takes {} arguments, {} given (I.13.4(d))", params.len(), c.args.len())));
        }
        let idx = self.insts.len();
        let label = format!("{child}#{idx}");
        let mut inst = Inst { label: label.clone(), module, clock: None, reads: Vec::new(), drives: Vec::new() };
        let kinds: BTreeMap<String, PortKind> =
            inst.module.ports().into_iter().map(|p| (p.name, p.kind)).collect();
        let port_tys: BTreeMap<String, Ty> = inst.module.ports().into_iter().map(|p| (p.name, p.ty)).collect();
        for (p, a) in params.iter().zip(c.args.iter()) {
            let syn::FnArg::Typed(pt) = p else { unreachable!() };
            let Pat::Ident(pi) = &*pt.pat else { return Err(unsup("a pattern parameter")) };
            let pname = pi.ident.to_string();
            let (kind, _) = port_shape(&pt.ty).unwrap_or_default();
            if kind == "Clock" {
                let clk = clock_arg(a).ok_or_else(|| unsup(format!("`{child}`'s clock `{pname}`: a parent clock")))?;
                if !self.clocks.contains(&clk) {
                    return Err(unsup(format!("`{child}`'s clock `{clk}` is not a clock of `{}`", self.name)));
                }
                inst.clock = Some(clk);
                continue;
            }
            let (wname, end) = wire_arg(a).ok_or_else(|| {
                unsup(format!("argument for `{child}`'s `{pname}`: a parent port, `n.0` or `n.1` (I.13.4(b))"))
            })?;
            let w = *names.get(&wname).ok_or_else(|| unsup(format!("`{wname}` is not a net or port of `{}`", self.name)))?;
            let ty = port_tys.get(&pname).cloned().ok_or_else(|| unsup(format!("`{child}` port `{pname}`")))?;
            if ty != self.wires[w].ty {
                return Err(unsup(format!(
                    "`{child}`'s `{pname}` is `{ty:?}`, the wire `{wname}` `{:?}` (I.13.1: wiring is type-checked)",
                    self.wires[w].ty
                )));
            }
            match kinds.get(&pname) {
                Some(PortKind::In) => {
                    if end == Some(0) {
                        return Err(unsup(format!("`{child}` reads `{wname}.0`, the driver end")));
                    }
                    inst.reads.push((pname, w));
                }
                Some(PortKind::RegOut) | Some(PortKind::Out) => {
                    if end == Some(1) {
                        return Err(unsup(format!("`{child}` drives `{wname}.1`, the reader end")));
                    }
                    if self.wires[w].driver.is_some() {
                        return Err(unsup(format!(
                            "wire `{wname}` has a second driver `{child}.{pname}` (I.13.1, II.1.2 R22: one driver)"
                        )));
                    }
                    self.wires[w].driver = Some(Driver::Inst(idx, pname.clone()));
                    inst.drives.push((pname, w));
                }
                None => return Err(unsup(format!("`{child}` has no port `{pname}`"))),
            }
        }
        if !inst.module.is_combinational() && inst.clock.is_none() {
            return Err(unsup(format!("`{label}` is clocked and was given no clock")));
        }
        self.insts.push(inst);
        Ok(())
    }

    /// I.13.2(a): the network is a DAG from the sources through the functions; a
    /// cycle among `combinational` instances is OUTSIDE (I.11.4, D-14 A4).
    fn topological_order(&self) -> Result<Vec<usize>, RefError> {
        let comb: Vec<usize> = (0..self.insts.len()).filter(|&i| self.insts[i].module.is_combinational()).collect();
        // A → B when a wire A drives is read by B.
        let mut indeg: BTreeMap<usize, usize> = comb.iter().map(|&i| (i, 0)).collect();
        let mut succ: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for &b in &comb {
            for (_, w) in &self.insts[b].reads {
                if let Some(Driver::Inst(a, _)) = &self.wires[*w].driver {
                    if self.insts[*a].module.is_combinational() {
                        succ.entry(*a).or_default().push(b);
                        *indeg.get_mut(&b).unwrap() += 1;
                    }
                }
            }
        }
        let mut ready: Vec<usize> = indeg.iter().filter(|(_, d)| **d == 0).map(|(i, _)| *i).collect();
        let mut order = Vec::new();
        while let Some(a) = ready.pop() {
            order.push(a);
            for b in succ.get(&a).cloned().unwrap_or_default() {
                let d = indeg.get_mut(&b).unwrap();
                *d -= 1;
                if *d == 0 {
                    ready.push(b);
                }
            }
        }
        if order.len() != comb.len() {
            let cyc: Vec<&str> = comb
                .iter()
                .filter(|i| !order.contains(i))
                .map(|&i| self.insts[i].label.as_str())
                .collect();
            return Err(unsup(format!(
                "a combinational cycle through {} — OUTSIDE the language (SPEC I.11.4, D-14 A4): the settle \
                 is a fixpoint problem, not a function",
                cyc.join(", ")
            )));
        }
        Ok(order)
    }

    pub fn clocks(&self) -> &[String] {
        &self.clocks
    }

    pub fn instance_labels(&self) -> Vec<String> {
        self.insts.iter().map(|i| i.label.clone()).collect()
    }

    /// The environment drives a parent input (I.1.5).
    pub fn drive(&mut self, input: &str, v: Value) -> Result<(), RefError> {
        let (_, w) = *self
            .inputs
            .iter()
            .find(|(n, _)| n == input)
            .ok_or_else(|| RefError::Runtime(format!("`{input}` is not an input of `{}`", self.name)))?;
        let ty = self.wires[w].ty.clone();
        self.wires[w].value = v.coerce_to(&ty)?;
        Ok(())
    }

    fn read_ins(&self, i: usize) -> Vec<(String, Value)> {
        self.insts[i].reads.iter().map(|(p, w)| (p.clone(), self.wires[*w].value.clone())).collect()
    }

    fn publish(&mut self, i: usize) {
        let obs = self.insts[i].module.observe();
        for (p, w) in self.insts[i].drives.clone() {
            if let Some((_, v)) = obs.iter().find(|(n, _)| *n == p) {
                self.wires[w].value = v.clone();
            }
        }
    }

    /// The settle (I.13.2(a)): every `RegOut` wire takes its driver's committed
    /// value (a source, I.6.4), then every function is evaluated once in the
    /// network's topological order over the settled values of its reads.
    pub fn settle(&mut self) -> Result<(), RefError> {
        for i in 0..self.insts.len() {
            if !self.insts[i].module.is_combinational() {
                self.publish(i);
            }
        }
        for k in 0..self.comb_order.len() {
            let i = self.comb_order[k];
            let ins = self.read_ins(i);
            let refs: Vec<(&str, Value)> = ins.iter().map(|(n, v)| (n.as_str(), v.clone())).collect();
            self.insts[i].module.evaluate(&refs)?;
            self.publish(i);
        }
        Ok(())
    }

    /// I.13.2(b), first half, for the domain of parent clock `clock`: the network
    /// settles on `(q_N, x_{N+1})` and every coroutine of the domain is resumed once,
    /// reading the settled wires. A resumption changes no wire (its writes are
    /// pending, I.6.1). Afterwards [`Design::observe`] is the pre-edge reading —
    /// observation 0 on the first cycle (I.9.4).
    pub fn pre_edge(&mut self, clock: &str) -> Result<(), RefError> {
        if !self.clocks.iter().any(|c| c == clock) {
            return Err(RefError::Runtime(format!("`{clock}` is not a clock of `{}`", self.name)));
        }
        self.settle()?;
        for i in 0..self.insts.len() {
            if self.insts[i].clock.as_deref() == Some(clock) {
                let ins = self.read_ins(i);
                let refs: Vec<(&str, Value)> = ins.iter().map(|(n, v)| (n.as_str(), v.clone())).collect();
                self.insts[i].module.resume(&refs)?;
            }
        }
        Ok(())
    }

    /// I.13.2(b), second half: the edge commits every pending value and advances
    /// every memory of the domain, then the network settles on `(q_{N+1}, x_{N+1})`.
    pub fn post_edge(&mut self, clock: &str) -> Result<(), RefError> {
        for i in 0..self.insts.len() {
            if self.insts[i].clock.as_deref() == Some(clock) {
                self.insts[i].module.edge();
            }
        }
        self.settle()
    }

    /// One cycle of the domain of `clock`: [`Design::pre_edge`] then [`Design::post_edge`].
    pub fn tick(&mut self, clock: &str) -> Result<(), RefError> {
        self.pre_edge(clock)?;
        self.post_edge(clock)
    }

    /// The parent's outputs, in signature order: each wire's settled value.
    pub fn observe(&self) -> Vec<(String, Value)> {
        self.outputs.iter().map(|(n, w)| (n.clone(), self.wires[*w].value.clone())).collect()
    }

    /// Every wire's current value, by name (nets and parent ports).
    pub fn wire_values(&self) -> Vec<(String, Value)> {
        self.wires.iter().map(|w| (w.name.clone(), w.value.clone())).collect()
    }

    /// Whether wire `name` is driven by a `RegOut` of a coroutine (a source of the
    /// settle) rather than a function or the environment.
    pub fn is_registered(&self, name: &str) -> Option<bool> {
        let w = self.wires.iter().find(|w| w.name == name)?;
        Some(match &w.driver {
            Some(Driver::Inst(i, _)) => !self.insts[*i].module.is_combinational(),
            _ => false,
        })
    }
}

/// A net declaration's payload type: `net::<T, D>()` (D-18 A3), or today's
/// `wire::<T, D>(init)` / `registered_wire::<T, D>(&clk, init)` — whose `init` is a
/// III.2 precondition (the zero of `T`) and no value of the net (I.13.4(c)).
fn net_type(e: &Expr, ev: &dyn ConstEval) -> Option<Result<Ty, RefError>> {
    let Expr::Call(c) = e else { return None };
    let Expr::Path(p) = &*c.func else { return None };
    let seg = p.path.segments.last()?;
    if !matches!(seg.ident.to_string().as_str(), "net" | "wire" | "registered_wire") {
        return None;
    }
    let syn::PathArguments::AngleBracketed(a) = &seg.arguments else {
        return Some(Err(unsup(format!("`{}` without `::<T, D>`", seg.ident))));
    };
    let Some(syn::GenericArgument::Type(t)) = a.args.first() else {
        return Some(Err(unsup(format!("`{}` without a payload type", seg.ident))));
    };
    Some(parse_ty(t, ev))
}

/// A clock argument: `clk` or `clk.clone()`.
fn clock_arg(e: &Expr) -> Option<String> {
    match e {
        Expr::Path(p) if p.path.segments.len() == 1 => Some(p.path.segments[0].ident.to_string()),
        Expr::MethodCall(mc) if mc.method == "clone" && mc.args.is_empty() => clock_arg(&mc.receiver),
        _ => None,
    }
}

/// A wire argument: a parent port `p`, a net end `n.0` / `n.1`.
fn wire_arg(e: &Expr) -> Option<(String, Option<u32>)> {
    match e {
        Expr::Path(p) if p.path.segments.len() == 1 => Some((p.path.segments[0].ident.to_string(), None)),
        Expr::Field(f) => match (&*f.base, &f.member) {
            (Expr::Path(p), syn::Member::Unnamed(ix)) if p.path.segments.len() == 1 => {
                Some((p.path.segments[0].ident.to_string(), Some(ix.index)))
            }
            _ => None,
        },
        Expr::MethodCall(mc) if mc.method == "clone" && mc.args.is_empty() => wire_arg(&mc.receiver),
        _ => None,
    }
}
