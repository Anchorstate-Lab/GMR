use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};
use std::io::{self, Write};

pub fn check_sha256_hex(s: &str) -> Result<(), String> {
    if s.len() != 64 {
        return Err(format!("expected 64 hex chars, got {}", s.len()));
    }
    if !s
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("expected lowercase hex (0-9, a-f)".to_owned());
    }
    Ok(())
}

#[macro_export]
macro_rules! string_newtype {
    ($(#[$doc:meta])* $name:ident, $validate:expr) => {
        $(#[$doc])*
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash,
            ::serde::Serialize, ::serde::Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn try_new(value: impl Into<String>) -> Result<Self, String> {
                let s = value.into();
                let check: fn(&str) -> Result<(), String> = $validate;
                check(&s).map_err(|e| format!("invalid {}: {e}", stringify!($name)))?;
                Ok(Self(s))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = String;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::try_new(s)
            }
        }
    };
}

const MAX_CANONICAL_DEPTH: usize = 1024;

string_newtype! {
    ContentHash, check_sha256_hex
}

pub fn canonicalize(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    canonical_write(&mut out, value).expect("canonical_write on Vec cannot fail");
    out
}

pub fn canonical_write<W: Write>(out: &mut W, value: &Value) -> io::Result<()> {
    let mut canonicalizer = Canonicalizer::new(out);
    canonicalizer.write(value)
}

pub fn content_hash_of(value: &Value) -> ContentHash {
    let mut hasher = Sha256::new();
    canonical_write(&mut hasher, value).expect("canonical_write into hasher cannot fail");
    let digest = hasher.finalize();
    ContentHash::new(format!("{digest:x}"))
}

pub fn content_hash_of_bytes(bytes: &[u8]) -> ContentHash {
    let digest = Sha256::digest(bytes);
    ContentHash::new(format!("{digest:x}"))
}

struct Canonicalizer<'a, W: Write> {
    out: &'a mut W,
    depth: usize,
}

impl<'a, W: Write> Canonicalizer<'a, W> {
    fn new(out: &'a mut W) -> Self {
        Self { out, depth: 0 }
    }

    fn write(&mut self, value: &Value) -> io::Result<()> {
        self.write_value(value)
    }

