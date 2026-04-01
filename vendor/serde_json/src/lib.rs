use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt;
use toml::map::Map;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct Error {
    message: String,
}

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

pub fn to_string_pretty<T>(value: &T) -> Result<String>
where
    T: Serialize,
{
    let value = toml::Value::try_from(value).map_err(|error| Error::new(error.to_string()))?;
    format_value(&value, 0)
}

pub fn from_str<T>(input: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    let mut parser = Parser::new(input);
    let value = parser.parse_root_value()?;
    parser.skip_ws();
    if !parser.is_eof() {
        return Err(Error::new("unexpected trailing characters"));
    }

    value
        .try_into()
        .map_err(|error: toml::de::Error| Error::new(error.to_string()))
}

fn format_value(value: &toml::Value, depth: usize) -> Result<String> {
    match value {
        toml::Value::String(text) => Ok(format!("\"{}\"", escape_string(text))),
        toml::Value::Integer(number) => Ok(number.to_string()),
        toml::Value::Float(number) => Ok(number.to_string()),
        toml::Value::Boolean(boolean) => Ok(boolean.to_string()),
        toml::Value::Array(values) => {
            if values.is_empty() {
                return Ok("[]".to_string());
            }

            let indent = "  ".repeat(depth + 1);
            let closing = "  ".repeat(depth);
            let mut lines = Vec::with_capacity(values.len() + 2);
            lines.push("[".to_string());
            for (index, item) in values.iter().enumerate() {
                let suffix = if index + 1 == values.len() { "" } else { "," };
                lines.push(format!("{}{}{}", indent, format_value(item, depth + 1)?, suffix));
            }
            lines.push(format!("{closing}]"));
            Ok(lines.join("\n"))
        }
        toml::Value::Table(table) => {
            if table.is_empty() {
                return Ok("{}".to_string());
            }

            let indent = "  ".repeat(depth + 1);
            let closing = "  ".repeat(depth);
            let mut lines = Vec::with_capacity(table.len() + 2);
            lines.push("{".to_string());
            for (index, (key, item)) in table.iter().enumerate() {
                let suffix = if index + 1 == table.len() { "" } else { "," };
                lines.push(format!(
                    "{}\"{}\": {}{}",
                    indent,
                    escape_string(key),
                    format_value(item, depth + 1)?,
                    suffix
                ));
            }
            lines.push(format!("{closing}}}"));
            Ok(lines.join("\n"))
        }
        toml::Value::Datetime(value) => Ok(format!("\"{}\"", value)),
    }
}

fn escape_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[derive(Debug)]
enum ParsedValue {
    Value(toml::Value),
    Null,
}

