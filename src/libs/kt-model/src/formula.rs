//! Mini-formula: parsing and evaluation (R.61).
//!
//! **Owner's decision (R.59, verbatim):** “Expressions are their own mini-formula,
//! evaluated live on every frame; there are no external interpreters.” Reconnaissance
//! (F-AUT-007) confirmed it from three sides: the switch of someone else's engine in After
//! Effects changed the result of finished projects, Blender showed what a document with
//! executable code costs, and Nuke showed that the price of a feature is set not by the
//! language but by the presence of a single property passport.
//!
//! **The set of operations is CLOSED.** No loops, no definitions, no access to the
//! file system, clock or randomness from the environment: the same text with the same
//! inputs must give the same bytes always and everywhere
//! (invariant 11). Hence also the ban on `rand` and `now`.
//!
//! **The error is NAMED.** An unknown name, division by zero, an unclosed
//! bracket — each with its own text and position in the string. A silent zero
//! would show a person a picture they would believe.

use kt_plugin_api::math::FrozenMath;
use std::collections::BTreeMap;

/// What went wrong — with a position in the source string.
#[derive(Debug, Clone, PartialEq)]
pub struct FormulaError {
    pub at: usize,
    pub kind: FormulaErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormulaErrorKind {
    /// A symbol that does not exist in the language.
    BadChar(char),
    /// A bracket is not closed.
    Unclosed,
    /// An extra closing bracket or junk at the tail.
    Trailing,
    /// A number, a name or “(” was expected.
    ExpectedValue,
    /// A name that is neither among the variables nor among the functions. Here also
    /// fall `now` and `rand`: they are DELIBERATELY absent from the language.
    UnknownName(String),
    /// The function needs a different number of arguments.
    Arity { name: String, want: usize, got: usize },
    /// Division by zero is an ERROR, not infinity: a silent infinity would travel
    /// into the frame and become a black screen.
    DivZero,
    /// The result is not finite (overflow, `0/0`, a root of a negative).
    NotFinite,
}

impl std::fmt::Display for FormulaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use FormulaErrorKind::*;
        let what = match &self.kind {
            BadChar(c) => format!("the language has no character {c:?}"),
            Unclosed => "a bracket is left open".into(),
            Trailing => "there is leftover text after the expression".into(),
            ExpectedValue => "a number, a name or '(' was expected".into(),
            UnknownName(n) => format!("no variable or function is named {n:?}"),
            Arity { name, want, got } => {
                format!("{name} takes {want} argument(s), got {got}")
            }
            DivZero => "division by zero".into(),
            NotFinite => "the result is not a finite number".into(),
        };
        write!(f, "formula, char {}: {what}", self.at)
    }
}

impl std::error::Error for FormulaError {}

