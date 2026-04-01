use super::format_detect::Format;
use super::ir::Node;
use super::token::{Token, TokenKind};
use super::tokenizer::TokenIterator;
use super::typed_value::ParsedTypedValue;
use super::{Error, ErrorKind, Result, Span};
use std::collections::VecDeque;

const MAX_NESTING: usize = 256;

pub fn parse_value(_input: &str) -> Result<Node> {
    parse_value_with_mode(_input, crate::typed_parse::ParseMode::Compat)
}

pub fn parse_value_with_mode(
    _input: &str,
    ttoon_mode: crate::typed_parse::ParseMode,
) -> Result<Node> {
    let format = super::format_detect::detect(_input);
    match format {
        Format::Tjson => parse_structure(_input),
        Format::Ttoon | Format::TypedUnit => crate::ttoon_parser::parse_ttoon(_input, ttoon_mode),
    }
}

pub(crate) fn parse_structure(_input: &str) -> Result<Node> {
    let mut cursor = TokenCursor::new(_input);
    let node = parse_value_tokens(&mut cursor, 0)?;

    if cursor.is_done() {
        Ok(node)
    } else if let Some(err) = cursor.pending_error() {
        Err(err)
    } else {
        Err(Error::new(
            ErrorKind::ParseError,
            "unexpected tokens after top-level value",
            cursor.peek_span(),
        ))
    }
}

fn parse_value_tokens(cursor: &mut TokenCursor, depth: usize) -> Result<Node> {
    let Some((kind, span)) = cursor.peek()?.map(|token| (token.kind.clone(), token.span)) else {
        return Err(Error::new(
            ErrorKind::ParseError,
            "unexpected end of input",
            None,
        ));
    };

    if depth > MAX_NESTING {
        return Err(Error::new(
            ErrorKind::ParseError,
            "structure nesting too deep",
            cursor.peek_span(),
        ));
    }

    match kind {
        TokenKind::LBrace => parse_object(cursor, depth + 1),
        TokenKind::LBracket => parse_list(cursor, depth + 1),
        TokenKind::String(_)
        | TokenKind::Number(_)
        | TokenKind::Keyword(_)
        | TokenKind::Typed(_) => parse_scalar_value_tokens(cursor, kind, span).map(Into::into),
        _ => Err(Error::new(
            ErrorKind::ParseError,
            "unexpected token",
            cursor.peek_span(),
        )),
    }
}

/// Parse a scalar token into the typed-value kernel first; Node materialization stays
/// at the parser boundary.
fn parse_scalar_value_tokens(
    cursor: &mut TokenCursor,
    kind: TokenKind,
    span: Span,
) -> Result<ParsedTypedValue> {
    match kind {
        TokenKind::String(raw) => {
            cursor.bump()?;
            crate::typed_parse::unescape_tjson_string(&raw)
                .map(ParsedTypedValue::String)
                .map_err(|e| Error::new(ErrorKind::LexError, e.message, Some(span)))
        }
        TokenKind::Number(value) => {
            if value == "+" || value == "-" {
                if let Some(next) = cursor.peek_next()? {
                    if let TokenKind::Keyword(keyword) = &next.kind {
                        if keyword == "inf" {
                            let sign = value;
                            cursor.bump()?;
                            cursor.bump()?;
                            return Ok(ParsedTypedValue::Float(if sign == "-" {
                                f64::NEG_INFINITY
                            } else {
                                f64::INFINITY
                            }));
                        }
                    }
                }
            }
            cursor.bump()?;
            crate::typed_parse::parse_number_like_typed_value(&value, span)
        }
        TokenKind::Keyword(value) => {
            cursor.bump()?;
            parse_keyword_typed_value(&value, span)
        }
        TokenKind::Typed(value) => {
            cursor.bump()?;
            crate::typed_parse::parse_unit_typed_value(
                &value,
                crate::typed_parse::ParseMode::Strict,
            )
            .map_err(|e| Error::new(ErrorKind::ParseError, e.message, Some(span)))
        }
        _ => Err(Error::new(
            ErrorKind::ParseError,
            "unexpected token",
            cursor.peek_span(),
        )),
    }
}

fn parse_list(cursor: &mut TokenCursor, depth: usize) -> Result<Node> {
    cursor.expect(TokenKind::LBracket)?;
    let mut items = Vec::new();

    while !cursor.is_done() {
        if cursor.consume_if(TokenKind::RBracket)? {
            return Ok(Node::List(items));
        }

        let value = parse_value_tokens(cursor, depth)?;
        items.push(value);

        if cursor.consume_if(TokenKind::Comma)? {
            if matches!(cursor.peek()?, Some(token) if token.kind == TokenKind::RBracket)
                || cursor.is_done()
            {
                return Err(Error::new(
                    ErrorKind::ParseError,
                    "expected value after ','",
                    cursor.peek_span(),
                ));
            }
            continue;
        }

        if cursor.consume_if(TokenKind::RBracket)? {
            return Ok(Node::List(items));
        }

        return Err(Error::new(
            ErrorKind::ParseError,
            "expected ',' or ']' in list",
            cursor.peek_span(),
        ));
    }

    Err(Error::new(
        ErrorKind::ParseError,
        "unterminated list",
        cursor.peek_span(),
    ))
}

