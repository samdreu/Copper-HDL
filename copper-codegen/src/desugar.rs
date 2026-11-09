//! FIR→FIR desugarings of shapes SPEC I.2.4 admits and the lowering below has no form for
//! (lines `II.3 found body-const`, `assoc-const-body` and `compound-assign` of SPEC §III.3.4;
//! `design_docs/review/MEASUREMENTS.md` §Z.3). Run first, before any pass looks at the body.
//!
//! * **A body-local `const` item** (`const K: u8 = 7;` anywhere in the module body) is a
//!   constant (I.2.4 DESUGARED, I.14.4): it is hoisted into the file-scope const machinery —
//!   `file_consts` (a `localparam`) and `file_const_source` (what `ConstEnv` reads, so a
//!   ticking or tick-free `for` bound and R5 see it). Rust makes an item visible in its whole
//!   block; hoisted, it is visible in the whole module, which is the same program unless the
//!   name is taken. A name already taken (a file const, another body const) is renamed
//!   `<K>__body<k>` in the block that declares it.
//! * **An associated const** `T::K` of an inherent `impl` (I.14.4) named in a body expression
//!   or a bound is hoisted the same way, as `T__K`, by the value `ConstEnv` folds it to.
//! * **A compound assignment** `v op= e` is `v = v op e` (I.2.4 DESUGARED); `v` is a place
//!   expression with no side effect (a local, a field, a bit), so naming it twice is the same.

use std::collections::{HashMap, HashSet};

use copper_core::frontend_ir::{
    ExprAssign, ExprBinary, ExprPath, ExprType, FrontendModuleIR, ItemConst, ItemStmt, RawStmt,
    RawStmtKind, RawTypeRef,
};

pub fn desugar(fir: &mut FrontendModuleIR) {
    let mut body = std::mem::take(&mut fir.raw_statements);
    // Range patterns first: their endpoints become expressions the const hoists then see.
    visit_stmts(&mut body, &mut |e| {
        compound_assign(e);
        range_patterns(e);
    });
    hoist_body_consts(&mut body, fir);
    hoist_assoc_consts(&mut body, fir);
    hoist_body_structs(&body, fir);
    // Last: a `let` in an inner scope that shadows an outer binding gets a name of its own
    // (MEASUREMENTS §AC), so no later pass maps the two bindings onto one SV signal.
    rename_scoped_shadows(&mut body);
    fir.raw_statements = body;
}

// ── range patterns ────────────────────────────────────────────────────────────

/// `lo..=hi => b` (or `lo..hi`, `lo..`, `..=hi`, `n @ lo..=hi`, an or-list holding one) becomes
/// a guarded arm, `_ if s >= lo && s <= hi => b` (SPEC I.2.4 IN; line `II.3 found
/// range-pattern`, MEASUREMENTS §Z.5): the same arm selection, since a range pattern binds
/// nothing and a guard is tested in arm order exactly as a pattern is. An element of a tuple
/// pattern over a tuple scrutinee is replaced by `_` and tested against that element. An existing
/// guard is conjoined after the range test.
fn range_patterns(e: &mut ExprType) {
    let ExprType::Match(m) = e else { return };
    let scrut = (*m.scrutinee).clone();
    for arm in &mut m.arms {
        let text = arm.pattern_text.trim().to_string();
        if !text.contains("..") {
            continue;
        }
        let span = arm.span;
        let (new_pat, cond) = if text.starts_with('(') && text.ends_with(')') {
            let ExprType::Tuple(t) = &scrut else { continue };
            let elems = split_top(&text[1..text.len() - 1], ',');
            if elems.len() != t.elements.len() {
                continue;
            }
            let mut pats = Vec::new();
            let mut conds = Vec::new();
            let mut ok = true;
            for (p, s) in elems.iter().zip(&t.elements) {
                if p.contains("..") {
                    match alternatives_test(p, s, span) {
                        Some((binder, c)) => {
                            pats.push(binder.unwrap_or_else(|| "_".to_string()));
                            conds.push(c);
                        }
                        None => {
                            ok = false;
                            break;
                        }
                    }
                } else {
                    pats.push(p.trim().to_string());
                }
            }
            if !ok || conds.is_empty() {
                continue;
            }
            // No binder left: every element is a test, and the arm a guarded `_` (a guarded
            // tuple pattern has no `case` form downstream).
            let binds = |p: &str| p.chars().next().is_some_and(|c| c.is_lowercase() || c == '_') && p != "_";
            if !pats.iter().any(|p| binds(p.trim())) {
                for (p, s) in pats.iter().zip(&t.elements) {
                    let p = p.trim();
                    if p != "_" {
                        match expr_of(p, span) {
                            Some(v) => conds.push(bin(s.clone(), "==", v, span)),
                            None => {
                                ok = false;
                                break;
                            }
                        }
                    }
                }
                if !ok {
                    continue;
                }
                let cond = conds.into_iter().reduce(|a, b| bin(a, "&&", b, span)).expect("non-empty");
                ("_".to_string(), cond)
            } else {
                let cond = conds.into_iter().reduce(|a, b| bin(a, "&&", b, span)).expect("non-empty");
                (format!("({})", pats.join(", ")), cond)
            }
        } else {
            match alternatives_test(&text, &scrut, span) {
                Some((binder, c)) => (binder.unwrap_or_else(|| "_".to_string()), c),
                None => continue,
            }
        };
        let guard = match arm.guard.take() {
            Some(g) => bin(cond, "&&", paren(*g, span), span),
            None => cond,
        };
        arm.pattern_text = new_pat;
        arm.guard = Some(Box::new(guard));
    }
}

