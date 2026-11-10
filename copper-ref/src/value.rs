//! The interpreter's run-time values and the small type grammar it reads off
//! annotations and port types (SPEC §I.14, as far as the corpus needs it).

use crate::bits::RefBits;
use crate::RefError;
use copper_core::Logic;
use std::fmt;

/// A Rust integer type the corpus uses for counters, `pc` locals and indices.
///
/// A value is held in an `i128` (`Value::Int`): the number itself for every type but
/// `u128`, whose value is the `i128` with the same 128-bit pattern (`v as u128`) —
/// so every reading of a `u128` as a number (order, arithmetic, a clamp) goes through
/// [`IntTy::cmp`], [`IntTy::to_u128`] or the interpreter's `u128` arms (SPEC III.3.4
/// `II.3 found ref-no-i128`, MEASUREMENTS §AG; until then `u128` stopped at `i128::MAX`
/// and there was no `i128`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IntTy {
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    /// An unsuffixed literal, or an arithmetic result of two: it takes the type of
    /// whatever it meets (rustc's inference, approximated by first contact).
    Unsuffixed,
}

impl IntTy {
    pub fn parse(s: &str) -> Option<IntTy> {
        Some(match s {
            "u8" => IntTy::U8,
            "u16" => IntTy::U16,
            "u32" => IntTy::U32,
            "u64" => IntTy::U64,
            "u128" => IntTy::U128,
            "usize" => IntTy::Usize,
            "i8" => IntTy::I8,
            "i16" => IntTy::I16,
            "i32" => IntTy::I32,
            "i64" => IntTy::I64,
            "i128" => IntTy::I128,
            "isize" => IntTy::Isize,
            _ => return None,
        })
    }

    /// The width of the type in bits (`usize`/`isize` 64, I.14.1(b)); `None` for an
    /// unsuffixed literal.
    pub fn width(self) -> Option<usize> {
        Some(match self {
            IntTy::U8 | IntTy::I8 => 8,
            IntTy::U16 | IntTy::I16 => 16,
            IntTy::U32 | IntTy::I32 => 32,
            IntTy::U64 | IntTy::I64 | IntTy::Usize | IntTy::Isize => 64,
            IntTy::U128 | IntTy::I128 => 128,
            IntTy::Unsuffixed => return None,
        })
    }

    pub fn is_signed(self) -> bool {
        matches!(self, IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64 | IntTy::I128 | IntTy::Isize)
    }

    /// The order of two values of this type: a `u128` compares its patterns unsigned.
    pub fn cmp(self, a: i128, b: i128) -> std::cmp::Ordering {
        if self == IntTy::U128 { (a as u128).cmp(&(b as u128)) } else { a.cmp(&b) }
    }

    /// The value as a `u128` when it is a non-negative number (a `u128`'s pattern, or a
    /// non-negative value of any other type); `None` for a negative value.
    pub fn to_u128(self, v: i128) -> Option<u128> {
        if self == IntTy::U128 { Some(v as u128) } else { u128::try_from(v).ok() }
    }

    /// The inclusive range of the type as the `i128`s that hold its extremes (a `u128`'s
    /// maximum is the pattern `-1`; order them with [`IntTy::cmp`]). `Unsuffixed` is
    /// unbounded here.
    pub fn range(self) -> (i128, i128) {
        match self {
            IntTy::U8 => (0, u8::MAX as i128),
            IntTy::U16 => (0, u16::MAX as i128),
            IntTy::U32 => (0, u32::MAX as i128),
            IntTy::U64 | IntTy::Usize => (0, u64::MAX as i128),
            IntTy::U128 => (0, -1),
            IntTy::I128 => (i128::MIN, i128::MAX),
            IntTy::I8 => (i8::MIN as i128, i8::MAX as i128),
            IntTy::I16 => (i16::MIN as i128, i16::MAX as i128),
            IntTy::I32 => (i32::MIN as i128, i32::MAX as i128),
            IntTy::I64 | IntTy::Isize => (i64::MIN as i128, i64::MAX as i128),
            IntTy::Unsuffixed => (i128::MIN, i128::MAX),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            IntTy::U8 => "u8",
            IntTy::U16 => "u16",
            IntTy::U32 => "u32",
            IntTy::U64 => "u64",
            IntTy::U128 => "u128",
            IntTy::Usize => "usize",
            IntTy::I8 => "i8",
            IntTy::I16 => "i16",
            IntTy::I32 => "i32",
            IntTy::I64 => "i64",
            IntTy::I128 => "i128",
            IntTy::Isize => "isize",
            IntTy::Unsuffixed => "{integer}",
        }
    }
}

