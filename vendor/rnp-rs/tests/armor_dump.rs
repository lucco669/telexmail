//! Phase 7: ASCII armor / dearmor / packet dumps.

mod common;
use common::signing_key;

use rnp::{
    ArmorType, Context, DumpFlags, JsonDumpFlags, JsonFlags, armor_bytes, dearmor_bytes,
    dump_packets_bytes_to_json,
};

#[test]
fn armor_dearmor_roundtrip() {
    let original = b"\x00\x01\x02\x03 some binary \xff\xfe data";
    let armored = armor_bytes(original, ArmorType::Message).expect("armor");
    let s = String::from_utf8(armored.clone()).expect("ascii");
    assert!(s.starts_with("-----BEGIN PGP MESSAGE-----"));

    let back = dearmor_bytes(&armored).expect("dearmor");
    assert_eq!(back.as_slice(), original.as_slice());
}

#[test]
fn dump_packets_json_is_valid_json_object() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "dumper <dumper@example.com>");
    let exported = key.export(rnp::ExportFlags::PUBLIC).expect("export public");
    let json = dump_packets_bytes_to_json(&exported, JsonDumpFlags::default())
        .expect("dump_packets_to_json");
    // The C side returns a JSON array of packet objects. Confirm it parses.
    let parsed: serde_lite::Value = serde_lite::parse(&json).expect("valid json");
    assert!(parsed.is_array(), "expected array, got: {parsed:?}");
}

#[test]
fn key_to_json_contains_keyid() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "jsoner <jsoner@example.com>");
    let json = key.to_json(JsonFlags::default()).expect("to_json");
    assert!(
        json.contains("\"keyid\""),
        "key JSON should include the keyid field, got: {json}"
    );
}

#[test]
fn key_packets_to_json_returns_array() {
    let ctx = Context::new().expect("ctx");
    let key = signing_key(&ctx, "packets <packets@example.com>");
    let json = key
        .packets_to_json(false, JsonDumpFlags::default())
        .expect("packets_to_json");
    let _parsed: serde_lite::Value = serde_lite::parse(&json).expect("valid json");
}

// ------- a tiny JSON parser so the test suite doesn't pull serde_json -------
//
// The librnp JSON shape is defined upstream and may change between versions.
// We don't want to couple the public crate API to serde. But for assertions
// like "this is a JSON array" we need to parse — so a tiny scratch parser
// here is sufficient and stays test-only.
mod serde_lite {
    pub enum Value {
        Array(Vec<Value>),
        Object(Vec<(String, Value)>),
        Str(String),
        Num(f64),
        Bool(bool),
        Null,
    }

    impl Value {
        pub fn is_array(&self) -> bool {
            matches!(self, Value::Array(_))
        }
    }

    pub fn parse(s: &str) -> Result<Value, String> {
        let mut p = Parser {
            bytes: s.as_bytes(),
            pos: 0,
        };
        p.ws();
        let v = p.value()?;
        p.ws();
        if p.pos != p.bytes.len() {
            return Err(format!("trailing bytes at {}", p.pos));
        }
        Ok(v)
    }

