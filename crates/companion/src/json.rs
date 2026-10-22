//! Minimal JSON boundary for Herdr CLI output and detached-process state
//! files (port of `love/uml/adapters/json.lua`). Objects are string-keyed;
//! `null` is an explicit value. Numbers are doubles, as in the Lua codec.

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

impl Json {
    /// Integer view for PIDs and other whole-number fields.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Number(n) if n.fract() == 0.0 && n.is_finite() => Some(*n as i64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(map) => map.get(key),
            _ => None,
        }
    }

    pub fn array_items(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JSON byte {}: {}", self.offset + 1, self.message)
    }
}

impl std::error::Error for JsonError {}

struct Decoder<'a> {
    text: &'a [u8],
    position: usize,
}

impl<'a> Decoder<'a> {
    fn fail<T>(&self, message: &str) -> Result<T, JsonError> {
        Err(JsonError {
            offset: self.position,
            message: message.to_owned(),
        })
    }

    fn skip(&mut self) {
        while self.position < self.text.len()
            && matches!(self.text[self.position], b' ' | b'\t' | b'\r' | b'\n')
        {
            self.position += 1;
        }
    }

    fn at(&self, byte: u8) -> bool {
        self.position < self.text.len() && self.text[self.position] == byte
    }

    fn string_value(&mut self) -> Result<String, JsonError> {
        self.position += 1;
        let mut out: Vec<u8> = Vec::new();
        while self.position < self.text.len() {
            let char = self.text[self.position];
            match char {
                b'"' => {
                    self.position += 1;
                    // Input is validated ASCII-plus-UTF8 by construction:
                    // escapes emit valid UTF-8 and raw bytes pass through.
                    return String::from_utf8(out).map_err(|_| JsonError {
                        offset: self.position,
                        message: "invalid utf-8 in string".to_owned(),
                    });
                }
                b'\\' => {
                    let escape = *self.text.get(self.position + 1).unwrap_or(&0);
                    if escape == b'u' {
                        let hex = self.slice(self.position + 2, 4);
                        let code = u32::from_str_radix(hex, 16).ok().filter(|_| {
                            hex.len() == 4 && hex.bytes().all(|b| b.is_ascii_hexdigit())
                        });
                        let mut code = match code {
                            Some(code) => code,
                            None => return self.fail("bad unicode escape"),
                        };
                        self.position += 6;
                        if (0xD800..=0xDBFF).contains(&code) {
                            if self.slice(self.position, 2) != "\\u" {
                                return self.fail("missing low surrogate");
                            }
                            let low_hex = self.slice(self.position + 2, 4);
                            let low = u32::from_str_radix(low_hex, 16).ok().filter(|_| {
                                low_hex.len() == 4 && low_hex.bytes().all(|b| b.is_ascii_hexdigit())
                            });
                            let low = match low {
                                Some(low) => low,
                                None => return self.fail("bad low surrogate"),
                            };
                            if !(0xDC00..=0xDFFF).contains(&low) {
                                return self.fail("bad low surrogate");
                            }
                            code = 0x10000 + (code - 0xD800) * 1024 + (low - 0xDC00);
                            self.position += 6;
                        }
                        let mut buffer = [0u8; 4];
                        let encoded = char::from_u32(code)
                            .map(|c| c.encode_utf8(&mut buffer).as_bytes().to_vec());
                        match encoded {
                            Some(bytes) => out.extend_from_slice(&bytes),
                            None => return self.fail("bad unicode escape"),
                        }
                    } else {
                        let decoded = match escape {
                            b'"' => Some(b'"'),
                            b'\\' => Some(b'\\'),
                            b'/' => Some(b'/'),
                            b'b' => Some(0x08),
                            b'f' => Some(0x0C),
                            b'n' => Some(b'\n'),
                            b'r' => Some(b'\r'),
                            b't' => Some(b'\t'),
                            _ => None,
                        };
                        match decoded {
                            Some(byte) => {
                                out.push(byte);
                                self.position += 2;
                            }
                            None => return self.fail("bad escape"),
                        }
                    }
                }
                _ => {
                    if char < 32 {
                        return self.fail("control character in string");
                    }
                    out.push(char);
                    self.position += 1;
                }
            }
        }
        self.fail("unterminated string")
    }