struct Parser<'a> {
    input: &'a str,
    index: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, index: 0 }
    }

    fn parse_root_value(&mut self) -> Result<toml::Value> {
        match self.parse_value()? {
            ParsedValue::Value(value) => Ok(value),
            ParsedValue::Null => Err(Error::new("null is not a valid root value")),
        }
    }

    fn parse_value(&mut self) -> Result<ParsedValue> {
        self.skip_ws();
        match self.peek() {
            Some('{') => self.parse_object().map(ParsedValue::Value),
            Some('[') => self.parse_array().map(ParsedValue::Value),
            Some('"') => self.parse_string().map(toml::Value::String).map(ParsedValue::Value),
            Some('t') | Some('f') => self.parse_bool().map(toml::Value::Boolean).map(ParsedValue::Value),
            Some('n') => {
                self.expect_literal("null")?;
                Ok(ParsedValue::Null)
            }
            Some('-') | Some('0'..='9') => self.parse_number().map(ParsedValue::Value),
            Some(ch) => Err(Error::new(format!("unexpected character: {ch}"))),
            None => Err(Error::new("unexpected end of input")),
        }
    }

    fn parse_object(&mut self) -> Result<toml::Value> {
        self.expect_char('{')?;
        self.skip_ws();
        let mut table = Map::new();

        if self.peek() == Some('}') {
            self.index += 1;
            return Ok(toml::Value::Table(table));
        }

        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect_char(':')?;
            self.skip_ws();

            if let ParsedValue::Value(value) = self.parse_value()? {
                table.insert(key, value);
            }

            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.index += 1;
                }
                Some('}') => {
                    self.index += 1;
                    break;
                }
                Some(ch) => return Err(Error::new(format!("unexpected character: {ch}"))),
                None => return Err(Error::new("unterminated object")),
            }
        }

        Ok(toml::Value::Table(table))
    }

    fn parse_array(&mut self) -> Result<toml::Value> {
        self.expect_char('[')?;
        self.skip_ws();
        let mut values = Vec::new();

        if self.peek() == Some(']') {
            self.index += 1;
            return Ok(toml::Value::Array(values));
        }

        loop {
            match self.parse_value()? {
                ParsedValue::Value(value) => values.push(value),
                ParsedValue::Null => return Err(Error::new("null is not supported in arrays")),
            }

            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.index += 1;
                }
                Some(']') => {
                    self.index += 1;
                    break;
                }
                Some(ch) => return Err(Error::new(format!("unexpected character: {ch}"))),
                None => return Err(Error::new("unterminated array")),
            }
        }

        Ok(toml::Value::Array(values))
    }

    fn parse_string(&mut self) -> Result<String> {
        self.expect_char('"')?;
        let mut value = String::new();

        while let Some(ch) = self.next_char() {
            match ch {
                '"' => return Ok(value),
                '\\' => {
                    let escaped = self
                        .next_char()
                        .ok_or_else(|| Error::new("unterminated escape sequence"))?;
                    match escaped {
                        '"' => value.push('"'),
                        '\\' => value.push('\\'),
                        '/' => value.push('/'),
                        'b' => value.push('\u{0008}'),
                        'f' => value.push('\u{000C}'),
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        'u' => return Err(Error::new("unicode escapes are not supported")),
                        other => {
                            return Err(Error::new(format!("invalid escape sequence: \\{other}")));
                        }
                    }
                }
                other => value.push(other),
            }
        }

        Err(Error::new("unterminated string"))
    }

    fn parse_bool(&mut self) -> Result<bool> {
        if self.remaining().starts_with("true") {
            self.index += 4;
            Ok(true)
        } else if self.remaining().starts_with("false") {
            self.index += 5;
            Ok(false)
        } else {
            Err(Error::new("invalid boolean"))
        }
    }

    fn parse_number(&mut self) -> Result<toml::Value> {
        let start = self.index;
        if self.peek() == Some('-') {
            self.index += 1;
        }

        self.consume_digits();
        if self.peek() == Some('.') {
            self.index += 1;
            self.consume_digits();
            let number = self.input[start..self.index]
                .parse::<f64>()
                .map_err(|error| Error::new(error.to_string()))?;
            Ok(toml::Value::Float(number))
        } else {
            let number = self.input[start..self.index]
                .parse::<i64>()
                .map_err(|error| Error::new(error.to_string()))?;
            Ok(toml::Value::Integer(number))
        }
    }

    fn consume_digits(&mut self) {
        while matches!(self.peek(), Some('0'..='9')) {
            self.index += 1;
        }
    }

    fn expect_literal(&mut self, literal: &str) -> Result<()> {
        if self.remaining().starts_with(literal) {
            self.index += literal.len();
            Ok(())
        } else {
            Err(Error::new(format!("expected literal: {literal}")))
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<()> {
        match self.next_char() {
            Some(ch) if ch == expected => Ok(()),
            Some(ch) => Err(Error::new(format!("expected '{expected}', found '{ch}'"))),
            None => Err(Error::new(format!("expected '{expected}', found end of input"))),
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(ch) if ch.is_whitespace()) {
            self.index += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn next_char(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.index += ch.len_utf8();
        Some(ch)
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.index..]
    }

    fn is_eof(&self) -> bool {
        self.index >= self.input.len()
    }
}
