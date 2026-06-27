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
        while let Some(byte) = self.next() {
            match byte {
                b'"' => return Ok(output),
                b'\\' => {
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
                        b'u' => {
                            if self.pos + 4 > self.text.len() {
                                return Err(json_parse_error("short JSON unicode escape"));
                            }
                            let hex = std::str::from_utf8(&self.text[self.pos..self.pos + 4])
                                .map_err(|_| json_parse_error("invalid JSON unicode escape"))?;
                            let codepoint = u16::from_str_radix(hex, 16)
                                .map_err(|_| json_parse_error("invalid JSON unicode escape"))?;
                            self.pos += 4;
                            if let Some(ch) = char::from_u32(u32::from(codepoint)) {
                                output.push(ch);
                            }
                        }
                        _ => return Err(json_parse_error("invalid JSON escape")),
                    }
                }
                byte if byte < 0x20 => return Err(json_parse_error("control byte in JSON string")),
                _ => output.push(byte as char),
            }
        }
        Err(json_parse_error("unterminated JSON string"))
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