/// A parsed expression. It is parsed ONCE (when the renderer is built) and
/// evaluated on every frame: text is not parsed in the hot path.
#[derive(Debug, Clone, PartialEq)]
pub struct Formula {
    root: Node,
    /// Names that the expression reads. They are needed by whoever builds the
    /// dependency graph: asking the tree on every frame is expensive.
    pub reads: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Num(f64),
    Var(String),
    Neg(Box<Node>),
    Bin(Op2, Box<Node>, Box<Node>),
    Call(Fun, Vec<Node>),
    /// `if(cond, a, b)` — branching WITHOUT statements: this is an expression.
    If(Box<Node>, Box<Node>, Box<Node>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op2 {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fun {
    Min,
    Max,
    Clamp,
    Abs,
    Floor,
    Ceil,
    Round,
    Sqrt,
    Sin,
    Cos,
    Lerp,
    /// Deterministic noise from an INTEGER seed — not from the environment.
    Noise,
}

/// The evaluator's function registry — name and number of arguments (R.67): the help in
/// the curve block is built FROM HERE, not from a copy in the interface.
pub const FUNCTIONS: &[(&str, usize)] = &[
    ("min", 2),
    ("max", 2),
    ("clamp", 3),
    ("abs", 1),
    ("floor", 1),
    ("ceil", 1),
    ("round", 1),
    ("sqrt", 1),
    ("sin", 1),
    ("cos", 1),
    ("lerp", 3),
    ("noise", 1),
    ("if", 3),
];

impl Fun {
    fn arity(self) -> usize {
        match self {
            Fun::Abs
            | Fun::Floor
            | Fun::Ceil
            | Fun::Round
            | Fun::Sqrt
            | Fun::Sin
            | Fun::Cos
            | Fun::Noise => 1,
            Fun::Min | Fun::Max => 2,
            Fun::Clamp | Fun::Lerp => 3,
        }
    }
    fn by_name(n: &str) -> Option<Fun> {
        Some(match n {
            "min" => Fun::Min,
            "max" => Fun::Max,
            "clamp" => Fun::Clamp,
            "abs" => Fun::Abs,
            "floor" => Fun::Floor,
            "ceil" => Fun::Ceil,
            "round" => Fun::Round,
            "sqrt" => Fun::Sqrt,
            "sin" => Fun::Sin,
            "cos" => Fun::Cos,
            "lerp" => Fun::Lerp,
            "noise" => Fun::Noise,
            _ => return None,
        })
    }
}

impl Formula {
    /// Parse the text. The error carries a position — the input field will highlight it.
    pub fn parse(src: &str) -> Result<Formula, FormulaError> {
        kt_time::cost::add(kt_time::cost::Cost::FormulaParse, 1);
        let mut p = Parser { s: src.as_bytes(), i: 0, reads: Vec::new() };
        p.ws();
        let root = p.expr()?;
        p.ws();
        if p.i < p.s.len() {
            return Err(FormulaError { at: p.i, kind: FormulaErrorKind::Trailing });
        }
        let mut reads = p.reads;
        reads.sort();
        reads.dedup();
        Ok(Formula { root, reads })
    }

    /// Evaluate with the given variables.
    pub fn eval(&self, vars: &BTreeMap<String, f64>) -> Result<f64, FormulaError> {
        let v = eval(&self.root, vars)?;
        if v.is_finite() {
            Ok(v)
        } else {
            Err(FormulaError { at: 0, kind: FormulaErrorKind::NotFinite })
        }
    }
}

// ---- compiled form (compiled-curves, JIT tier T1) ------------------------------------------------

/// One compiled node: reads the numbered variable slots and returns what [`eval`] returns for the
/// same node, error included.
type Op = Box<dyn Fn(&[f64]) -> Result<f64, FormulaError> + Send + Sync>;

/// A formula COMPILED once into closures over numbered variable slots (compiled-curves). The
/// interpreter walks the boxed tree, looks every name up in a `BTreeMap<String, f64>` the caller
/// allocated, and builds a `Vec` per function call — per sampled point of a speed curve, per frame of
/// a link, per pixel of a graph. Here the names are resolved to slot indices once, and an evaluation
/// is the closures alone: no allocation, no string compare, no lock.
///
/// **The same bits as [`Formula::eval`]** (invariant 11): every node performs the same IEEE
/// operation on the same operands in the same order (Rust neither contracts `a·b+c` into an FMA nor
/// reassociates), the transcendental functions are the same `kt_plugin_api::math` ones, a binary
/// operator and a call evaluate their operands left to right and return the FIRST error, `if`
/// evaluates only the branch it takes, and a name absent from the slot list is the interpreter's
/// `UnknownName` at evaluation, not at compilation. Guard: t2100 (both paths on every operator,
/// function, error and random trees).
pub struct Compiled {
    root: Op,
}

impl std::fmt::Debug for Compiled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Compiled")
    }
}

impl Compiled {
    /// Evaluate with the slot values in the order of the names given to [`Formula::compile`].
    #[inline]
    pub fn eval(&self, slots: &[f64]) -> Result<f64, FormulaError> {
        let v = (self.root)(slots)?;
        if v.is_finite() {
            Ok(v)
        } else {
            Err(FormulaError { at: 0, kind: FormulaErrorKind::NotFinite })
        }
    }
}

impl Formula {
    /// Compile with the variables named by `vars`: slot `i` of [`Compiled::eval`] is `vars[i]`.
    pub fn compile(&self, vars: &[&str]) -> Compiled {
        Compiled { root: compile(&self.root, vars) }
    }
}

fn compile(n: &Node, vars: &[&str]) -> Op {
    match n {
        Node::Num(v) => {
            let v = *v;
            Box::new(move |_| Ok(v))
        }
        Node::Var(name) => match vars.iter().position(|v| v == name) {
            Some(i) => Box::new(move |s| Ok(s[i])),
            None => {
                let name = name.clone();
                Box::new(move |_| {
                    Err(FormulaError { at: 0, kind: FormulaErrorKind::UnknownName(name.clone()) })
                })
            }
        },
        Node::Neg(a) => {
            let a = compile(a, vars);
            Box::new(move |s| Ok(-a(s)?))
        }
        Node::Bin(op, a, b) => {
            let (a, b, op) = (compile(a, vars), compile(b, vars), *op);
            Box::new(move |s| {
                let (x, y) = (a(s)?, b(s)?);
                bin(op, x, y)
            })
        }
        Node::If(c, a, b) => {
            let (c, a, b) = (compile(c, vars), compile(a, vars), compile(b, vars));
            Box::new(move |s| if c(s)? != 0.0 { a(s) } else { b(s) })
        }
        Node::Call(f, args) => {
            let f = *f;
            let mut ops: Vec<Op> = args.iter().map(|x| compile(x, vars)).collect();
            // the arity is checked by the parser; the slots are filled left to right as `collect`
            match ops.len() {
                1 => {
                    let a = ops.pop().expect("one argument");
                    Box::new(move |s| Ok(call(f, &[a(s)?])))
                }
                2 => {
                    let b = ops.pop().expect("two arguments");
                    let a = ops.pop().expect("two arguments");
                    Box::new(move |s| {
                        let x = a(s)?;
                        let y = b(s)?;
                        Ok(call(f, &[x, y]))
                    })
                }
                _ => {
                    let c = ops.pop().expect("three arguments");
                    let b = ops.pop().expect("three arguments");
                    let a = ops.pop().expect("three arguments");
                    Box::new(move |s| {
                        let x = a(s)?;
                        let y = b(s)?;
                        let z = c(s)?;
                        Ok(call(f, &[x, y, z]))
                    })
                }
            }
        }
    }
}

/// A binary operator — ONE body for the interpreter and the compiled form.
#[inline]
fn bin(op: Op2, x: f64, y: f64) -> Result<f64, FormulaError> {
    Ok(match op {
        Op2::Add => x + y,
        Op2::Sub => x - y,
        Op2::Mul => x * y,
        Op2::Div | Op2::Mod if y == 0.0 => {
            return Err(FormulaError { at: 0, kind: FormulaErrorKind::DivZero });
        }
        Op2::Div => x / y,
        Op2::Mod => x.rem_euclid(y),
        Op2::Lt => f64::from(u8::from(x < y)),
        Op2::Gt => f64::from(u8::from(x > y)),
        Op2::Le => f64::from(u8::from(x <= y)),
        Op2::Ge => f64::from(u8::from(x >= y)),
        Op2::Eq => f64::from(u8::from(x == y)),
        Op2::Ne => f64::from(u8::from(x != y)),
    })
}

/// A function on its evaluated arguments — ONE body for the interpreter and the compiled form.
#[inline]
fn call(f: Fun, a: &[f64]) -> f64 {
    match f {
        Fun::Min => a[0].min(a[1]),
        Fun::Max => a[0].max(a[1]),
        Fun::Clamp => a[0].clamp(a[1].min(a[2]), a[2].max(a[1])),
        Fun::Abs => a[0].abs(),
        Fun::Floor => a[0].floor(),
        Fun::Ceil => a[0].ceil(),
        Fun::Round => a[0].round(),
        Fun::Sqrt => a[0].sqrt(),
        Fun::Sin => a[0].fsin(),
        Fun::Cos => a[0].fcos(),
        Fun::Lerp => a[0] + (a[1] - a[0]) * a[2],
        // DETERMINISTIC noise: an integer seed into a hash, nothing from the
        // environment. The same algorithm as the counter-based noise of the
        // generator — there is no second “randomness” in the program.
        Fun::Noise => {
            let mut h = (a[0].floor() as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            h ^= h >> 29;
            h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
            h ^= h >> 32;
            (h >> 11) as f64 / ((1u64 << 53) as f64) * 2.0 - 1.0
        }
    }
}

fn eval(n: &Node, vars: &BTreeMap<String, f64>) -> Result<f64, FormulaError> {
    Ok(match n {
        Node::Num(v) => *v,
        Node::Var(name) => *vars.get(name).ok_or_else(|| FormulaError {
            at: 0,
            kind: FormulaErrorKind::UnknownName(name.clone()),
        })?,
        Node::Neg(a) => -eval(a, vars)?,
        Node::Bin(op, a, b) => {
            let (x, y) = (eval(a, vars)?, eval(b, vars)?);
            bin(*op, x, y)?
        }
        Node::If(c, a, b) => {
            if eval(c, vars)? != 0.0 {
                eval(a, vars)?
            } else {
                eval(b, vars)?
            }
        }
        Node::Call(f, args) => {
            let a: Result<Vec<f64>, _> = args.iter().map(|x| eval(x, vars)).collect();
            call(*f, &a?)
        }
    })
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    reads: Vec<String>,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && (self.s[self.i] as char).is_whitespace() {
            self.i += 1;
        }
    }
    fn eat(&mut self, lit: &str) -> bool {
        if self.s[self.i..].starts_with(lit.as_bytes()) {
            self.i += lit.len();
            self.ws();
            true
        } else {
            false
        }
    }
    /// Comparisons are the weakest level: `a < b` gives 0 or 1.
    fn expr(&mut self) -> Result<Node, FormulaError> {
        let mut a = self.sum()?;
        loop {
            let op = if self.eat("<=") {
                Op2::Le
            } else if self.eat(">=") {
                Op2::Ge
            } else if self.eat("==") {
                Op2::Eq
            } else if self.eat("!=") {
                Op2::Ne
            } else if self.eat("<") {
                Op2::Lt
            } else if self.eat(">") {
                Op2::Gt
            } else {
                return Ok(a);
            };
            a = Node::Bin(op, Box::new(a), Box::new(self.sum()?));
        }
    }
    fn sum(&mut self) -> Result<Node, FormulaError> {
        let mut a = self.prod()?;
        loop {
            let op = if self.eat("+") {
                Op2::Add
            } else if self.eat("-") {
                Op2::Sub
            } else {
                return Ok(a);
            };
            a = Node::Bin(op, Box::new(a), Box::new(self.prod()?));
        }
    }
    fn prod(&mut self) -> Result<Node, FormulaError> {
        let mut a = self.unary()?;
        loop {
            let op = if self.eat("*") {
                Op2::Mul
            } else if self.eat("/") {
                Op2::Div
            } else if self.eat("%") {
                Op2::Mod
            } else {
                return Ok(a);
            };
            a = Node::Bin(op, Box::new(a), Box::new(self.unary()?));
        }
    }
    fn unary(&mut self) -> Result<Node, FormulaError> {
        if self.eat("-") {
            return Ok(Node::Neg(Box::new(self.unary()?)));
        }
        self.atom()
    }
    fn atom(&mut self) -> Result<Node, FormulaError> {
        self.ws();
        if self.i >= self.s.len() {
            return Err(FormulaError { at: self.i, kind: FormulaErrorKind::ExpectedValue });
        }
        let at = self.i;
        let c = self.s[self.i] as char;
        if c == '(' {
            self.i += 1;
            self.ws();
            let e = self.expr()?;
            self.ws();
            if !self.eat(")") {
                return Err(FormulaError { at: self.i, kind: FormulaErrorKind::Unclosed });
            }
            return Ok(e);
        }
        if c.is_ascii_digit() || c == '.' {
            let st = self.i;
            while self.i < self.s.len()
                && ((self.s[self.i] as char).is_ascii_digit() || self.s[self.i] == b'.')
            {
                self.i += 1;
            }
            let t = std::str::from_utf8(&self.s[st..self.i]).unwrap_or("");
            self.ws();
            return t
                .parse::<f64>()
                .map(Node::Num)
                .map_err(|_| FormulaError { at: st, kind: FormulaErrorKind::ExpectedValue });
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let st = self.i;
            while self.i < self.s.len()
                && ((self.s[self.i] as char).is_ascii_alphanumeric()
                    || self.s[self.i] == b'_'
                    || self.s[self.i] == b'.')
            {
                self.i += 1;
            }
            let name = std::str::from_utf8(&self.s[st..self.i]).unwrap_or("").to_string();
            self.ws();
            if self.eat("(") {
                let mut args = Vec::new();
                self.ws();
                if !self.eat(")") {
                    loop {
                        args.push(self.expr()?);
                        self.ws();
                        if self.eat(",") {
                            continue;
                        }
                        if self.eat(")") {
                            break;
                        }
                        return Err(FormulaError { at: self.i, kind: FormulaErrorKind::Unclosed });
                    }
                }
                if name == "if" {
                    if args.len() != 3 {
                        return Err(FormulaError {
                            at: st,
                            kind: FormulaErrorKind::Arity { name, want: 3, got: args.len() },
                        });
                    }
                    let mut it = args.into_iter();
                    let (c, a, b) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
                    return Ok(Node::If(Box::new(c), Box::new(a), Box::new(b)));
                }
                let f = Fun::by_name(&name).ok_or_else(|| FormulaError {
                    at: st,
                    kind: FormulaErrorKind::UnknownName(name.clone()),
                })?;
                if args.len() != f.arity() {
                    return Err(FormulaError {
                        at: st,
                        kind: FormulaErrorKind::Arity { name, want: f.arity(), got: args.len() },
                    });
                }
                return Ok(Node::Call(f, args));
            }
            self.reads.push(name.clone());
            return Ok(Node::Var(name));
        }
        Err(FormulaError { at, kind: FormulaErrorKind::BadChar(c) })
    }
}