/// A type, as far as the interpreter needs one: to size a `Bits` literal, to zero a
/// port, to flatten a port value to bits.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Ty {
    Logic,
    Bits(u32),
    Bool,
    Int(IntTy),
    Array(Box<Ty>, usize),
    Tuple(Vec<Ty>),
    /// An enum or struct by name.
    Named(String),
}

impl Ty {
    /// Total bit width of a port payload (`Logic`, `Bits<N>`, arrays of those).
    pub fn bit_width(&self) -> Option<usize> {
        match self {
            Ty::Logic => Some(1),
            Ty::Bits(w) => Some(*w as usize),
            // Integer and `bool` port payloads (I.14.1; Phase C step 8): the
            // integer's width, `usize` 64 bits.
            Ty::Bool => Some(1),
            Ty::Int(t) => t.width(),
            Ty::Array(t, n) => t.bit_width().map(|w| w * n),
            _ => None,
        }
    }
}

/// A run-time value.
#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    /// `Logic` or `Bits<N>` (a `Logic` is width 1; `Logic` and `Bits<1>` are not
    /// distinguished, which no program in the kernel can observe).
    Bits(RefBits),
    /// A `Bits` literal whose width rustc infers from context and this interpreter
    /// has not met yet: `Bits::zero()`, `Bits::from_u8(3)`, `Bits::from_lit::<1>()`.
    /// Sized at first contact with a sized value (`coerce`).
    Lit(u128),
    /// `Bits::x()` of a width not yet known.
    LitX,
    /// `Bits::all_ones()` of a width not yet known (rustc infers it; I.14.4, Phase C
    /// step 8 — the interpreter refused this spelling until then).
    LitOnes,
    Bool(bool),
    Int(i128, IntTy),
    /// A unit enum variant, `(enum name, variant)`.
    Enum(String, String),
    Tuple(Vec<Value>),
    Array(Vec<Value>),
    Struct(String, Vec<(String, Value)>),
    /// The poison run's marker (SPEC §I.0.5(a), §I.7.7): a local the register
    /// inference did not name, overwritten at a tick. Reading it is the finding.
    Poison(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bits(b) => write!(f, "{b:?}"),
            Value::Lit(v) => write!(f, "{v} (unsized Bits literal)"),
            Value::LitX => write!(f, "x (unsized)"),
            Value::LitOnes => write!(f, "all ones (unsized)"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(v, IntTy::U128) => write!(f, "{}u128", *v as u128),
            Value::Int(v, t) => write!(f, "{v}{}", t.name()),
            Value::Enum(e, v) => write!(f, "{e}::{v}"),
            Value::Tuple(vs) => {
                write!(f, "(")?;
                for (i, v) in vs.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{v}")?;
                }
                write!(f, ")")
            }
            Value::Array(vs) => {
                write!(f, "[")?;
                for (i, v) in vs.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Value::Struct(n, fs) => {
                write!(f, "{n} {{ ")?;
                for (i, (k, v)) in fs.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, " }}")
            }
            Value::Poison(n) => write!(f, "<poison of `{n}`>"),
        }
    }
}

impl Value {
    pub fn logic(l: Logic) -> Value {
        Value::Bits(RefBits::from_logic(l))
    }

    /// The zero of a type — what a port powers on at (SPEC §I.6.6) and what a
    /// declared-but-unsized local starts as.
    pub fn zero_of(ty: &Ty) -> Result<Value, RefError> {
        Ok(match ty {
            Ty::Logic => Value::logic(Logic::Zero),
            Ty::Bits(w) => Value::Bits(RefBits::zero(*w)),
            Ty::Bool => Value::Bool(false),
            Ty::Int(t) => Value::Int(0, *t),
            Ty::Array(t, n) => Value::Array(vec![Value::zero_of(t)?; *n]),
            Ty::Tuple(ts) => Value::Tuple(ts.iter().map(Value::zero_of).collect::<Result<_, _>>()?),
            Ty::Named(n) => {
                return Err(RefError::Runtime(format!("no zero for the named type `{n}`")));
            }
        })
    }