fn parse_object(cursor: &mut TokenCursor, depth: usize) -> Result<Node> {
    use indexmap::IndexMap;

    cursor.expect(TokenKind::LBrace)?;
    let mut map = IndexMap::new();

    while !cursor.is_done() {
        if cursor.consume_if(TokenKind::RBrace)? {
            return Ok(Node::Object(map));
        }

        let key = match cursor.peek()? {
            Some(Token {
                kind: TokenKind::String(raw),
                span,
            }) => {
                let raw = raw.clone();
                let span = *span;
                cursor.bump()?;
                crate::typed_parse::unescape_tjson_string(&raw)
                    .map_err(|e| Error::new(ErrorKind::LexError, e.message, Some(span)))?
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::ParseError,
                    "object keys must be strings",
                    cursor.peek_span(),
                ));
            }
        };

        cursor.expect(TokenKind::Colon)?;
        let value = parse_value_tokens(cursor, depth)?;
        map.insert(key, value);

        if cursor.consume_if(TokenKind::Comma)? {
            if matches!(cursor.peek()?, Some(token) if token.kind == TokenKind::RBrace)
                || cursor.is_done()
            {
                return Err(Error::new(
                    ErrorKind::ParseError,
                    "expected value after ','",
                    cursor.peek_span(),
                ));
            }
            continue;
        }

        if cursor.consume_if(TokenKind::RBrace)? {
            return Ok(Node::Object(map));
        }

        return Err(Error::new(
            ErrorKind::ParseError,
            "expected ',' or '}' in object",
            cursor.peek_span(),
        ));
    }

    Err(Error::new(
        ErrorKind::ParseError,
        "unterminated object",
        cursor.peek_span(),
    ))
}

struct TokenCursor<'a> {
    tokens: VecDeque<Token>,
    iter: TokenIterator<'a>,
    pending_error: Option<Error>,
}

impl<'a> TokenCursor<'a> {
    fn new(input: &'a str) -> Self {
        let mut cursor = Self {
            tokens: VecDeque::with_capacity(2),
            iter: TokenIterator::new(input),
            pending_error: None,
        };
        cursor.fill_lookahead();
        cursor
    }

    fn fill_lookahead(&mut self) {
        while self.tokens.len() < 2 && self.pending_error.is_none() {
            match self.iter.next() {
                Some(Ok(token)) => self.tokens.push_back(token),
                Some(Err(err)) => self.pending_error = Some(err),
                None => break,
            }
        }
    }

    fn pending_error(&self) -> Option<Error> {
        if self.tokens.is_empty() {
            self.pending_error.clone()
        } else {
            None
        }
    }

    fn peek(&mut self) -> Result<Option<&Token>> {
        self.fill_lookahead();
        if let Some(token) = self.tokens.front() {
            Ok(Some(token))
        } else if let Some(err) = &self.pending_error {
            Err(err.clone())
        } else {
            Ok(None)
        }
    }

    fn peek_span(&mut self) -> Option<Span> {
        match self.peek() {
            Ok(Some(token)) => Some(token.span),
            Err(err) => err.span,
            Ok(None) => None,
        }
    }

    fn peek_next(&mut self) -> Result<Option<&Token>> {
        self.fill_lookahead();
        if let Some(token) = self.tokens.get(1) {
            Ok(Some(token))
        } else if self.tokens.len() < 2 {
            if let Some(err) = &self.pending_error {
                Err(err.clone())
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn bump(&mut self) -> Result<()> {
        if self.peek()?.is_some() {
            self.tokens.pop_front();
            self.fill_lookahead();
        }
        Ok(())
    }

    fn is_done(&self) -> bool {
        self.tokens.is_empty() && self.pending_error.is_none()
    }

    fn expect(&mut self, kind: TokenKind) -> Result<()> {
        match self.peek()? {
            Some(token) if token.kind == kind => {
                self.bump()?;
                Ok(())
            }
            _ => Err(Error::new(
                ErrorKind::ParseError,
                "unexpected token",
                self.peek_span(),
            )),
        }
    }

    fn consume_if(&mut self, kind: TokenKind) -> Result<bool> {
        if matches!(self.peek()?, Some(token) if token.kind == kind) {
            self.bump()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Handle Keyword tokens — delegates pure keywords to typed_parse SSOT.
fn parse_keyword_typed_value(keyword: &str, span: Span) -> Result<ParsedTypedValue> {
    match keyword {
        "null" | "true" | "false" | "inf" | "+inf" | "-inf" | "nan" => {
            crate::typed_parse::parse_keyword_typed_value(keyword, span)
        }
        _ => Err(Error::new(
            ErrorKind::ParseError,
            "unknown keyword",
            Some(span),
        )),
    }
}
