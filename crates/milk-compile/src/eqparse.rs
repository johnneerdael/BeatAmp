use anyhow::{Result, bail, ensure};
use effect_model::equations::{Binary, MAX_DEPTH, Span, Unary};
use logos::Logos;

#[derive(Logos, Clone, Debug, PartialEq)]
#[logos(skip r"[ \t\r\n]+")]
#[logos(skip(r"//[^\r\n]*", allow_greedy = true))]
pub(crate) enum Token {
    #[regex(r"([0-9]+(\.[0-9]*)?|\.[0-9]+)([eE][+-]?[0-9]+)?", |l| l.slice().parse::<f64>().ok())]
    Number(f64),
    #[regex(r"[$a-zA-Z_][$a-zA-Z_0-9]*", |l| l.slice().to_ascii_lowercase())]
    Name(String),
    #[token("+")]
    Add,
    #[token("-")]
    Sub,
    #[token("*")]
    Mul,
    #[token("/")]
    Div,
    #[token("%")]
    Mod,
    #[token("^")]
    Pow,
    #[token("=")]
    Assign,
    #[token("+=")]
    AddAssign,
    #[token("-=")]
    SubAssign,
    #[token("*=")]
    MulAssign,
    #[token("/=")]
    DivAssign,
    #[token("%=")]
    ModAssign,
    #[token("^=")]
    PowAssign,
    #[token("&=")]
    AndAssign,
    #[token("|=")]
    OrAssign,
    #[token("==")]
    Equal,
    #[token("!=")]
    NotEqual,
    #[token("<")]
    Below,
    #[token(">")]
    Above,
    #[token("<=")]
    BelowEq,
    #[token(">=")]
    AboveEq,
    #[token("&&")]
    And,
    #[token("||")]
    Or,
    #[token("&")]
    BitAnd,
    #[token("|")]
    BitOr,
    #[token("!")]
    Not,
    #[token("(")]
    Open,
    #[token(")")]
    Close,
    #[token(",")]
    Comma,
    #[token(";")]
    Semi,
    #[token("?")]
    Question,
    #[token(":")]
    Colon,
}

pub(crate) enum Expr {
    Number(f64),
    Name(String),
    Sequence(Vec<Node>),
    Unary(Unary, Box<Node>),
    Binary(Binary, Box<Node>, Box<Node>),
    Logical(bool, Box<Node>, Box<Node>),
    Assign(Option<Binary>, Box<Node>, Box<Node>),
    Call(String, Vec<Node>),
}
pub(crate) struct Node {
    pub expr: Expr,
    pub span: Span,
    height: usize,
}
impl Node {
    fn new(expr: Expr, span: Span, height: usize) -> Result<Self> {
        ensure!(
            height <= MAX_DEPTH,
            "line {} column {}: equation nesting exceeds {MAX_DEPTH}",
            span.source_line,
            span.column
        );
        Ok(Self { expr, span, height })
    }
}