/// The test an or-list of scalar patterns (`1 | 4..=6`, `n @ 0..=3`) makes of `s`, and the
/// binder of an `n @ …` form. `None` for a shape this desugaring does not take (a binder in an
/// or-list, a pattern it cannot spell as a comparison).
fn alternatives_test(text: &str, s: &ExprType, span: copper_core::frontend_ir::SourceSpan) -> Option<(Option<String>, ExprType)> {
    let mut text = text.trim();
    let mut binder = None;
    if let Some((b, rest)) = text.split_once('@') {
        let b = b.trim();
        if !b.chars().all(|c| c.is_alphanumeric() || c == '_') || b.is_empty() {
            return None;
        }
        binder = Some(b.to_string());
        text = rest.trim();
    }
    let mut tests = Vec::new();
    for alt in split_top(text, '|') {
        let alt = alt.trim();
        if let Some((lo, hi, inclusive)) = split_range(alt) {
            let mut parts = Vec::new();
            if !lo.is_empty() {
                parts.push(bin(s.clone(), ">=", expr_of(lo, span)?, span));
            }
            if !hi.is_empty() {
                parts.push(bin(s.clone(), if inclusive { "<=" } else { "<" }, expr_of(hi, span)?, span));
            }
            tests.push(paren(parts.into_iter().reduce(|a, b| bin(a, "&&", b, span))?, span));
        } else if alt == "_" {
            return None; // `_ | 0..3` is `_`: leave it to the lowering's own refusal
        } else {
            tests.push(bin(s.clone(), "==", expr_of(alt, span)?, span));
        }
    }
    let test = tests.into_iter().reduce(|a, b| bin(a, "||", b, span))?;
    Some((binder, paren(test, span)))
}

/// `lo..=hi` / `lo..hi` / `lo..` / `..=hi` → (lo, hi, inclusive); `None` if not a range.
fn split_range(t: &str) -> Option<(&str, &str, bool)> {
    if let Some((lo, hi)) = t.split_once("..=") {
        return Some((lo.trim(), hi.trim(), true));
    }
    t.split_once("..").map(|(lo, hi)| (lo.trim(), hi.trim(), false))
}

/// A pattern endpoint (`3`, `0x1F`, `-4`, `K`, `T::K`) as an expression.
fn expr_of(t: &str, span: copper_core::frontend_ir::SourceSpan) -> Option<ExprType> {
    let e = syn::parse_str::<syn::Expr>(t).ok()?;
    let _ = span;
    Some(crate::parser::parse_expr_type(&e, &HashSet::new()))
}

fn bin(l: ExprType, op: &str, r: ExprType, span: copper_core::frontend_ir::SourceSpan) -> ExprType {
    ExprType::Binary(ExprBinary { left: Box::new(l), op: op.to_string(), right: Box::new(r), span })
}

fn paren(e: ExprType, span: copper_core::frontend_ir::SourceSpan) -> ExprType {
    let _ = span;
    e
}

