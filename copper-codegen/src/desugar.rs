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