    struct Parser<'a> {
        bytes: &'a [u8],
        pos: usize,
    }

    impl<'a> Parser<'a> {
        fn ws(&mut self) {
            while self.pos < self.bytes.len() && (self.bytes[self.pos] as char).is_whitespace() {
                self.pos += 1;
            }
        }

        fn value(&mut self) -> Result<Value, String> {
            self.ws();
            match self.bytes.get(self.pos).copied() {
                Some(b'{') => self.object(),
                Some(b'[') => self.array(),
                Some(b'"') => self.string().map(Value::Str),
                Some(b't') | Some(b'f') => self.boolean(),
                Some(b'n') => self.null(),
                Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
                _ => Err(format!("unexpected at {}", self.pos)),
            }
        }

        fn object(&mut self) -> Result<Value, String> {
            self.pos += 1; // {
            let mut items = Vec::new();
            self.ws();
            if self.bytes.get(self.pos) == Some(&b'}') {
                self.pos += 1;
                return Ok(Value::Object(items));
            }
            loop {
                self.ws();
                let k = self.string()?;
                self.ws();
                if self.bytes.get(self.pos) != Some(&b':') {
                    return Err("expected :".into());
                }
                self.pos += 1;
                let v = self.value()?;
                items.push((k, v));
                self.ws();
                match self.bytes.get(self.pos).copied() {
                    Some(b',') => {
                        self.pos += 1;
                    }
                    Some(b'}') => {
                        self.pos += 1;
                        return Ok(Value::Object(items));
                    }
                    _ => return Err("expected , or }".into()),
                }
            }
        }

        fn array(&mut self) -> Result<Value, String> {
            self.pos += 1; // [
            let mut items = Vec::new();
            self.ws();
            if self.bytes.get(self.pos) == Some(&b']') {
                self.pos += 1;
                return Ok(Value::Array(items));
            }
            loop {
                let v = self.value()?;
                items.push(v);
                self.ws();
                match self.bytes.get(self.pos).copied() {
                    Some(b',') => {
                        self.pos += 1;
                    }
                    Some(b']') => {
                        self.pos += 1;
                        return Ok(Value::Array(items));
                    }
                    _ => return Err("expected , or ]".into()),
                }
            }
        }

        fn string(&mut self) -> Result<String, String> {
            if self.bytes.get(self.pos) != Some(&b'"') {
                return Err("expected \"".into());
            }
            self.pos += 1;
            let mut s = String::new();
            while let Some(&c) = self.bytes.get(self.pos) {
                self.pos += 1;
                if c == b'"' {
                    return Ok(s);
                }
                if c == b'\\' {
                    let esc = self.bytes.get(self.pos).copied().unwrap_or(b'?');
                    self.pos += 1;
                    s.push(esc as char);
                    continue;
                }
                s.push(c as char);
            }
            Err("unterminated string".into())
        }

        fn boolean(&mut self) -> Result<Value, String> {
            if self.bytes[self.pos..].starts_with(b"true") {
                self.pos += 4;
                Ok(Value::Bool(true))
            } else if self.bytes[self.pos..].starts_with(b"false") {
                self.pos += 5;
                Ok(Value::Bool(false))
            } else {
                Err("bad bool".into())
            }
        }

        fn null(&mut self) -> Result<Value, String> {
            if self.bytes[self.pos..].starts_with(b"null") {
                self.pos += 4;
                Ok(Value::Null)
            } else {
                Err("bad null".into())
            }
        }

        fn number(&mut self) -> Result<Value, String> {
            let start = self.pos;
            while let Some(&c) = self.bytes.get(self.pos) {
                if c == b'-'
                    || c == b'+'
                    || c == b'.'
                    || c == b'e'
                    || c == b'E'
                    || c.is_ascii_digit()
                {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            let s = std::str::from_utf8(&self.bytes[start..self.pos]).map_err(|e| e.to_string())?;
            s.parse::<f64>().map(Value::Num).map_err(|e| e.to_string())
        }
    }

    impl std::fmt::Debug for Value {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Value::Array(v) => f.debug_list().entries(v.iter()).finish(),
                Value::Object(o) => f
                    .debug_map()
                    .entries(o.iter().map(|(k, v)| (k, v)))
                    .finish(),
                Value::Str(s) => write!(f, "{s:?}"),
                Value::Num(n) => write!(f, "{n}"),
                Value::Bool(b) => write!(f, "{b}"),
                Value::Null => write!(f, "null"),
            }
        }
    }
}

// Silence unused warning when the test only uses some functions.
#[allow(dead_code)]
fn _refs() {
    let _ = (DumpFlags::MPI.bits(), JsonDumpFlags::MPI.bits());
}