/// Split `s` on top-level `sep` (outside brackets).
fn split_top(s: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if c == sep && depth == 0 {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out
}

// ── body-local struct items ───────────────────────────────────────────────────

/// A `struct` item declared in the body (I.2.4: DESUGARED, one local per field) is a file
/// struct for the lowering (§Z.8); the name is the module's own, so it is added only when no
/// file struct has it.
fn hoist_body_structs(body: &[RawStmt], fir: &mut FrontendModuleIR) {
    fn walk(stmts: &[RawStmt], out: &mut Vec<copper_core::frontend_ir::ItemStruct>) {
        for s in stmts {
            match &s.kind {
                RawStmtKind::Item(ItemStmt::Struct(st)) => out.push(st.clone()),
                RawStmtKind::Expr(es) => walk_e(&es.expr, out),
                RawStmtKind::Local(l) => {
                    if let Some(i) = &l.init {
                        walk_e(i, out)
                    }
                }
                RawStmtKind::Item(_) => {}
            }
        }
    }
    fn walk_e(e: &ExprType, out: &mut Vec<copper_core::frontend_ir::ItemStruct>) {
        match e {
            ExprType::Block(b) => walk(&b.stmts, out),
            ExprType::Loop(l) => walk(&l.body, out),
            ExprType::While(w) => walk(&w.body, out),
            ExprType::ForLoop(f) => walk(&f.body, out),
            ExprType::If(i) => {
                walk(&i.then_block, out);
                if let Some(e) = &i.else_branch {
                    walk_e(e, out)
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    walk(body, &mut found);
    for st in found {
        if !fir.file_structs.iter().any(|f| f.name == st.name) {
            fir.file_structs.push(st);
        }
    }
}

// ── compound assignment ───────────────────────────────────────────────────────

const COMPOUND: &[(&str, &str)] = &[
    ("+=", "+"),
    ("-=", "-"),
    ("*=", "*"),
    ("/=", "/"),
    ("%=", "%"),
    ("^=", "^"),
    ("&=", "&"),
    ("|=", "|"),
    ("<<=", "<<"),
    (">>=", ">>"),
];

fn compound_assign(e: &mut ExprType) {
    let ExprType::Binary(b) = e else { return };
    let op: String = b.op.chars().filter(|c| !c.is_whitespace()).collect();
    let Some((_, plain)) = COMPOUND.iter().find(|(c, _)| *c == op) else { return };
    let span = b.span;
    let value = ExprType::Binary(ExprBinary {
        left: b.left.clone(),
        op: plain.to_string(),
        right: b.right.clone(),
        span,
    });
    *e = ExprType::Assign(ExprAssign { left: b.left.clone(), right: Box::new(value), span });
}

// ── body-local consts ─────────────────────────────────────────────────────────

fn hoist_body_consts(body: &mut Vec<RawStmt>, fir: &mut FrontendModuleIR) {
    let mut taken: HashSet<String> = fir.file_consts.iter().map(|c| c.name.clone()).collect();
    let mut hoisted: Vec<ItemConst> = Vec::new();
    hoist_in_list(body, &mut taken, &mut hoisted);
    for c in hoisted {
        fir.file_const_source.push(format!("const {}: {} = {};", c.name, c.ty.ty_text, c.value_text));
        fir.file_consts.push(c);
    }
}

/// Hoist the const items of one statement list (then of every list nested in it).
fn hoist_in_list(stmts: &mut Vec<RawStmt>, taken: &mut HashSet<String>, hoisted: &mut Vec<ItemConst>) {
    let mut renames: HashMap<String, String> = HashMap::new();
    let start = hoisted.len();
    let mut kept = Vec::with_capacity(stmts.len());
    for s in std::mem::take(stmts) {
        if let RawStmtKind::Item(ItemStmt::Const(c)) = &s.kind {
            let mut c = c.clone();
            if taken.contains(&c.name) {
                let mut k = 0;
                let fresh = loop {
                    let n = format!("{}__body{k}", c.name);
                    if !taken.contains(&n) {
                        break n;
                    }
                    k += 1;
                };
                renames.insert(c.name.clone(), fresh.clone());
                c.name = fresh;
            }
            taken.insert(c.name.clone());
            hoisted.push(c);
        } else {
            kept.push(s);
        }
    }
    *stmts = kept;
    if !renames.is_empty() {
        // The block's own consts may name each other: rename in their initializers too.
        for c in hoisted[start..].iter_mut() {
            for (from, to) in &renames {
                c.value_text = rename_ident_in_text(&c.value_text, from, to);
            }
        }
        visit_stmts(stmts, &mut |e| {
            if let ExprType::Path(p) = e {
                let id: String = p.path_text.chars().filter(|c| !c.is_whitespace()).collect();
                if let Some(to) = renames.get(&id) {
                    p.path_text = to.clone();
                }
            }
        });
    }
    for s in stmts.iter_mut() {
        for_each_nested_list(s, &mut |l| hoist_in_list(l, taken, hoisted));
    }
}

/// `text` (a token string) with every whole-word `from` replaced by `to`.
fn rename_ident_in_text(text: &str, from: &str, to: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let flush = |w: &mut String, out: &mut String| {
        out.push_str(if w == from { to } else { w.as_str() });
        w.clear();
    };
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            word.push(ch);
        } else {
            flush(&mut word, &mut out);
            out.push(ch);
        }
    }
    flush(&mut word, &mut out);
    out
}

// ── associated consts ─────────────────────────────────────────────────────────

fn hoist_assoc_consts(body: &mut [RawStmt], fir: &mut FrontendModuleIR) {
    // `T::K` → its declared type, from the inherent impls `ConstEnv` reads.
    let mut declared: HashMap<String, String> = HashMap::new();
    for text in &fir.file_const_source {
        let Ok(syn::Item::Impl(imp)) = syn::parse_str::<syn::Item>(text) else { continue };
        if imp.trait_.is_some() {
            continue;
        }
        let syn::Type::Path(tp) = &*imp.self_ty else { continue };
        let Some(seg) = tp.path.segments.last() else { continue };
        for it in &imp.items {
            if let syn::ImplItem::Const(c) = it {
                let ty = &c.ty;
                declared.insert(format!("{}::{}", seg.ident, c.ident), quote::quote!(#ty).to_string());
            }
        }
    }
    if declared.is_empty() {
        return;
    }
    let env = copper_analysis::consts::ConstEnv::from_items(fir.file_const_source.iter().map(String::as_str));
    let mut taken: HashSet<String> = fir.file_consts.iter().map(|c| c.name.clone()).collect();
    let mut hoisted: HashMap<String, ItemConst> = HashMap::new();
    visit_stmts(body, &mut |e| {
        let ExprType::Path(p) = e else { return };
        let key: String = p.path_text.chars().filter(|c| !c.is_whitespace()).collect();
        let (Some(ty), Some(v)) = (declared.get(&key), env.get(&key)) else { return };
        let name = match hoisted.get(&key) {
            Some(c) => c.name.clone(),
            None => {
                let base = key.replace("::", "__");
                let mut name = base.clone();
                let mut k = 0;
                while taken.contains(&name) {
                    name = format!("{base}{k}");
                    k += 1;
                }
                taken.insert(name.clone());
                hoisted.insert(
                    key.clone(),
                    ItemConst {
                        name: name.clone(),
                        ty: RawTypeRef { ty_text: ty.clone(), span: p.span },
                        value_text: v.to_string(),
                        attrs: Vec::new(),
                        span: p.span,
                    },
                );
                name
            }
        };
        *e = ExprType::Path(ExprPath { path_text: name, span: p.span });
    });
    // A pattern naming `T::K` (`T::K => …`) names the hoisted const.
    visit_stmts(body, &mut |e| {
        let ExprType::Match(m) = e else { return };
        for arm in &mut m.arms {
            let alts = split_top(&arm.pattern_text, '|');
            let mut changed = false;
            let new: Vec<String> = alts
                .iter()
                .map(|a| {
                    let key: String = a.chars().filter(|c| !c.is_whitespace()).collect();
                    match (declared.get(&key), env.get(&key)) {
                        (Some(ty), Some(v)) => {
                            changed = true;
                            let name = match hoisted.get(&key) {
                                Some(c) => c.name.clone(),
                                None => {
                                    let base = key.replace("::", "__");
                                    let mut name = base.clone();
                                    let mut k = 0;
                                    while taken.contains(&name) {
                                        name = format!("{base}{k}");
                                        k += 1;
                                    }
                                    taken.insert(name.clone());
                                    hoisted.insert(key.clone(), ItemConst {
                                        name: name.clone(),
                                        ty: RawTypeRef { ty_text: ty.clone(), span: arm.span },
                                        value_text: v.to_string(),
                                        attrs: Vec::new(),
                                        span: arm.span,
                                    });
                                    name
                                }
                            };
                            format!(" {name} ")
                        }
                        _ => a.clone(),
                    }
                })
                .collect();
            if changed {
                arm.pattern_text = new.join("|").trim().to_string();
            }
        }
    });
    let mut hoisted: Vec<ItemConst> = hoisted.into_values().collect();
    hoisted.sort_by(|a, b| a.name.cmp(&b.name));
    for c in hoisted {
        fir.file_const_source.push(format!("const {}: {} = {};", c.name, c.ty.ty_text, c.value_text));
        fir.file_consts.push(c);
    }
}

// ── a mutable post-order visitor over every expression of a body ─────────────

pub(crate) fn visit_stmts(stmts: &mut [RawStmt], f: &mut dyn FnMut(&mut ExprType)) {
    for s in stmts {
        match &mut s.kind {
            RawStmtKind::Local(l) => {
                if let Some(i) = l.init.as_mut() {
                    visit_expr(i, f);
                }
            }
            RawStmtKind::Expr(es) => visit_expr(&mut es.expr, f),
            RawStmtKind::Item(_) => {}
        }
    }
}

fn visit_expr(e: &mut ExprType, f: &mut dyn FnMut(&mut ExprType)) {
    match e {
        ExprType::Array(a) => a.elements.iter_mut().for_each(|x| visit_expr(x, f)),
        ExprType::Assign(a) => {
            visit_expr(&mut a.left, f);
            visit_expr(&mut a.right, f);
        }
        ExprType::Await(a) => visit_expr(&mut a.base, f),
        ExprType::Binary(b) => {
            visit_expr(&mut b.left, f);
            visit_expr(&mut b.right, f);
        }
        ExprType::Block(b) => visit_stmts(&mut b.stmts, f),
        ExprType::Call(c) => {
            visit_expr(&mut c.func, f);
            c.args.iter_mut().for_each(|x| visit_expr(x, f));
        }
        ExprType::Cast(c) => visit_expr(&mut c.expr, f),
        ExprType::Field(fl) => visit_expr(&mut fl.base, f),
        ExprType::Index(i) => {
            visit_expr(&mut i.base, f);
            visit_expr(&mut i.index, f);
        }
        ExprType::If(i) => {
            visit_expr(&mut i.condition, f);
            visit_stmts(&mut i.then_block, f);
            if let Some(eb) = i.else_branch.as_mut() {
                visit_expr(eb, f);
            }
        }
        ExprType::Loop(l) => visit_stmts(&mut l.body, f),
        ExprType::Match(m) => {
            visit_expr(&mut m.scrutinee, f);
            for arm in &mut m.arms {
                if let Some(g) = arm.guard.as_mut() {
                    visit_expr(g, f);
                }
                visit_expr(&mut arm.body, f);
            }
        }
        ExprType::MethodCall(mc) => {
            visit_expr(&mut mc.receiver, f);
            mc.args.iter_mut().for_each(|x| visit_expr(x, f));
        }
        ExprType::Range(r) => {
            if let Some(s) = r.start.as_mut() {
                visit_expr(s, f);
            }
            if let Some(en) = r.end.as_mut() {
                visit_expr(en, f);
            }
        }
        ExprType::Reference(r) => visit_expr(&mut r.expr, f),
        ExprType::Repeat(r) => {
            visit_expr(&mut r.expr, f);
            visit_expr(&mut r.len, f);
        }
        ExprType::Return(r) => {
            if let Some(v) = r.value.as_mut() {
                visit_expr(v, f);
            }
        }
        ExprType::Struct(s) => {
            s.fields.iter_mut().for_each(|x| visit_expr(&mut x.expr, f));
            if let Some(r) = s.rest.as_mut() {
                visit_expr(r, f);
            }
        }
        ExprType::Tuple(t) => t.elements.iter_mut().for_each(|x| visit_expr(x, f)),
        ExprType::Unary(u) => visit_expr(&mut u.expr, f),
        ExprType::While(w) => {
            visit_expr(&mut w.condition, f);
            visit_stmts(&mut w.body, f);
        }
        ExprType::ForLoop(fl) => {
            visit_expr(&mut fl.iter, f);
            visit_stmts(&mut fl.body, f);
        }
        ExprType::Try(t) => visit_expr(&mut t.expr, f),
        _ => {}
    }
    f(e);
}

/// Call `f` on every statement list directly nested in `s` (a block, a branch, a loop body).
fn for_each_nested_list(s: &mut RawStmt, f: &mut dyn FnMut(&mut Vec<RawStmt>)) {
    fn in_expr(e: &mut ExprType, f: &mut dyn FnMut(&mut Vec<RawStmt>)) {
        match e {
            ExprType::Block(b) => f(&mut b.stmts),
            ExprType::If(i) => {
                in_expr(&mut i.condition, f);
                f(&mut i.then_block);
                if let Some(eb) = i.else_branch.as_mut() {
                    in_expr(eb, f);
                }
            }
            ExprType::Loop(l) => f(&mut l.body),
            ExprType::While(w) => f(&mut w.body),
            ExprType::ForLoop(fl) => f(&mut fl.body),
            ExprType::Match(m) => m.arms.iter_mut().for_each(|a| in_expr(&mut a.body, f)),
            ExprType::Assign(a) => in_expr(&mut a.right, f),
            ExprType::Await(a) => in_expr(&mut a.base, f),
            _ => {}
        }
    }
    match &mut s.kind {
        RawStmtKind::Local(l) => {
            if let Some(i) = l.init.as_mut() {
                in_expr(i, f);
            }
        }
        RawStmtKind::Expr(es) => in_expr(&mut es.expr, f),
        RawStmtKind::Item(_) => {}
    }
}

// ── scoped shadows ────────────────────────────────────────────────────────────

/// The suffix of a renamed shadow (`x__shadow`, `x__shadow1`, …; see [`rename_scoped_shadows`]).
pub(crate) const SHADOW_SUFFIX: &str = "__shadow";

/// The source local a renamed shadow stands for: `x__shadow` and `x__shadow3` are `x`.
pub fn shadow_source(name: &str) -> Option<&str> {
    let i = name.rfind(SHADOW_SUFFIX)?;
    let rest = &name[i + SHADOW_SUFFIX.len()..];
    (i > 0 && rest.chars().all(|c| c.is_ascii_digit())).then(|| &name[..i])
}

/// Give every binding that shadows a binding of an OUTER scope a name of its own, for its
/// scope only (Rust's block scoping; SPEC I.14, line `II.3 found for-var-shadow-nested`,
/// MEASUREMENTS §AC). The lowering maps every binding of one name onto one SV signal, which is
/// right for a re-`let` in the same block — the later binding hides the earlier one for the
/// rest of that block, as a reassignment does — and wrong for a shadow in an inner block, which
/// ends at the block's end: after it the outer binding is visible again, unchanged. Until
/// 2026-11-09 a shadow in an `if` branch, a `match` arm or a bare block overwrote the outer
/// local; a body `let` shadowing a `for`'s variable overwrote the loop variable (tick-free: the
/// SV `for`'s own variable, lint-rejected) or the tick-bearing `for`'s counter (silently: the
/// trip count changed); one shadowing a register was a duplicate SV declaration.
///
/// The SHADOW is renamed (`<x>__shadow`, then `<x>__shadow1`, … for a name already taken),
/// never the binding it shadows: a `for`'s variable keeps its name, so the tick-bearing `for`'s
/// counter stays the source local the register inference names (§AB.5). Its initializer is
/// read before the binding takes effect (`let x = x + 1` reads the outer `x`). Binders this pass
/// does not rename — a `match` arm's, an `if let`'s, a closure's — still hide an outer binding
/// of their name in their scope, so a read there is never redirected to the outer one's name.
pub(crate) fn rename_scoped_shadows(body: &mut [RawStmt]) {
    let mut taken: HashSet<String> = HashSet::new();
    collect_names(body, &mut taken);
    let mut sc = Scopes { stack: vec![HashMap::new()], taken };
    shadow_list(body, &mut sc);
}

struct Scopes {
    /// Source name → the name it is lowered under, innermost scope last.
    stack: Vec<HashMap<String, String>>,
    taken: HashSet<String>,
}

impl Scopes {
    fn lookup(&self, n: &str) -> Option<(usize, &String)> {
        self.stack.iter().enumerate().rev().find_map(|(d, m)| m.get(n).map(|v| (d, v)))
    }

    /// Bind `n` in the innermost scope; the name it is lowered under.
    fn bind(&mut self, n: &str) -> String {
        if n == "_" || n.is_empty() {
            return n.to_string();
        }
        let top = self.stack.len() - 1;
        let name = match self.lookup(n) {
            Some((d, cur)) if d == top => cur.clone(),
            Some(_) => {
                let mut k = 0usize;
                loop {
                    let c = if k == 0 { format!("{n}{SHADOW_SUFFIX}") } else { format!("{n}{SHADOW_SUFFIX}{k}") };
                    if !self.taken.contains(&c) {
                        self.taken.insert(c.clone());
                        break c;
                    }
                    k += 1;
                }
            }
            None => n.to_string(),
        };
        self.stack[top].insert(n.to_string(), name.clone());
        name
    }

    /// Bind `n` to itself in the innermost scope: a binder this pass does not rename.
    fn mask(&mut self, n: &str) {
        let top = self.stack.len() - 1;
        self.stack[top].insert(n.to_string(), n.to_string());
    }

    fn renamed(&self, n: &str) -> Option<String> {
        self.lookup(n).filter(|(_, v)| v.as_str() != n).map(|(_, v)| v.clone())
    }

    fn scoped<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.stack.push(HashMap::new());
        let r = f(self);
        self.stack.pop();
        r
    }
}

fn is_ident(t: &str) -> bool {
    let mut cs = t.chars();
    cs.next().is_some_and(|c| c.is_alphabetic() || c == '_') && cs.all(|c| c.is_alphanumeric() || c == '_')
}

/// The binders of a `let` name as the parser keeps it: `x`, or `(a, b, _)`.
fn let_binders(name: &str) -> Option<Vec<String>> {
    let t = name.trim();
    if is_ident(t) {
        return Some(vec![t.to_string()]);
    }
    let inner = t.strip_prefix('(')?.strip_suffix(')')?;
    inner.split(',').map(|p| p.trim()).filter(|p| !p.is_empty()).map(|p| is_ident(p).then(|| p.to_string())).collect()
}

/// The lower-case identifiers of a pattern's text that may be binders (a superset: a
/// path segment, a field name, a keyword are left out; a lower-case const is not).
fn pattern_binders(text: &str) -> Vec<String> {
    let bytes: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_alphanumeric() || bytes[i] == '_') {
                i += 1;
            }
            let w: String = bytes[start..i].iter().collect();
            let before: String = bytes[..start].iter().rev().take_while(|c| !c.is_whitespace()).take(2).collect();
            let after: String = bytes[i..].iter().skip_while(|c| c.is_whitespace()).take(2).collect();
            let lower = w.chars().next().is_some_and(|c| c.is_lowercase() || c == '_');
            let keyword = matches!(w.as_str(), "_" | "ref" | "mut" | "true" | "false");
            let path = before.starts_with("::") || after.starts_with("::") || after.starts_with('(') || after.starts_with('{') || after.starts_with('!');
            if lower && !keyword && !path && !w.chars().all(|c| c.is_ascii_digit() || c == '_') {
                out.push(w);
            }
        } else if c.is_ascii_digit() {
            while i < bytes.len() && (bytes[i].is_alphanumeric() || bytes[i] == '_') {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    out
}

fn collect_names(stmts: &[RawStmt], out: &mut HashSet<String>) {
    let mut v = stmts.to_vec();
    visit_stmts(&mut v, &mut |e| match e {
        ExprType::Path(p) => {
            out.insert(p.path_text.chars().filter(|c| !c.is_whitespace()).collect());
        }
        ExprType::ForLoop(f) => {
            out.insert(f.pat_text.trim().to_string());
        }
        _ => {}
    });
    fn lets(stmts: &[RawStmt], out: &mut HashSet<String>) {
        for s in stmts {
            if let RawStmtKind::Local(l) = &s.kind {
                out.extend(let_binders(&l.name).unwrap_or_default());
            }
        }
    }
    lets(stmts, out);
    for s in &mut v {
        let mut lists = Vec::new();
        for_each_nested_list(s, &mut |l| lists.push(l.clone()));
        for l in lists {
            collect_names(&l, out);
        }
    }
}

fn shadow_list(stmts: &mut [RawStmt], sc: &mut Scopes) {
    for s in stmts.iter_mut() {
        match &mut s.kind {
            RawStmtKind::Local(l) => {
                if let Some(i) = l.init.as_mut() {
                    shadow_expr(i, sc);
                }
                match let_binders(&l.name) {
                    Some(bs) if bs.len() == 1 && is_ident(l.name.trim()) => {
                        l.name = sc.bind(&bs[0]);
                    }
                    Some(bs) => {
                        let names: Vec<String> = bs.iter().map(|b| sc.bind(b)).collect();
                        l.name = format!("({})", names.join(", "));
                    }
                    None => {}
                }
            }
            RawStmtKind::Expr(es) => shadow_expr(&mut es.expr, sc),
            RawStmtKind::Item(_) => {}
        }
    }
}

fn shadow_expr(e: &mut ExprType, sc: &mut Scopes) {
    match e {
        ExprType::Path(p) => {
            let id: String = p.path_text.chars().filter(|c| !c.is_whitespace()).collect();
            if let Some(to) = sc.renamed(&id) {
                p.path_text = to;
            }
        }
        ExprType::Macro(m) => {
            for (from, to) in sc.stack.iter().flat_map(|m| m.iter()).filter(|(f, t)| f != t).map(|(f, t)| (f.clone(), t.clone())).collect::<Vec<_>>() {
                if sc.renamed(&from).as_deref() == Some(to.as_str()) {
                    m.tokens_text = rename_ident_in_text(&m.tokens_text, &from, &to);
                }
            }
        }
        ExprType::Block(b) => sc.scoped(|sc| shadow_list(&mut b.stmts, sc)),
        ExprType::Async(a) => sc.scoped(|sc| shadow_list(&mut a.block, sc)),
        ExprType::Const(c) => sc.scoped(|sc| shadow_list(&mut c.stmts, sc)),
        ExprType::Loop(l) => sc.scoped(|sc| shadow_list(&mut l.body, sc)),
        ExprType::While(w) => {
            shadow_expr(&mut w.condition, sc);
            sc.scoped(|sc| shadow_list(&mut w.body, sc));
        }
        ExprType::If(i) => {
            shadow_expr(&mut i.condition, sc);
            let mut lets = Vec::new();
            if_let_binders(&i.condition, &mut lets);
            sc.scoped(|sc| {
                lets.iter().for_each(|b| sc.mask(b));
                shadow_list(&mut i.then_block, sc);
            });
            if let Some(eb) = i.else_branch.as_mut() {
                shadow_expr(eb, sc);
            }
        }
        ExprType::Let(l) => shadow_expr(&mut l.expr, sc),
        ExprType::ForLoop(f) => {
            shadow_expr(&mut f.iter, sc);
            sc.scoped(|sc| {
                let pat = f.pat_text.trim().to_string();
                if is_ident(&pat) {
                    f.pat_text = sc.bind(&pat);
                } else {
                    pattern_binders(&pat).iter().for_each(|b| sc.mask(b));
                }
                // The body is a block of its own, inside the pattern's scope: a body `let` of
                // the variable's name shadows it (`II.3 found for-var-shadow`).
                sc.scoped(|sc| shadow_list(&mut f.body, sc));
            });
        }
        ExprType::Match(m) => {
            shadow_expr(&mut m.scrutinee, sc);
            for arm in &mut m.arms {
                sc.scoped(|sc| {
                    pattern_binders(&arm.pattern_text).iter().for_each(|b| sc.mask(b));
                    if let Some(g) = arm.guard.as_mut() {
                        shadow_expr(g, sc);
                    }
                    shadow_expr(&mut arm.body, sc);
                });
            }
        }
        ExprType::Closure(c) => sc.scoped(|sc| {
            c.params.iter().flat_map(|p| pattern_binders(p)).for_each(|b| sc.mask(&b));
            shadow_expr(&mut c.body, sc);
        }),
        ExprType::Array(a) => a.elements.iter_mut().for_each(|x| shadow_expr(x, sc)),
        ExprType::Assign(a) => {
            shadow_expr(&mut a.left, sc);
            shadow_expr(&mut a.right, sc);
        }
        ExprType::Await(a) => shadow_expr(&mut a.base, sc),
        ExprType::Binary(b) => {
            shadow_expr(&mut b.left, sc);
            shadow_expr(&mut b.right, sc);
        }
        ExprType::Call(c) => {
            shadow_expr(&mut c.func, sc);
            c.args.iter_mut().for_each(|x| shadow_expr(x, sc));
        }
        ExprType::Cast(c) => shadow_expr(&mut c.expr, sc),
        ExprType::Field(fl) => shadow_expr(&mut fl.base, sc),
        ExprType::Index(i) => {
            shadow_expr(&mut i.base, sc);
            shadow_expr(&mut i.index, sc);
        }
        ExprType::MethodCall(mc) => {
            shadow_expr(&mut mc.receiver, sc);
            mc.args.iter_mut().for_each(|x| shadow_expr(x, sc));
        }
        ExprType::Range(r) => {
            if let Some(x) = r.start.as_mut() {
                shadow_expr(x, sc);
            }
            if let Some(x) = r.end.as_mut() {
                shadow_expr(x, sc);
            }
        }
        ExprType::Reference(r) => shadow_expr(&mut r.expr, sc),
        ExprType::Repeat(r) => {
            shadow_expr(&mut r.expr, sc);
            shadow_expr(&mut r.len, sc);
        }
        ExprType::Return(r) => {
            if let Some(v) = r.value.as_mut() {
                shadow_expr(v, sc);
            }
        }
        ExprType::Break(b) => {
            if let Some(v) = b.expr.as_mut() {
                shadow_expr(v, sc);
            }
        }
        ExprType::Yield(y) => {
            if let Some(v) = y.value.as_mut() {
                shadow_expr(v, sc);
            }
        }
        ExprType::Struct(st) => {
            st.fields.iter_mut().for_each(|x| shadow_expr(&mut x.expr, sc));
            if let Some(r) = st.rest.as_mut() {
                shadow_expr(r, sc);
            }
        }
        ExprType::Tuple(t) => t.elements.iter_mut().for_each(|x| shadow_expr(x, sc)),
        ExprType::Unary(u) => shadow_expr(&mut u.expr, sc),
        ExprType::Try(t) => shadow_expr(&mut t.expr, sc),
        ExprType::Lit(_) | ExprType::Continue(_) => {}
    }
}

/// The binders of the `let` patterns in an `if` condition (`if let p = e`, `&&`-chained).
fn if_let_binders(cond: &ExprType, out: &mut Vec<String>) {
    match cond {
        ExprType::Let(l) => out.extend(pattern_binders(&l.pattern_text)),
        ExprType::Binary(b) if b.op.trim() == "&&" => {
            if_let_binders(&b.left, out);
            if_let_binders(&b.right, out);
        }
        _ => {}
    }
}

/// The renamed shadows (see [`rename_scoped_shadows`]) whose value is live at a tick: a
/// statement of the shadow's scope — the rest of its block, after its `let` — holds a tick, and
/// that statement or a later one reads the shadow. A shadow's scope is entered afresh on every
/// pass through its block, so no back edge carries it; this is liveness at a tick (SPEC I.7.1)
/// for the shadow alone, which the register inference — by name, over the source, where the
/// shadow and the binding it shadows share one — cannot give. Over-approximate only where the
/// tick-bearing statement reads the shadow before its tick (a flop holding what a wire would
/// pass on).
pub(crate) fn shadows_live_at_tick(stmts: &[RawStmt]) -> HashSet<String> {
    let mut out = HashSet::new();
    live_in_list(stmts, &mut out);
    out
}

fn live_in_list(stmts: &[RawStmt], out: &mut HashSet<String>) {
    for (k, s) in stmts.iter().enumerate() {
        if let RawStmtKind::Local(l) = &s.kind {
            for b in let_binders(&l.name).unwrap_or_default() {
                if shadow_source(&b).is_none() {
                    continue;
                }
                let rest = &stmts[k + 1..];
                if let Some(t) = rest.iter().position(crate::control_extract::stmt_contains_tick) {
                    if crate::control_extract::stmts_mention(&rest[t..], &b) {
                        out.insert(b);
                    }
                }
            }
        }
        let mut s = s.clone();
        for_each_nested_list(&mut s, &mut |l| live_in_list(l, out));
        // A renamed `for` variable: its scope is the body.
        if let RawStmtKind::Expr(es) = &s.kind {
            for_pats_live(&es.expr, out);
        }
    }
}

fn for_pats_live(e: &ExprType, out: &mut HashSet<String>) {
    if let ExprType::ForLoop(f) = e {
        let p = f.pat_text.trim();
        // A tick-bearing `for`'s variable is its counter: a register whether or not the body
        // reads it.
        if shadow_source(p).is_some() && f.body.iter().any(crate::control_extract::stmt_contains_tick) {
            out.insert(p.to_string());
        }
    }
}
