use anyhow::{Result, ensure};
use effect_model::equations::*;
use rand_mt::Mt;
use std::collections::{BTreeMap, BTreeSet};

const CLOSE: f64 = 0.00001;
const MEMORY_CELLS: usize = 1048576;
const PAGE: usize = 1024;
const MAX_PAGES: usize = 1024;
const MAX_STACK: usize = 512;

/// Sparse bounded pages; reads never allocate. Global banks are per preset,
/// explicitly shared among that preset's scopes, never process-global.
#[derive(Default)]
pub struct Memory {
    pages: BTreeMap<usize, Box<[f64; PAGE]>>,
}
impl Memory {
    pub fn get(&self, index: usize) -> Result<f64> {
        ensure!(
            index < MEMORY_CELLS,
            "memory address exceeds {MEMORY_CELLS} cells"
        );
        Ok(self
            .pages
            .get(&(index / PAGE))
            .map_or(0.0, |p| p[index % PAGE]))
    }
    pub fn set(&mut self, index: usize, value: f64) -> Result<()> {
        ensure!(
            index < MEMORY_CELLS,
            "memory address exceeds {MEMORY_CELLS} cells"
        );
        let page = index / PAGE;
        ensure!(
            self.pages.contains_key(&page) || self.pages.len() < MAX_PAGES,
            "memory page budget exceeded"
        );
        self.pages
            .entry(page)
            .or_insert_with(|| Box::new([0.0; PAGE]))[index % PAGE] = value;
        Ok(())
    }
    pub fn allocated_bytes(&self) -> usize {
        self.pages.len() * PAGE * 8
    }
}

pub struct Random {
    rng: Mt,
}
impl Random {
    pub fn new(seed: u32) -> Self {
        Self { rng: Mt::new(seed) }
    }
}