    fn write_value(&mut self, value: &Value) -> io::Result<()> {
        if self.depth > MAX_CANONICAL_DEPTH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "JSON structure exceeds maximum canonicalization depth",
            ));
        }

        match value {
            Value::Object(map) => self.write_object(map),
            Value::Array(items) => self.write_array(items),
            Value::Number(number) => self.write_number(number),
            Value::String(_) | Value::Bool(_) | Value::Null => self.write_json_scalar(value),
        }
    }

    fn write_json_scalar(&mut self, value: &Value) -> io::Result<()> {
        let bytes = serde_json::to_vec(value).expect("scalar serialisation cannot fail");
        self.out.write_all(&bytes)
    }

    fn write_number(&mut self, number: &Number) -> io::Result<()> {
        let text = Self::canonical_number_string(number)?;
        self.out.write_all(text.as_bytes())
    }

    fn write_array(&mut self, items: &[Value]) -> io::Result<()> {
        self.depth += 1;
        let result = (|| {
            self.out.write_all(b"[")?;
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    self.out.write_all(b",")?;
                }
                self.write_value(item)?;
            }
            self.out.write_all(b"]")
        })();
        self.depth -= 1;
        result
    }

    fn write_object(&mut self, map: &Map<String, Value>) -> io::Result<()> {
        self.depth += 1;
        let result = (|| {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            self.out.write_all(b"{")?;
            for (i, (k, v)) in entries.iter().enumerate() {
                if i > 0 {
                    self.out.write_all(b",")?;
                }
                let key_bytes = serde_json::to_vec(k).expect("string key serialisation cannot fail");
                self.out.write_all(&key_bytes)?;
                self.out.write_all(b":")?;
                self.write_value(v)?;
            }
            self.out.write_all(b"}")
        })();
        self.depth -= 1;
        result
    }

    fn canonical_number_string(number: &Number) -> io::Result<String> {
        if number.is_i64() {
            return Ok(number.as_i64().unwrap().to_string());
        }

        if number.is_u64() {
            return Ok(number.as_u64().unwrap().to_string());
        }

        if let Some(f) = number.as_f64() {
            if !f.is_finite() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "JSON numbers must be finite",
                ));
            }

            let mut s = ryu::Buffer::new().format(f).to_owned();
            if s == "-0" || s == "-0.0" {
                return Ok("0".to_owned());
            }

            if s.contains('.') || s.contains('e') || s.contains('E') {
                if s.contains('.') {
                    while s.ends_with('0') {
                        s.pop();
                    }
                    if s.ends_with('.') {
                        s.pop();
                    }
                }
                if let Some(pos) = s.find('E') {
                    s.replace_range(pos..=pos, "e");
                }
            }

            return Ok(s);
        }

        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported JSON number type",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn key_order_does_not_affect_output() {
        let a = json!({"b": 1, "a": 2, "c": 3});
        let b = json!({"c": 3, "a": 2, "b": 1});
        assert_eq!(canonicalize(&a), canonicalize(&b));
    }

    #[test]
    fn nested_keys_sorted() {
        let value = json!({ "outer": {"z": 1, "a": 2}, "list": [{"y": 1, "x": 2}] });
        let rendered = String::from_utf8(canonicalize(&value)).unwrap();
        assert_eq!(
            rendered,
            r#"{"list":[{"x":2,"y":1}],"outer":{"a":2,"z":1}}"#
        );
    }

    #[test]
    fn array_order_preserved() {
        assert_ne!(
            canonicalize(&json!([3, 1, 2])),
            canonicalize(&json!([1, 2, 3]))
        );
    }

    #[test]
    fn whitespace_in_source_does_not_matter() {
        let a: Value = serde_json::from_str(r#"{ "a": 1, "b": 2 }"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap();
        assert_eq!(canonicalize(&a), canonicalize(&b));
    }

    #[test]
    fn empty_object_and_array() {
        assert_eq!(canonicalize(&json!({})), b"{}");
        assert_eq!(canonicalize(&json!([])), b"[]");
    }

    #[test]
    fn content_hash_is_key_order_independent() {
        let a = json!({"x": 1, "y": [1, 2]});
        let b = json!({"y": [1, 2], "x": 1});
        assert_eq!(content_hash_of(&a), content_hash_of(&b));
        assert_ne!(
            content_hash_of(&a),
            content_hash_of(&json!({"x": 2, "y": [1, 2]}))
        );
    }

    #[test]
    fn content_hash_validates_as_sha256_hex() {
        let h = content_hash_of(&json!({"k": "v"}));
        assert!(ContentHash::try_new(h.as_str()).is_ok());
    }

    #[test]
    fn canonical_write_supports_write_into_hasher() {
        let value = json!({"x": [1, 2, 3], "y": {"b": 2, "a": 1}});
        let mut hasher = Sha256::new();
        canonical_write(&mut hasher, &value).expect("canonical_write works");
        let digest = hasher.finalize();
        assert_eq!(digest.len(), 32);
    }

    #[test]
    fn number_formatting_is_normalized_for_jcs() {
        let b = json!({"n": 1.2300});
        let mut out = Vec::new();
        canonical_write(&mut out, &b).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), r#"{"n":1.23}"#);

        let mut out = Vec::new();
        canonical_write(&mut out, &json!(-0.0)).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "0");
    }

    #[test]
    fn sha256_hex_check_rejects_wrong_length_and_case() {
        assert!(check_sha256_hex(&"a".repeat(64)).is_ok());
        assert!(check_sha256_hex(&"a".repeat(63)).is_err());
        assert!(check_sha256_hex(&"A".repeat(64)).is_err());
        assert!(check_sha256_hex(&"g".repeat(64)).is_err());
    }
}
