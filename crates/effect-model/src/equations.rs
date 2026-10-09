use serde::{Deserialize, Serialize};

pub const EQUATION_ABI: u32 = 1;
pub const MAX_SYMBOLS: usize = 4096;
pub const MAX_INSTRUCTIONS: usize = 131072;
pub const MAX_DEPTH: usize = 128;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Span {
    pub source_line: u32,
    pub column: u32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Binary {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Below,
    Above,
    BelowEq,
    AboveEq,
    Equal,
    NotEqual,
    BitAnd,
    BitOr,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Unary {
    Neg,
    Not,
    Bool,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Function {
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Sqr,
    Sqrt,
    Pow,
    Exp,
    Log,
    Log10,
    Abs,
    Min,
    Max,
    Sign,
    Floor,
    Ceil,
    Rand,
    Band,
    Bor,
    Sigmoid,
}
impl Function {
    pub fn arity(self) -> usize {
        match self {
            Self::Atan2
            | Self::Pow
            | Self::Min
            | Self::Max
            | Self::Band
            | Self::Bor
            | Self::Sigmoid => 2,
            _ => 1,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Op {
    Push(f64),
    Load(u16),
    Store(u16),
    Pop,
    Dup,
    Unary(Unary),
    Binary(Binary),
    Call(Function),
    MemoryLoad { global: bool },
    MemoryStore { global: bool },
    Jump(u32),
    JumpIfFalse(u32),
    LoopStart { end: u32 },
    LoopEnd { start: u32 },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Instruction {
    pub op: Op,
    pub span: Span,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Program {
    pub abi: u32,
    pub symbols: Vec<String>,
    pub instructions: Vec<Instruction>,
}

/// Equation artifact only. Shaders/effect graph must validate before playback.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EquationBundle {
    pub abi: u32,
    pub symbols: Vec<String>,
    pub sections: std::collections::BTreeMap<String, Vec<Instruction>>,
    pub static_values: std::collections::BTreeMap<String, f64>,
}