    fn slice(&self, start: usize, length: usize) -> &'a str {
        self.text
            .get(start..start + length)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .unwrap_or("")
    }

    fn number_value(&mut self) -> Result<Json, JsonError> {
        let start = self.position;
        while self.position < self.text.len()
            && matches!(
                self.text[self.position],
                b'-' | b'+' | b'0'..=b'9' | b'.' | b'e' | b'E'
            )
        {
            self.position += 1;
        }
        let literal = &self.text[start..self.position];
        let literal = std::str::from_utf8(literal).unwrap_or("");
        // ^-?\d+\.?\d*[eE]?[-+]?\d*$
        let valid = !literal.is_empty() && {
            let bytes = literal.as_bytes();
            let mut i = usize::from(bytes[0] == b'-');
            let digits_start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let mut ok = i > digits_start;
            if i < bytes.len() && bytes[i] == b'.' {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if ok && i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
                i += 1;
                if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
                    i += 1;
                }
                let exp_start = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                // The Lua pattern allows an empty exponent; tonumber then
                // fails, so reject it here.
                ok = i > exp_start;
            }
            ok && i == bytes.len()
        };
        if !valid {
            return self.fail("bad number");
        }
        literal
            .parse::<f64>()
            .map(Json::Number)
            .map_err(|_| JsonError {
                offset: self.position,
                message: "bad number".to_owned(),
            })
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, JsonError> {
        if self.slice(self.position, word.len()) != word {
            return self.fail(&format!("expected {word}"));
        }
        self.position += word.len();
        Ok(value)
    }

    fn value(&mut self) -> Result<Json, JsonError> {
        self.skip();
        match self.text.get(self.position) {
            Some(b'{') => self.read_object(),
            Some(b'[') => self.read_array(),
            Some(b'"') => self.string_value().map(Json::String),
            Some(b'-') | Some(b'0'..=b'9') => self.number_value(),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            _ => self.fail("unexpected input"),
        }
    }

    fn read_array(&mut self) -> Result<Json, JsonError> {
        self.position += 1;
        let mut items = Vec::new();
        self.skip();
        if self.at(b']') {
            self.position += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.skip();
            if self.at(b',') {
                self.position += 1;
                self.skip();
            } else if self.at(b']') {
                self.position += 1;
                return Ok(Json::Array(items));
            } else {
                return self.fail("expected , or ]");
            }
        }
    }

    fn read_object(&mut self) -> Result<Json, JsonError> {
        self.position += 1;
        let mut object = BTreeMap::new();
        self.skip();
        if self.at(b'}') {
            self.position += 1;
            return Ok(Json::Object(object));
        }
        loop {
            self.skip();
            if !self.at(b'"') {
                return self.fail("expected object key");
            }
            let key = self.string_value()?;
            self.skip();
            if !self.at(b':') {
                return self.fail("expected :");
            }
            self.position += 1;
            object.insert(key, self.value()?);
            self.skip();
            if self.at(b',') {
                self.position += 1;
            } else if self.at(b'}') {
                self.position += 1;
                return Ok(Json::Object(object));
            } else {
                return self.fail("expected , or }");
            }
        }
    }
}

pub fn decode(text: &str) -> Result<Json, JsonError> {
    let mut decoder = Decoder {
        text: text.as_bytes(),
        position: 0,
    };
    let result = decoder.value()?;
    decoder.skip();
    if decoder.position < decoder.text.len() {
        return decoder.fail("trailing input");
    }
    Ok(result)
}

fn encode_string(value: &str, out: &mut String) {
    out.push('"');
    for char in value.chars() {
        match char {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 32 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

pub fn encode(value: &Json) -> String {
    let mut out = String::new();
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Number(n) => {
            if n.fract() == 0.0 && n.is_finite() && n.abs() < 9.0e15 {
                out.push_str(&format!("{n:.0}"));
            } else {
                out.push_str(&format!("{n}"));
            }
        }
        Json::String(s) => encode_string(s, &mut out),
        Json::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&encode(item));
            }
            out.push(']');
        }
        Json::Object(map) => {
            out.push('{');
            for (index, (key, item)) in map.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                encode_string(key, &mut out);
                out.push(':');
                out.push_str(&encode(item));
            }
            out.push('}');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_scalars_arrays_and_objects() {
        let value = decode(r#"{"a":[1,2.5,-3e2],"b":"x","c":true,"d":null}"#).unwrap();
        assert_eq!(
            Some(&Json::Number(1.0)),
            value
                .get("a")
                .and_then(|a| a.array_items())
                .and_then(|i| i.first())
        );
        assert_eq!(Some("x"), value.get("b").and_then(Json::as_str));
        assert_eq!(Some(true), value.get("c").and_then(Json::as_bool));
        assert_eq!(Some(&Json::Null), value.get("d"));
    }

    #[test]
    fn decodes_escapes_and_surrogate_pairs() {
        let value = decode(r#""\" \\ \b\f\n\r\t A 中""#).unwrap();
        assert_eq!(Some("\" \\ \u{8}\u{c}\n\r\t A 中"), value.as_str());
    }

    #[test]
    fn rejects_malformed_input_with_a_byte_position() {
        for bad in [
            "",
            "{",
            "not json",
            r#"{"a":01x}"#,
            r#"["a",]"#,
            "{\"a\":1} extra",
            "\"bad \\q escape\"",
            "\"unterminated",
        ] {
            assert!(decode(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn round_trips_and_distinguishes_empty_array_from_object() {
        let value = Json::Object(BTreeMap::from([
            ("list".to_owned(), Json::Array(vec![])),
            ("map".to_owned(), Json::Object(BTreeMap::new())),
        ]));
        let text = encode(&value);
        assert_eq!(r#"{"list":[],"map":{}}"#, text);
        assert_eq!(value, decode(&text).unwrap());
    }

    #[test]
    fn whole_numbers_have_an_integer_view() {
        assert_eq!(Some(42), decode("42").unwrap().as_i64());
        assert_eq!(Some(-3), decode("-3").unwrap().as_i64());
        assert_eq!(None, decode("2.5").unwrap().as_i64());
        assert_eq!(None, decode(r#""42""#).unwrap().as_i64());
    }
}