    /// Flatten a port value to bits, LSB first, element-major for arrays — the
    /// harness's `RandStim::as_bits` layout.
    pub fn to_bits(&self) -> Result<Vec<Logic>, RefError> {
        match self {
            Value::Bits(b) => Ok(b.to_logic_vec()),
            Value::Bool(b) => Ok(vec![if *b { Logic::One } else { Logic::Zero }]),
            Value::Int(i, t) => {
                let w = t.width().ok_or_else(|| RefError::Runtime("an unsized integer at a port".into()))?;
                Ok((0..w).map(|k| if (i >> k) & 1 == 1 { Logic::One } else { Logic::Zero }).collect())
            }
            Value::Array(vs) => {
                let mut out = Vec::new();
                for v in vs {
                    out.extend(v.to_bits()?);
                }
                Ok(out)
            }
            other => Err(RefError::Runtime(format!("not a port value: {other}"))),
        }
    }

    /// The inverse of `to_bits` for a declared port type.
    pub fn from_bits(ty: &Ty, bits: &[Logic]) -> Result<Value, RefError> {
        match ty {
            Ty::Logic | Ty::Bits(_) => {
                let w = ty.bit_width().unwrap();
                if bits.len() != w {
                    return Err(RefError::Runtime(format!(
                        "port of width {w} given {} bits",
                        bits.len()
                    )));
                }
                Ok(Value::Bits(RefBits::from_logic_slice(bits)))
            }
            // A `bool` / integer port has no X (I.14.2(a)): an X driven onto one is
            // outside the environment's values (the harness drives X-free inputs).
            Ty::Bool | Ty::Int(_) => {
                let w = ty.bit_width().unwrap();
                if bits.len() != w {
                    return Err(RefError::Runtime(format!("port of width {w} given {} bits", bits.len())));
                }
                let mut v: i128 = 0;
                for (k, b) in bits.iter().enumerate() {
                    match b {
                        Logic::One => v |= 1i128 << k,
                        Logic::Zero => {}
                        Logic::X => return Err(RefError::Runtime("an X bit on a `bool`/integer port".into())),
                    }
                }
                Ok(match ty {
                    Ty::Bool => Value::Bool(v == 1),
                    Ty::Int(t) => {
                        if t.is_signed() && w < 128 && (v >> (w - 1)) & 1 == 1 {
                            v -= 1i128 << w;
                        }
                        Value::Int(v, *t)
                    }
                    _ => unreachable!(),
                })
            }
            Ty::Array(t, n) => {
                let ew = t.bit_width().ok_or_else(|| RefError::Runtime("array port of a non-bit element".into()))?;
                if bits.len() != ew * n {
                    return Err(RefError::Runtime(format!(
                        "array port of {n}×{ew} bits given {} bits",
                        bits.len()
                    )));
                }
                let mut vs = Vec::with_capacity(*n);
                for i in 0..*n {
                    vs.push(Value::from_bits(t, &bits[i * ew..(i + 1) * ew])?);
                }
                Ok(Value::Array(vs))
            }
            other => Err(RefError::Runtime(format!("not a port type: {other:?}"))),
        }
    }