pub(crate) struct Parser {
    tokens: Vec<(Token, Span)>,
    next: usize,
    depth: usize,
    fallback: Span,
}
impl Parser {
    pub fn new(source: &str, origins: &[(usize, u32)]) -> Result<Self> {
        let mut tokens = vec![];
        for (token, range) in Token::lexer(source).spanned() {
            let origin = origins
                .partition_point(|(offset, _)| *offset <= range.start)
                .saturating_sub(1);
            let (offset, line) = origins[origin];
            let prefix = &source[offset..range.start];
            let extra_lines = prefix.bytes().filter(|b| *b == b'\n').count() as u32;
            let column = prefix.rsplit('\n').next().unwrap_or("").len() as u32 + 1;
            let span = Span {
                source_line: line + extra_lines,
                column,
            };
            let token = token.map_err(|_| {
                anyhow::anyhow!(
                    "line {} column {}: invalid equation token",
                    span.source_line,
                    span.column
                )
            })?;
            tokens.push((token, span));
            ensure!(tokens.len() <= 131072, "equation token limit exceeded");
        }
        Ok(Self {
            tokens,
            next: 0,
            depth: 0,
            fallback: Span {
                source_line: origins[0].1,
                column: 1,
            },
        })
    }
    pub fn parse(mut self) -> Result<Node> {
        let result = self.sequence()?;
        ensure!(
            self.next == self.tokens.len(),
            "line {}: unexpected closing token",
            self.span().source_line
        );
        Ok(result)
    }
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.next).map(|v| &v.0)
    }
    fn span(&self) -> Span {
        self.tokens
            .get(self.next)
            .or_else(|| self.tokens.last())
            .map(|v| v.1.clone())
            .unwrap_or_else(|| self.fallback.clone())
    }
    fn take(&mut self) -> Result<(Token, Span)> {
        let pair = self.tokens.get(self.next).cloned().ok_or_else(|| {
            anyhow::anyhow!(
                "line {}: unexpected end of equation",
                self.span().source_line
            )
        })?;
        self.next += 1;
        Ok(pair)
    }
    fn expect(&mut self, token: Token) -> Result<()> {
        let (actual, span) = self.take()?;
        ensure!(
            actual == token,
            "line {} column {}: expected {token:?}, got {actual:?}",
            span.source_line,
            span.column
        );
        Ok(())
    }
    fn sequence(&mut self) -> Result<Node> {
        let span = self.span();
        let mut nodes = vec![];
        while !matches!(
            self.peek(),
            None | Some(Token::Close | Token::Comma | Token::Colon)
        ) {
            if matches!(self.peek(), Some(Token::Semi)) {
                self.next += 1;
                continue;
            }
            nodes.push(self.expression(0)?);
            match self.peek() {
                Some(Token::Semi) => {
                    self.next += 1;
                }
                None | Some(Token::Close | Token::Comma | Token::Colon) => break,
                _ => bail!(
                    "line {} column {}: expected equation separator",
                    self.span().source_line,
                    self.span().column
                ),
            }
        }
        let height = nodes.iter().map(|n| n.height).max().unwrap_or(0) + 1;
        Node::new(Expr::Sequence(nodes), span, height)
    }
    fn expression(&mut self, min: u8) -> Result<Node> {
        self.depth += 1;
        ensure!(
            self.depth <= MAX_DEPTH,
            "line {}: parser nesting limit exceeded",
            self.span().source_line
        );
        let (token, span) = self.take()?;
        let mut left = match token {
            Token::Number(v) => {
                ensure!(
                    v.is_finite(),
                    "line {}: nonfinite numeric literal",
                    span.source_line
                );
                Node::new(Expr::Number(v), span, 1)?
            }
            Token::Name(name) => {
                if matches!(self.peek(), Some(Token::Open)) {
                    self.next += 1;
                    let mut args = vec![];
                    if !matches!(self.peek(), Some(Token::Close)) {
                        loop {
                            args.push(self.sequence()?);
                            ensure!(
                                args.len() <= 3,
                                "line {}: function argument limit exceeded",
                                span.source_line
                            );
                            if matches!(self.peek(), Some(Token::Comma)) {
                                self.next += 1;
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::Close)?;
                    let height = args.iter().map(|n| n.height).max().unwrap_or(0) + 1;
                    Node::new(Expr::Call(name, args), span, height)?
                } else {
                    Node::new(Expr::Name(name), span, 1)?
                }
            }
            Token::Sub | Token::Add | Token::Not => {
                let value = self.expression(20)?;
                let height = value.height + 1;
                match token {
                    Token::Add => value,
                    Token::Sub => {
                        Node::new(Expr::Unary(Unary::Neg, Box::new(value)), span, height)?
                    }
                    _ => Node::new(Expr::Unary(Unary::Not, Box::new(value)), span, height)?,
                }
            }
            Token::Open => {
                let node = self.sequence()?;
                self.expect(Token::Close)?;
                node
            }
            _ => bail!(
                "line {} column {}: expected expression, got {token:?}",
                span.source_line,
                span.column
            ),
        };
        loop {
            if matches!(self.peek(), Some(Token::Question)) {
                if 2 < min {
                    break;
                }
                self.next += 1;
                let yes = self.sequence()?;
                self.expect(Token::Colon)?;
                let no = self.expression(2)?;
                let height = 1 + left.height.max(yes.height).max(no.height);
                let span = left.span.clone();
                left = Node::new(Expr::Call("if".into(), vec![left, yes, no]), span, height)?;
                continue;
            }
            let Some((l, r)) = self.peek().and_then(binding) else {
                break;
            };
            if l < min {
                break;
            }
            let (op, _) = self.take()?;
            let right = self.expression(r)?;
            let height = left.height.max(right.height) + 1;
            let span = left.span.clone();
            let expr = match op {
                Token::Assign => Expr::Assign(None, Box::new(left), Box::new(right)),
                Token::AddAssign
                | Token::SubAssign
                | Token::MulAssign
                | Token::DivAssign
                | Token::ModAssign
                | Token::PowAssign
                | Token::AndAssign
                | Token::OrAssign => {
                    Expr::Assign(Some(binary(&op)), Box::new(left), Box::new(right))
                }
                Token::And | Token::Or => {
                    Expr::Logical(op == Token::And, Box::new(left), Box::new(right))
                }
                _ => Expr::Binary(binary(&op), Box::new(left), Box::new(right)),
            };
            left = Node::new(expr, span, height)?;
        }
        self.depth -= 1;
        Ok(left)
    }
}
fn binding(t: &Token) -> Option<(u8, u8)> {
    Some(match t {
        Token::Assign
        | Token::AddAssign
        | Token::SubAssign
        | Token::MulAssign
        | Token::DivAssign
        | Token::ModAssign
        | Token::PowAssign
        | Token::AndAssign
        | Token::OrAssign => (1, 1),
        Token::Or => (3, 4),
        Token::And => (5, 6),
        Token::BitOr => (7, 8),
        Token::BitAnd => (9, 10),
        Token::Equal | Token::NotEqual => (11, 12),
        Token::Below | Token::Above | Token::BelowEq | Token::AboveEq => (13, 14),
        Token::Add | Token::Sub => (15, 16),
        Token::Mul | Token::Div | Token::Mod => (17, 18),
        Token::Pow => (21, 20),
        _ => return None,
    })
}
fn binary(t: &Token) -> Binary {
    match t {
        Token::Add | Token::AddAssign => Binary::Add,
        Token::Sub | Token::SubAssign => Binary::Sub,
        Token::Mul | Token::MulAssign => Binary::Mul,
        Token::Div | Token::DivAssign => Binary::Div,
        Token::Mod | Token::ModAssign => Binary::Mod,
        Token::Pow | Token::PowAssign => Binary::Pow,
        Token::BitAnd | Token::AndAssign => Binary::BitAnd,
        Token::BitOr | Token::OrAssign => Binary::BitOr,
        Token::Below => Binary::Below,
        Token::Above => Binary::Above,
        Token::BelowEq => Binary::BelowEq,
        Token::AboveEq => Binary::AboveEq,
        Token::Equal => Binary::Equal,
        Token::NotEqual => Binary::NotEqual,
        _ => unreachable!(),
    }
}
