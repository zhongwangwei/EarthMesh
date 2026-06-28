use std::collections::BTreeMap;
use std::io;

use super::JsonNode;

pub(crate) struct JsonParser<'a> {
    text: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        Self {
            text: text.as_bytes(),
            pos: 0,
        }
    }

    pub(crate) fn parse(mut self) -> io::Result<JsonNode> {
        let value = self.parse_value()?;
        self.skip_ws();
        if self.pos != self.text.len() {
            return Err(json_parse_error("trailing JSON content"));
        }
        Ok(value)
    }

    fn parse_value(&mut self) -> io::Result<JsonNode> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => self.parse_string().map(JsonNode::String),
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number().map(JsonNode::Number),
            Some(b't') => {
                self.expect_literal(b"true")?;
                Ok(JsonNode::Bool(true))
            }
            Some(b'f') => {
                self.expect_literal(b"false")?;
                Ok(JsonNode::Bool(false))
            }
            Some(b'n') => {
                self.expect_literal(b"null")?;
                Ok(JsonNode::Null)
            }
            _ => Err(json_parse_error("expected JSON value")),
        }
    }

    fn parse_object(&mut self) -> io::Result<JsonNode> {
        self.expect_byte(b'{')?;
        let mut object = BTreeMap::new();
        self.skip_ws();
        if self.consume_if(b'}') {
            return Ok(JsonNode::Object(object));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect_byte(b':')?;
            let value = self.parse_value()?;
            object.insert(key, value);
            self.skip_ws();
            if self.consume_if(b'}') {
                break;
            }
            self.expect_byte(b',')?;
        }
        Ok(JsonNode::Object(object))
    }

    fn parse_array(&mut self) -> io::Result<JsonNode> {
        self.expect_byte(b'[')?;
        let mut array = Vec::new();
        self.skip_ws();
        if self.consume_if(b']') {
            return Ok(JsonNode::Array(array));
        }
        loop {
            array.push(self.parse_value()?);
            self.skip_ws();
            if self.consume_if(b']') {
                break;
            }
            self.expect_byte(b',')?;
        }
        Ok(JsonNode::Array(array))
    }

    fn parse_string(&mut self) -> io::Result<String> {
        self.expect_byte(b'"')?;
        let mut output = String::new();
        let mut chunk_start = self.pos;
        loop {
            let Some(byte) = self.peek() else {
                return Err(json_parse_error("unterminated JSON string"));
            };
            match byte {
                b'"' => {
                    self.push_string_chunk(&mut output, chunk_start, self.pos)?;
                    self.pos += 1;
                    return Ok(output);
                }
                b'\\' => {
                    self.push_string_chunk(&mut output, chunk_start, self.pos)?;
                    self.pos += 1;
                    let Some(escaped) = self.next() else {
                        return Err(json_parse_error("unterminated JSON escape"));
                    };
                    match escaped {
                        b'"' => output.push('"'),
                        b'\\' => output.push('\\'),
                        b'/' => output.push('/'),
                        b'b' => output.push('\u{0008}'),
                        b'f' => output.push('\u{000c}'),
                        b'n' => output.push('\n'),
                        b'r' => output.push('\r'),
                        b't' => output.push('\t'),
                        b'u' => output.push(self.parse_unicode_escape()?),
                        _ => return Err(json_parse_error("invalid JSON escape")),
                    }
                    chunk_start = self.pos;
                }
                byte if byte < 0x20 => return Err(json_parse_error("control byte in JSON string")),
                _ => self.pos += 1,
            }
        }
    }

    fn push_string_chunk(&self, output: &mut String, start: usize, end: usize) -> io::Result<()> {
        let chunk = std::str::from_utf8(&self.text[start..end])
            .map_err(|_| json_parse_error("invalid UTF-8 in JSON string"))?;
        output.push_str(chunk);
        Ok(())
    }

    fn parse_unicode_escape(&mut self) -> io::Result<char> {
        let first = self.parse_unicode_code_unit()?;
        let codepoint = if (0xD800..=0xDBFF).contains(&first) {
            if self.next() != Some(b'\\') || self.next() != Some(b'u') {
                return Err(json_parse_error("missing JSON unicode low surrogate"));
            }
            let second = self.parse_unicode_code_unit()?;
            if !(0xDC00..=0xDFFF).contains(&second) {
                return Err(json_parse_error("invalid JSON unicode low surrogate"));
            }
            0x10000 + (((u32::from(first) - 0xD800) << 10) | (u32::from(second) - 0xDC00))
        } else if (0xDC00..=0xDFFF).contains(&first) {
            return Err(json_parse_error("unexpected JSON unicode low surrogate"));
        } else {
            u32::from(first)
        };
        char::from_u32(codepoint).ok_or_else(|| json_parse_error("invalid JSON unicode escape"))
    }

    fn parse_unicode_code_unit(&mut self) -> io::Result<u16> {
        if self.pos + 4 > self.text.len() {
            return Err(json_parse_error("short JSON unicode escape"));
        }
        let hex = std::str::from_utf8(&self.text[self.pos..self.pos + 4])
            .map_err(|_| json_parse_error("invalid JSON unicode escape"))?;
        let code_unit = u16::from_str_radix(hex, 16)
            .map_err(|_| json_parse_error("invalid JSON unicode escape"))?;
        self.pos += 4;
        Ok(code_unit)
    }

    fn parse_number(&mut self) -> io::Result<f64> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        self.consume_digits();
        if self.peek() == Some(b'.') {
            self.pos += 1;
            self.consume_digits();
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            self.consume_digits();
        }
        let raw = std::str::from_utf8(&self.text[start..self.pos])
            .map_err(|_| json_parse_error("invalid JSON number"))?;
        raw.parse::<f64>()
            .map_err(|_| json_parse_error("invalid JSON number"))
    }

    fn consume_digits(&mut self) {
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }

    fn expect_literal(&mut self, literal: &[u8]) -> io::Result<()> {
        if self.text.get(self.pos..self.pos + literal.len()) == Some(literal) {
            self.pos += literal.len();
            Ok(())
        } else {
            Err(json_parse_error("invalid JSON literal"))
        }
    }

    fn expect_byte(&mut self, expected: u8) -> io::Result<()> {
        match self.next() {
            Some(byte) if byte == expected => Ok(()),
            _ => Err(json_parse_error("unexpected JSON byte")),
        }
    }

    fn consume_if(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.text.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.pos += 1;
        Some(byte)
    }
}

fn json_parse_error(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_raw_utf8_and_unicode_surrogates() {
        let parsed = JsonParser::new(r#"{"name":"中文","emoji":"\uD83D\uDE00"}"#)
            .parse()
            .expect("parse unicode");
        let object = parsed.as_object().expect("object");
        assert_eq!(object.get("name").and_then(JsonNode::as_str), Some("中文"));
        assert_eq!(object.get("emoji").and_then(JsonNode::as_str), Some("😀"));
    }

    #[test]
    fn rejects_unpaired_unicode_surrogates() {
        assert!(JsonParser::new(r#""\uD83Dx""#).parse().is_err());
        assert!(JsonParser::new(r#""\uDE00""#).parse().is_err());
    }
}