pub struct Vm {
    symbols: Vec<String>,
    indices: BTreeMap<String, usize>,
    values: Vec<f64>,
    local: Memory,
    stack: Vec<f64>,
    loops: Vec<(u64, usize)>,
    pub instructions_executed: u64,
    pub nonfinite_assignments: u64,
}
impl Vm {
    pub fn new(program: &Program) -> Result<Self> {
        validate(program)?;
        Ok(Self {
            symbols: program.symbols.clone(),
            indices: program
                .symbols
                .iter()
                .enumerate()
                .map(|(i, n)| (n.clone(), i))
                .collect(),
            values: vec![0.0; program.symbols.len()],
            local: Memory::default(),
            stack: Vec::with_capacity(MAX_STACK),
            loops: Vec::with_capacity(MAX_DEPTH),
            instructions_executed: 0,
            nonfinite_assignments: 0,
        })
    }
    pub fn get(&self, name: &str) -> f64 {
        self.indices.get(name).map_or(0.0, |i| self.values[*i])
    }
    pub fn set(&mut self, name: &str, value: f64) -> Result<()> {
        ensure!(value.is_finite(), "nonfinite external binding {name}");
        if let Some(i) = self.indices.get(name) {
            self.values[*i] = value;
        }
        Ok(())
    }
    pub fn has(&self, name: &str) -> bool {
        self.indices.contains_key(name)
    }
    pub fn local_memory_bytes(&self) -> usize {
        self.local.allocated_bytes()
    }
    pub fn execute(
        &mut self,
        program: &Program,
        memory: &mut Memory,
        random: &mut Random,
        budget: u64,
    ) -> Result<f64> {
        ensure!(
            program.abi == EQUATION_ABI && self.symbols == program.symbols,
            "program/scope mismatch"
        );
        self.execute_prevalidated(program, memory, random, budget)
    }
    // PresetState owns immutable programs validated against one shared symbol
    // layout at construction. Avoid repeated string-table comparisons per mesh
    // vertex/point; all instruction, memory and stack limits remain enforced.
    pub(crate) fn execute_prevalidated(
        &mut self,
        program: &Program,
        memory: &mut Memory,
        random: &mut Random,
        budget: u64,
    ) -> Result<f64> {
        ensure!(
            program.abi == EQUATION_ABI && self.values.len() == program.symbols.len(),
            "validated layout mismatch"
        );
        self.stack.clear();
        self.loops.clear();
        self.instructions_executed = 0;
        let mut at = 0;
        while at < program.instructions.len() {
            let instruction = &program.instructions[at];
            let span = &instruction.span;
            ensure!(
                self.instructions_executed < budget,
                "line {}: VM instruction budget {budget} exhausted",
                span.source_line
            );
            self.instructions_executed += 1;
            at += 1;
            match instruction.op {
                Op::Push(v) => self.stack.push(v),
                Op::Load(slot) => self.stack.push(
                    *self
                        .values
                        .get(slot as usize)
                        .ok_or_else(|| anyhow::anyhow!("invalid variable slot"))?,
                ),
                Op::Store(slot) => {
                    let v = self.pop()?;
                    let clean = self.clean_assignment(v);
                    *self
                        .values
                        .get_mut(slot as usize)
                        .ok_or_else(|| anyhow::anyhow!("invalid variable slot"))? = clean;
                    self.stack.push(clean);
                }
                Op::Pop => {
                    self.pop()?;
                }
                Op::Dup => {
                    let v = *self
                        .stack
                        .last()
                        .ok_or_else(|| anyhow::anyhow!("empty value stack"))?;
                    self.stack.push(v);
                }
                Op::Unary(op) => {
                    let v = self.pop()?;
                    self.stack.push(match op {
                        Unary::Neg => -v,
                        Unary::Not => (!truth(v)) as u8 as f64,
                        Unary::Bool => truth(v) as u8 as f64,
                    });
                }
                Op::Binary(op) => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack.push(binary(op, a, b));
                }
                Op::Call(function) => {
                    let b = if function.arity() == 2 {
                        self.pop()?
                    } else {
                        0.0
                    };
                    let a = self.pop()?;
                    let v = match function {
                        Function::Sin => a.sin(),
                        Function::Cos => a.cos(),
                        Function::Tan => a.tan(),
                        Function::Asin => a.asin(),
                        Function::Acos => a.acos(),
                        Function::Atan => a.atan(),
                        Function::Atan2 => a.atan2(b),
                        Function::Sqr => a * a,
                        Function::Sqrt => a.abs().sqrt(),
                        Function::Pow => a.powf(b),
                        Function::Exp => a.exp(),
                        Function::Log => a.ln(),
                        Function::Log10 => a.log10(),
                        Function::Abs => a.abs(),
                        Function::Min => a.min(b),
                        Function::Max => a.max(b),
                        Function::Sign => {
                            if a == 0.0 {
                                0.0
                            } else {
                                a.signum()
                            }
                        }
                        Function::Floor => a.floor(),
                        Function::Ceil => a.ceil(),
                        Function::Rand => {
                            ensure!(
                                a.is_finite(),
                                "line {}: nonfinite RNG bound",
                                span.source_line
                            );
                            random.rng.next_u32() as f64 / u32::MAX as f64 * a.floor().max(1.0)
                        }
                        Function::Band => (truth(a) && truth(b)) as u8 as f64,
                        Function::Bor => (truth(a) || truth(b)) as u8 as f64,
                        Function::Sigmoid => {
                            let t = 1.0 + (-a * b).exp();
                            if t.abs() > CLOSE { 1.0 / t } else { 0.0 }
                        }
                    };
                    self.stack.push(v);
                }
                Op::MemoryLoad { global } => {
                    let index = address(self.pop()?)?;
                    let v = if global {
                        memory.get(index)?
                    } else {
                        self.local.get(index)?
                    };
                    self.stack.push(v);
                }
                Op::MemoryStore { global } => {
                    let value = self.pop()?;
                    let value = self.clean_assignment(value);
                    let index = address(self.pop()?)?;
                    if global {
                        memory.set(index, value)?;
                    } else {
                        self.local.set(index, value)?;
                    }
                    self.stack.push(value);
                }
                Op::Jump(target) => {
                    ensure!(
                        target as usize <= program.instructions.len(),
                        "invalid jump target"
                    );
                    at = target as usize;
                }
                Op::JumpIfFalse(target) => {
                    let v = self.pop()?;
                    ensure!(
                        target as usize <= program.instructions.len(),
                        "invalid conditional target"
                    );
                    if !truth(v) {
                        at = target as usize;
                    }
                }
                Op::LoopStart { end } => {
                    let count = self.pop()?;
                    ensure!(
                        count.is_finite(),
                        "line {}: nonfinite loop count",
                        span.source_line
                    );
                    ensure!(
                        end as usize <= program.instructions.len(),
                        "invalid loop target"
                    );
                    let count = count.floor().max(0.0) as u64;
                    if count == 0 {
                        self.stack.push(0.0);
                        at = end as usize;
                    } else {
                        ensure!(
                            self.loops.len() < MAX_DEPTH,
                            "loop nesting budget exhausted"
                        );
                        self.loops.push((count, at));
                    }
                }
                Op::LoopEnd { start } => {
                    let (count, begin) = self
                        .loops
                        .last_mut()
                        .ok_or_else(|| anyhow::anyhow!("loop end without start"))?;
                    ensure!(*begin == start as usize, "loop control mismatch");
                    *count -= 1;
                    if *count > 0 {
                        at = start as usize;
                        self.pop()?;
                    } else {
                        self.loops.pop();
                    }
                }
            }
            ensure!(
                self.stack.len() <= MAX_STACK,
                "line {}: value stack budget exceeded",
                span.source_line
            );
        }
        ensure!(self.loops.is_empty(), "unfinished loop state");
        ensure!(
            self.stack.len() == 1,
            "equation result stack must contain exactly one value"
        );
        let result = self.pop()?;
        ensure!(result.is_finite(), "nonfinite unassigned equation result");
        Ok(result)
    }
    fn pop(&mut self) -> Result<f64> {
        self.stack
            .pop()
            .ok_or_else(|| anyhow::anyhow!("VM value stack underflow"))
    }
    fn clean_assignment(&mut self, value: f64) -> f64 {
        if !value.is_finite() {
            self.nonfinite_assignments += 1;
            0.0
        } else if value.is_subnormal() {
            0.0
        } else {
            value
        }
    }
}
fn truth(v: f64) -> bool {
    v.abs() > CLOSE
}
fn address(v: f64) -> Result<usize> {
    ensure!(
        v.is_finite() && v >= 0.0 && v < MEMORY_CELLS as f64,
        "invalid or over-budget memory address {v}"
    );
    Ok((v + CLOSE).trunc() as usize)
}
fn binary(op: Binary, a: f64, b: f64) -> f64 {
    match op {
        Binary::Add => a + b,
        Binary::Sub => a - b,
        Binary::Mul => a * b,
        Binary::Div => a / b,
        Binary::Pow => a.powf(b),
        Binary::Mod => {
            let denominator = b.abs().trunc() as u32;
            if denominator == 0 {
                0.0
            } else {
                ((a.abs().trunc() as u32) % denominator) as f64
            }
        }
        Binary::Equal => ((a - b).abs() < CLOSE) as u8 as f64,
        Binary::NotEqual => ((a - b).abs() >= CLOSE) as u8 as f64,
        Binary::Below => (a < b) as u8 as f64,
        Binary::Above => (a > b) as u8 as f64,
        Binary::BelowEq => (a <= b) as u8 as f64,
        Binary::AboveEq => (a >= b) as u8 as f64,
        Binary::BitAnd => ((a.trunc() as i64) & (b.trunc() as i64)) as f64,
        Binary::BitOr => ((a.trunc() as i64) | (b.trunc() as i64)) as f64,
    }
}
fn validate(p: &Program) -> Result<()> {
    ensure!(p.abi == EQUATION_ABI, "unsupported equation ABI");
    ensure!(p.symbols.len() <= MAX_SYMBOLS, "variable budget exceeded");
    ensure!(
        p.symbols.iter().collect::<BTreeSet<_>>().len() == p.symbols.len(),
        "duplicate symbols"
    );
    ensure!(
        !p.instructions.is_empty() && p.instructions.len() <= MAX_INSTRUCTIONS,
        "invalid instruction count"
    );
    for (i, instruction) in p.instructions.iter().enumerate() {
        match instruction.op {
            Op::Push(v) => ensure!(v.is_finite(), "nonfinite bytecode constant"),
            Op::Load(slot) | Op::Store(slot) => {
                ensure!((slot as usize) < p.symbols.len(), "invalid variable index")
            }
            Op::Jump(target) | Op::JumpIfFalse(target) => ensure!(
                target as usize <= p.instructions.len(),
                "invalid jump target"
            ),
            Op::LoopStart { end } => {
                ensure!(
                    end as usize > i + 1 && end as usize <= p.instructions.len(),
                    "invalid loop extent"
                );
                ensure!(
                    matches!(p.instructions[end as usize-1].op,Op::LoopEnd{start} if start as usize==i+1),
                    "invalid loop end"
                );
            }
            Op::LoopEnd { start } => ensure!(
                start > 0
                    && start as usize <= i
                    && matches!(p.instructions[start as usize-1].op,Op::LoopStart{end} if end as usize==i+1),
                "invalid loop start"
            ),
            _ => {}
        }
    }
    Ok(())
}
