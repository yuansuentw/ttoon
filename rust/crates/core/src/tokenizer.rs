use super::token::{Token, TokenKind};
use super::{Error, ErrorKind, Result, Span};

pub struct Tokenizer<'a> {
    input: &'a str,
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input }
    }

    pub fn tokenize(&self) -> Result<Vec<Token>> {
        let mut cursor = Cursor::new(self.input);
        let mut tokens = Vec::new();

        while let Some(token) = cursor.next_token() {
            tokens.push(token?);
        }

        Ok(tokens)
    }
}

/// Lazy token iterator: yields one Token per `next()` call.
/// Shares `Cursor` with `Tokenizer::tokenize()` — identical token emission behaviour.
pub struct TokenIterator<'a> {
    cursor: Cursor<'a>,
}

impl<'a> TokenIterator<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            cursor: Cursor::new(input),
        }
    }
}

impl<'a> Iterator for TokenIterator<'a> {
    type Item = Result<Token>;

    fn next(&mut self) -> Option<Self::Item> {
        self.cursor.next_token()
    }
}

struct Cursor<'a> {
    input: &'a str,
    iter: std::str::CharIndices<'a>,
    lookahead: Option<(usize, char)>,
    line: usize,
    column: usize,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            iter: input.char_indices(),
            lookahead: None,
            line: 1,
            column: 1,
        }
    }

    fn peek(&mut self) -> Option<char> {
        if self.lookahead.is_none() {
            self.lookahead = self.iter.next();
        }
        self.lookahead.map(|(_, ch)| ch)
    }

    fn bump(&mut self) -> Option<(usize, char)> {
        let next = if self.lookahead.is_some() {
            self.lookahead.take()
        } else {
            self.iter.next()
        };

        if let Some((_, ch)) = next {
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }

        next
    }

    /// Produce the next token, skipping whitespace and BOM.
    /// Returns `None` at EOF, `Some(Err)` on lex error, `Some(Ok)` otherwise.
    fn next_token(&mut self) -> Option<Result<Token>> {
        loop {
            let ch = self.peek()?;
            if ch.is_whitespace() || ch == '\u{FEFF}' {
                self.bump();
                continue;
            }

            let span = self.span();
            let token = match ch {
                '{' => {
                    self.bump();
                    Token::new(TokenKind::LBrace, span)
                }
                '}' => {
                    self.bump();
                    Token::new(TokenKind::RBrace, span)
                }
                '[' => {
                    self.bump();
                    Token::new(TokenKind::LBracket, span)
                }
                ']' => {
                    self.bump();
                    Token::new(TokenKind::RBracket, span)
                }
                ':' => {
                    self.bump();
                    Token::new(TokenKind::Colon, span)
                }
                ',' => {
                    self.bump();
                    Token::new(TokenKind::Comma, span)
                }
                '"' => match self.read_string() {
                    Ok(value) => Token::new(TokenKind::String(value), span),
                    Err(e) => return Some(Err(e)),
                },
                ch if ch.is_ascii_digit() || ch == '+' || ch == '-' => {
                    let value = self.read_number_like();
                    Token::new(TokenKind::Number(value), span)
                }
                ch if ch.is_ascii_alphabetic() => {
                    let value = self.read_keyword();
                    if matches!(value.as_str(), "uuid" | "hex" | "b64") {
                        if self.peek() == Some('(') {
                            match self.read_typed() {
                                Ok(content) => Token::new(
                                    TokenKind::Typed(format!("{value}({content})")),
                                    span,
                                ),
                                Err(e) => return Some(Err(e)),
                            }
                        } else {
                            Token::new(TokenKind::Keyword(value), span)
                        }
                    } else {
                        Token::new(TokenKind::Keyword(value), span)
                    }
                }
                _ => {
                    return Some(Err(Error::new(
                        ErrorKind::LexError,
                        "unexpected character",
                        Some(span),
                    )));
                }
            };

            return Some(Ok(token));
        }
    }

    fn span(&mut self) -> Span {
        let offset = self
            .peek()
            .and_then(|_| self.lookahead)
            .map_or(self.input.len(), |(idx, _)| idx);
        Span {
            offset,
            line: self.line,
            column: self.column,
        }
    }

    /// 識別字串邊界，回傳 raw 內容（保留 backslash escapes，不含外層 `"`）。
    /// unescape 由 typed_parse 的 unescape_ttoon_string / unescape_tjson_string 負責。
    fn read_string(&mut self) -> Result<String> {
        let start_span = self.span();
        let mut output = String::with_capacity(64);
        let _ = self.bump(); // consume opening "

        loop {
            let Some((_, ch)) = self.bump() else {
                return Err(Error::new(
                    ErrorKind::LexError,
                    "unterminated string",
                    Some(start_span),
                ));
            };

            match ch {
                '"' => break, // closing quote
                '\\' => {
                    // peek at next char to avoid treating \" as terminator,
                    // but keep both chars in output for later unescape
                    let Some((_, escaped)) = self.bump() else {
                        return Err(Error::new(
                            ErrorKind::LexError,
                            "unterminated escape sequence",
                            Some(start_span),
                        ));
                    };
                    output.push('\\');
                    output.push(escaped);
                    // for \uXXXX: consume the 4 hex digits and preserve them
                    if escaped == 'u' {
                        for _ in 0..4 {
                            let Some((_, hc)) = self.bump() else {
                                return Err(Error::new(
                                    ErrorKind::LexError,
                                    "incomplete unicode escape",
                                    Some(start_span),
                                ));
                            };
                            output.push(hc);
                        }
                    }
                }
                _ => {
                    if ch.is_control() {
                        return Err(Error::new(
                            ErrorKind::LexError,
                            "invalid control character in string",
                            Some(start_span),
                        ));
                    }
                    output.push(ch);
                }
            }
        }

        Ok(output)
    }

    fn read_number_like(&mut self) -> String {
        let mut output = String::with_capacity(32);
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit()
                || matches!(
                    ch,
                    '+' | '-' | '.' | 'e' | 'E' | '_' | 'm' | ':' | 'T' | 'Z'
                )
            {
                let (_, ch) = self.bump().expect("peeked char missing");
                output.push(ch);
            } else {
                break;
            }
        }
        output
    }

    fn read_keyword(&mut self) -> String {
        let mut output = String::with_capacity(16);
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                let (_, ch) = self.bump().expect("peeked char missing");
                output.push(ch);
            } else {
                break;
            }
        }
        output
    }

    /// Read T-TOON typed wrapper content: consumes '(' then reads until matching ')'
    fn read_typed(&mut self) -> Result<String> {
        let start_span = self.span();
        // Consume the opening '('
        self.bump();
        let mut output = String::with_capacity(64);
        loop {
            let Some((_, ch)) = self.bump() else {
                return Err(Error::new(
                    ErrorKind::LexError,
                    "unterminated typed wrapper",
                    Some(start_span),
                ));
            };
            if ch == ')' {
                break;
            }
            output.push(ch);
        }
        Ok(output)
    }
}