    /// Give a value the type `ty` where the type is informative: an unsized
    /// literal takes a `Bits` width; an unsuffixed integer takes its integer type.
    /// Anything already sized is checked, not changed.
    pub fn coerce_to(self, ty: &Ty) -> Result<Value, RefError> {
        match (self, ty) {
            (Value::Lit(v), Ty::Bits(w)) => {
                if *w < 128 && v >> w != 0 {
                    return Err(RefError::Runtime(format!(
                        "constant {v} does not fit in Bits<{w}>"
                    )));
                }
                Ok(Value::Bits(RefBits::from_uint(*w, v)))
            }
            (Value::Lit(v), Ty::Logic) => {
                if v > 1 {
                    return Err(RefError::Runtime(format!("constant {v} is not a Logic")));
                }
                Ok(Value::Bits(RefBits::from_uint(1, v)))
            }
            (Value::LitX, Ty::Bits(w)) => Ok(Value::Bits(RefBits::all_x(*w))),
            (Value::LitX, Ty::Logic) => Ok(Value::Bits(RefBits::all_x(1))),
            (Value::LitOnes, Ty::Bits(w)) => Ok(Value::Bits(RefBits::all_ones(*w))),
            (Value::LitOnes, Ty::Logic) => Ok(Value::Bits(RefBits::all_ones(1))),
            (Value::Int(v, IntTy::Unsuffixed), Ty::Int(t)) => {
                // An unsuffixed literal holds its number, never a `u128` pattern.
                let (lo, hi) = t.range();
                let fits = if *t == IntTy::U128 { v >= 0 } else { v >= lo && v <= hi };
                if !fits {
                    return Err(RefError::Runtime(format!("literal {v} out of range for {}", t.name())));
                }
                Ok(Value::Int(v, *t))
            }
            (Value::Array(vs), Ty::Array(t, n)) => {
                if vs.len() != *n {
                    return Err(RefError::Runtime(format!(
                        "array of {} elements where [_; {n}] was declared",
                        vs.len()
                    )));
                }
                Ok(Value::Array(vs.into_iter().map(|v| v.coerce_to(t)).collect::<Result<_, _>>()?))
            }
            (Value::Tuple(vs), Ty::Tuple(ts)) if vs.len() == ts.len() => Ok(Value::Tuple(
                vs.into_iter().zip(ts).map(|(v, t)| v.coerce_to(t)).collect::<Result<_, _>>()?,
            )),
            (Value::Bits(b), Ty::Bits(w)) if b.width != *w => Err(RefError::Runtime(format!(
                "a Bits<{}> value where Bits<{w}> was declared",
                b.width
            ))),
            (Value::Bits(b), Ty::Logic) if b.width != 1 => Err(RefError::Runtime(format!(
                "a Bits<{}> value where Logic was declared",
                b.width
            ))),
            (v, _) => Ok(v),
        }
    }

    /// Give an unsized literal the width of `other` (first contact), leaving
    /// everything else alone. Used by every binary operator and assignment.
    pub fn size_like(self, other: &Value) -> Value {
        match (self, other) {
            (Value::Lit(v), Value::Bits(b)) => Value::Bits(RefBits::from_uint(b.width, v)),
            (Value::LitX, Value::Bits(b)) => Value::Bits(RefBits::all_x(b.width)),
            (Value::LitOnes, Value::Bits(b)) => Value::Bits(RefBits::all_ones(b.width)),
            (Value::Int(v, IntTy::Unsuffixed), Value::Int(_, t)) => Value::Int(v, *t),
            (s, _) => s,
        }
    }

    /// Reading a poisoned local is the under-inference the poison run looks for.
    pub fn check_poison(&self) -> Result<(), RefError> {
        match self {
            Value::Poison(n) => Err(RefError::Poisoned(n.clone())),
            Value::Tuple(vs) | Value::Array(vs) => vs.iter().try_for_each(Value::check_poison),
            Value::Struct(_, fs) => fs.iter().try_for_each(|(_, v)| v.check_poison()),
            _ => Ok(()),
        }
    }

    pub fn as_bits(&self) -> Result<RefBits, RefError> {
        match self {
            Value::Bits(b) => Ok(*b),
            Value::Poison(n) => Err(RefError::Poisoned(n.clone())),
            Value::Lit(v) => Err(RefError::Runtime(format!(
                "the width of the Bits literal {v} was never fixed by its context"
            ))),
            Value::LitX => Err(RefError::Runtime("the width of a Bits::x() was never fixed by its context".into())),
            Value::LitOnes => Err(RefError::Runtime("the width of a Bits::all_ones() was never fixed by its context".into())),
            other => Err(RefError::Runtime(format!("expected a Bits/Logic value, found {other}"))),
        }
    }

    pub fn as_logic(&self) -> Result<Logic, RefError> {
        let b = self.as_bits()?;
        if b.width != 1 {
            return Err(RefError::Runtime(format!("expected a Logic, found {b:?}")));
        }
        Ok(b.as_logic())
    }

    pub fn as_bool(&self) -> Result<bool, RefError> {
        match self {
            Value::Bool(b) => Ok(*b),
            Value::Poison(n) => Err(RefError::Poisoned(n.clone())),
            other => Err(RefError::Runtime(format!("expected a bool, found {other}"))),
        }
    }

    pub fn as_int(&self) -> Result<i128, RefError> {
        match self {
            Value::Int(v, _) => Ok(*v),
            Value::Poison(n) => Err(RefError::Poisoned(n.clone())),
            other => Err(RefError::Runtime(format!("expected an integer, found {other}"))),
        }
    }

    pub fn as_index(&self) -> Result<usize, RefError> {
        let v = self.as_int()?;
        usize::try_from(v).map_err(|_| RefError::Runtime(format!("negative index {v}")))
    }
}
