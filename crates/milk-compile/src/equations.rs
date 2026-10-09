use crate::eqparse::{Expr, Node, Parser};
use anyhow::{Result, bail, ensure};
use effect_model::equations::*;
use milk_import::parser::SourceLine;
use std::collections::BTreeMap;

pub fn compile(source: &str) -> Result<Program> {
    compile_mapped(source, &[(0, 1)])
}
pub fn compile_lines(lines: &[SourceLine]) -> Result<Program> {
    let mut source = String::new();
    let mut origins = vec![];
    for line in lines {
        origins.push((source.len(), line.source_line));
        source.push_str(&line.text);
        source.push('\n');
    }
    if origins.is_empty() {
        origins.push((0, 1));
    }
    compile_mapped(&source, &origins)
}
fn compile_mapped(source: &str, origins: &[(usize, u32)]) -> Result<Program> {
    ensure!(
        source.len() <= 512 * 1024,
        "equation source size limit exceeded"
    );
    let node = Parser::new(source, origins)?.parse()?;
    let mut lower = Lower {
        program: Program {
            abi: EQUATION_ABI,
            symbols: vec![],
            instructions: vec![],
        },
        slots: BTreeMap::new(),
    };
    lower.emit(&node)?;
    Ok(lower.program)
}
struct Lower {
    program: Program,
    slots: BTreeMap<String, u16>,
}
impl Lower {
    fn op(&mut self, op: Op, span: &Span) -> Result<usize> {
        ensure!(
            self.program.instructions.len() < MAX_INSTRUCTIONS,
            "line {}: instruction limit exceeded",
            span.source_line
        );
        let index = self.program.instructions.len();
        self.program.instructions.push(Instruction {
            op,
            span: span.clone(),
        });
        Ok(index)
    }
    fn slot(&mut self, name: &str) -> Result<u16> {
        if let Some(v) = self.slots.get(name) {
            return Ok(*v);
        }
        ensure!(
            self.program.symbols.len() < MAX_SYMBOLS,
            "symbol limit exceeded"
        );
        let id = self.program.symbols.len() as u16;
        self.program.symbols.push(name.into());
        self.slots.insert(name.into(), id);
        Ok(id)
    }
    fn emit(&mut self, n: &Node) -> Result<()> {
        let span = &n.span;
        match &n.expr {
            Expr::Number(v) => {
                self.op(Op::Push(*v), span)?;
            }
            Expr::Name(name) => {
                let constant = match name.as_str() {
                    "$pi" => Some(std::f64::consts::PI),
                    "$e" => Some(std::f64::consts::E),
                    _ => None,
                };
                if let Some(v) = constant {
                    self.op(Op::Push(v), span)?;
                } else {
                    let slot = self.slot(name)?;
                    self.op(Op::Load(slot), span)?;
                }
            }
            Expr::Sequence(nodes) => {
                if nodes.is_empty() {
                    self.op(Op::Push(0.0), span)?;
                }
                for (i, node) in nodes.iter().enumerate() {
                    self.emit(node)?;
                    if i + 1 < nodes.len() {
                        self.op(Op::Pop, &node.span)?;
                    }
                }
            }
            Expr::Unary(op, value) => {
                self.emit(value)?;
                self.op(Op::Unary(*op), span)?;
            }
            Expr::Binary(op, a, b) => {
                self.emit(a)?;
                self.emit(b)?;
                self.op(Op::Binary(*op), span)?;
            }
            Expr::Logical(and, a, b) => {
                self.emit(a)?;
                let branch = self.op(Op::JumpIfFalse(0), span)?;
                if *and {
                    self.emit(b)?;
                    self.op(Op::Unary(Unary::Bool), span)?;
                } else {
                    self.op(Op::Push(1.0), span)?;
                }
                let done = self.op(Op::Jump(0), span)?;
                let other = self.program.instructions.len() as u32;
                if *and {
                    self.op(Op::Push(0.0), span)?;
                } else {
                    self.emit(b)?;
                    self.op(Op::Unary(Unary::Bool), span)?;
                }
                let end = self.program.instructions.len() as u32;
                self.program.instructions[branch].op = Op::JumpIfFalse(other);
                self.program.instructions[done].op = Op::Jump(end);
            }
            Expr::Assign(binary, lhs, rhs) => {
                let lhs = unsequence(lhs);
                match &lhs.expr {
                    Expr::Name(name) if !name.starts_with('$') => {
                        let slot = self.slot(name)?;
                        if binary.is_some() {
                            self.op(Op::Load(slot), span)?;
                        }
                        self.emit(rhs)?;
                        if let Some(op) = binary {
                            self.op(Op::Binary(*op), span)?;
                        }
                        self.op(Op::Store(slot), span)?;
                    }
                    Expr::Call(name, args)
                        if matches!(name.as_str(), "megabuf" | "gmegabuf") && args.len() == 1 =>
                    {
                        let global = name == "gmegabuf";
                        self.emit(&args[0])?;
                        if binary.is_some() {
                            self.op(Op::Dup, span)?;
                            self.op(Op::MemoryLoad { global }, span)?;
                        }
                        self.emit(rhs)?;
                        if let Some(op) = binary {
                            self.op(Op::Binary(*op), span)?;
                        }
                        self.op(Op::MemoryStore { global }, span)?;
                    }
                    _ => bail!(
                        "line {} column {}: invalid assignment target",
                        span.source_line,
                        span.column
                    ),
                }
            }
            Expr::Call(name, args) => match name.as_str() {
                "if" => {
                    self.arity(args, 3, span)?;
                    self.emit(&args[0])?;
                    let branch = self.op(Op::JumpIfFalse(0), span)?;
                    self.emit(&args[1])?;
                    let jump = self.op(Op::Jump(0), span)?;
                    let other = self.program.instructions.len() as u32;
                    self.emit(&args[2])?;
                    let end = self.program.instructions.len() as u32;
                    self.program.instructions[branch].op = Op::JumpIfFalse(other);
                    self.program.instructions[jump].op = Op::Jump(end);
                }
                "loop" => {
                    self.arity(args, 2, span)?;
                    self.emit(&args[0])?;
                    let head = self.op(Op::LoopStart { end: 0 }, span)?;
                    let start = self.program.instructions.len() as u32;
                    self.emit(&args[1])?;
                    self.op(Op::LoopEnd { start }, span)?;
                    self.program.instructions[head].op = Op::LoopStart {
                        end: self.program.instructions.len() as u32,
                    };
                }
                "megabuf" | "gmegabuf" => {
                    self.arity(args, 1, span)?;
                    self.emit(&args[0])?;
                    self.op(
                        Op::MemoryLoad {
                            global: name == "gmegabuf",
                        },
                        span,
                    )?;
                }
                "assign" => {
                    self.arity(args, 2, span)?;
                    let lhs = unsequence(&args[0]);
                    match &lhs.expr {
                        Expr::Name(name) => {
                            let slot = self.slot(name)?;
                            self.emit(&args[1])?;
                            self.op(Op::Store(slot), span)?;
                        }
                        _ => bail!("line {}: unsupported assign target", span.source_line),
                    }
                }
                "bnot" => {
                    self.arity(args, 1, span)?;
                    self.emit(&args[0])?;
                    self.op(Op::Unary(Unary::Not), span)?;
                }
                "above" | "below" | "equal" => {
                    self.arity(args, 2, span)?;
                    self.emit(&args[0])?;
                    self.emit(&args[1])?;
                    self.op(
                        Op::Binary(match name.as_str() {
                            "above" => Binary::Above,
                            "below" => Binary::Below,
                            _ => Binary::Equal,
                        }),
                        span,
                    )?;
                }
                "exec2" | "exec3" => {
                    self.arity(args, if name == "exec2" { 2 } else { 3 }, span)?;
                    for (i, arg) in args.iter().enumerate() {
                        self.emit(arg)?;
                        if i + 1 < args.len() {
                            self.op(Op::Pop, span)?;
                        }
                    }
                }
                _ => {
                    let function = match name.as_str() {
                        "sin" => Function::Sin,
                        "cos" => Function::Cos,
                        "tan" => Function::Tan,
                        "asin" => Function::Asin,
                        "acos" => Function::Acos,
                        "atan" => Function::Atan,
                        "atan2" => Function::Atan2,
                        "sqr" => Function::Sqr,
                        "sqrt" => Function::Sqrt,
                        "pow" => Function::Pow,
                        "exp" => Function::Exp,
                        "log" => Function::Log,
                        "log10" => Function::Log10,
                        "abs" => Function::Abs,
                        "min" => Function::Min,
                        "max" => Function::Max,
                        "sign" => Function::Sign,
                        "int" | "floor" => Function::Floor,
                        "ceil" => Function::Ceil,
                        "rand" => Function::Rand,
                        "band" => Function::Band,
                        "bor" => Function::Bor,
                        "sigmoid" => Function::Sigmoid,
                        _ => bail!(
                            "line {} column {}: unsupported equation function {name}",
                            span.source_line,
                            span.column
                        ),
                    };
                    self.arity(args, function.arity(), span)?;
                    for arg in args {
                        self.emit(arg)?;
                    }
                    self.op(Op::Call(function), span)?;
                }
            },
        }
        Ok(())
    }
    fn arity(&self, args: &[Node], count: usize, span: &Span) -> Result<()> {
        ensure!(
            args.len() == count,
            "line {} column {}: expected {count} arguments, got {}",
            span.source_line,
            span.column,
            args.len()
        );
        Ok(())
    }
}
fn unsequence(mut n: &Node) -> &Node {
    while let Expr::Sequence(nodes) = &n.expr {
        if nodes.len() != 1 {
            break;
        }
        n = &nodes[0];
    }
    n
}
