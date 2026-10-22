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

use std::collections::BTreeMap;

thread_local! {
    /// Parse cache by text (R.67): the CURVE formula is computed from
    /// `curve_value` in the interface on every pixel of the graph, and parsing
    /// the text on every probe would be an allocation in the draw path. Formula rendering
    /// does not compute them at all — the resolver expands them before the audio stream.
    static PARSED: std::cell::RefCell<std::collections::HashMap<String, std::rc::Rc<Formula>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// A parsed formula from the stream's cache; invalid text — `None` (the cache
/// stores only successes, the error is named by `Formula::parse` where the text is
/// entered).
pub fn cached(expr: &str) -> Option<std::rc::Rc<Formula>> {
    PARSED.with(|c| {
        if let Some(f) = c.borrow().get(expr) {
            return Some(f.clone());
        }
        let f = std::rc::Rc::new(Formula::parse(expr).ok()?);
        let mut m = c.borrow_mut();
        if m.len() > 256 {
            m.clear();
        }
        m.insert(expr.to_string(), f.clone());
        Some(f)
    })
}

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
    Bin(Op, Box<Node>, Box<Node>),
    Call(Fun, Vec<Node>),
    /// `if(cond, a, b)` — branching WITHOUT statements: this is an expression.
    If(Box<Node>, Box<Node>, Box<Node>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
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
            match op {
                Op::Add => x + y,
                Op::Sub => x - y,
                Op::Mul => x * y,
                Op::Div | Op::Mod if y == 0.0 => {
                    return Err(FormulaError { at: 0, kind: FormulaErrorKind::DivZero });
                }
                Op::Div => x / y,
                Op::Mod => x.rem_euclid(y),
                Op::Lt => f64::from(u8::from(x < y)),
                Op::Gt => f64::from(u8::from(x > y)),
                Op::Le => f64::from(u8::from(x <= y)),
                Op::Ge => f64::from(u8::from(x >= y)),
                Op::Eq => f64::from(u8::from(x == y)),
                Op::Ne => f64::from(u8::from(x != y)),
            }
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
            let a = a?;
            match f {
                Fun::Min => a[0].min(a[1]),
                Fun::Max => a[0].max(a[1]),
                Fun::Clamp => a[0].clamp(a[1].min(a[2]), a[2].max(a[1])),
                Fun::Abs => a[0].abs(),
                Fun::Floor => a[0].floor(),
                Fun::Ceil => a[0].ceil(),
                Fun::Round => a[0].round(),
                Fun::Sqrt => a[0].sqrt(),
                Fun::Sin => a[0].sin(),
                Fun::Cos => a[0].cos(),
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
                Op::Le
            } else if self.eat(">=") {
                Op::Ge
            } else if self.eat("==") {
                Op::Eq
            } else if self.eat("!=") {
                Op::Ne
            } else if self.eat("<") {
                Op::Lt
            } else if self.eat(">") {
                Op::Gt
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
                Op::Add
            } else if self.eat("-") {
                Op::Sub
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
                Op::Mul
            } else if self.eat("/") {
                Op::Div
            } else if self.eat("%") {
                Op::Mod
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
