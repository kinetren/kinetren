//! Мини-формула: разбор и вычисление (Р.61).
//!
//! **Решение владельца (Р.59, дословно):** «Выражения — своя мини-формула,
//! считается вживую на каждом кадре; внешних интерпретаторов нет.» Разведка
//! (F-AUT-007) подтвердила его с трёх сторон: смена чужого движка у After
//! Effects поменяла результат готовых проектов, Blender показал, чем платит
//! документ с исполняемым кодом, а Nuke — что цена фичи определяется не
//! языком, а наличием единого паспорта свойства.
//!
//! **Набор операций ЗАКРЫТ.** Ни циклов, ни определений, ни обращения к
//! файловой системе, часам и случайности из окружения: одинаковый текст при
//! одинаковых входах обязан давать одинаковые байты всегда и везде
//! (инвариант 11). Отсюда же запрет на `rand` и `now`.
//!
//! **Ошибка НАЗЫВАЕТСЯ.** Неизвестное имя, деление на ноль, незакрытая
//! скобка — каждая со своим текстом и позицией в строке. Молчаливый ноль
//! показал бы человеку картинку, которой он поверит.

use std::collections::BTreeMap;

thread_local! {
    /// Кэш разбора по тексту (Р.67): формула ОТРЕЗКА считается из
    /// `curve_value` в интерфейсе на каждый пиксель графика, и разбирать
    /// текст на каждую пробу было бы аллокацией в draw-пути. Рендер формул
    /// не считает вовсе — их разворачивает резолвер до аудио-потока.
    static PARSED: std::cell::RefCell<std::collections::HashMap<String, std::rc::Rc<Formula>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Разобранная формула из кэша потока; невалидный текст — `None` (кэш
/// хранит только удачи, ошибку называет `Formula::parse` там, где текст
/// вводят).
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

/// Что пошло не так — с позицией в исходной строке.
#[derive(Debug, Clone, PartialEq)]
pub struct FormulaError {
    pub at: usize,
    pub kind: FormulaErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormulaErrorKind {
    /// Символ, которого в языке нет.
    BadChar(char),
    /// Скобка не закрыта.
    Unclosed,
    /// Лишняя закрывающая скобка или мусор в хвосте.
    Trailing,
    /// Ожидалось число, имя или «(».
    ExpectedValue,
    /// Имя, которого нет ни среди переменных, ни среди функций. Сюда же
    /// попадают `now` и `rand`: их в языке нет СОЗНАТЕЛЬНО.
    UnknownName(String),
    /// Функции нужно другое число аргументов.
    Arity { name: String, want: usize, got: usize },
    /// Деление на ноль — ОШИБКА, а не бесконечность: молчаливая
    /// бесконечность уехала бы в кадр и стала бы чёрным экраном.
    DivZero,
    /// Результат не конечен (переполнение, `0/0`, корень из минуса).
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

/// Разобранное выражение. Разбирается ОДИН раз (при сборке рендерера),
/// считается на каждом кадре: текст в горячем пути не парсится.
#[derive(Debug, Clone, PartialEq)]
pub struct Formula {
    root: Node,
    /// Имена, которые выражение читает. Нужны тому, кто строит граф
    /// зависимостей: спрашивать у дерева на каждом кадре — дорого.
    pub reads: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Num(f64),
    Var(String),
    Neg(Box<Node>),
    Bin(Op, Box<Node>, Box<Node>),
    Call(Fun, Vec<Node>),
    /// `if(cond, a, b)` — ветвление БЕЗ инструкций: это выражение.
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
    /// Детерминированный шум от ЦЕЛОГО зерна — не из окружения.
    Noise,
}

/// Реестр функций вычислителя — имя и число аргументов (Р.67): справка в
/// блоке кривой строится ОТСЮДА, а не из копии в интерфейсе.
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
    /// Разобрать текст. Ошибка несёт позицию — поле ввода подсветит её.
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

    /// Посчитать при данных переменных.
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
                // ДЕТЕРМИНИРОВАННЫЙ шум: целое зерно в хэш, ничего из
                // окружения. Тот же алгоритм, что у counter-based шума
                // генератора, — второй «случайности» в программе нет.
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
    /// Сравнения — самый слабый уровень: `a < b` даёт 0 или 1.
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
