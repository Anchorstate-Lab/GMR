use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

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

string_newtype! {
    ContentHash, check_sha256_hex
}

pub fn canonicalize(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write_canonical(&mut out, value);
    out
}

pub fn content_hash_of(value: &Value) -> ContentHash {
    let digest = Sha256::digest(canonicalize(value));
    ContentHash::new(format!("{digest:x}"))
}

pub fn content_hash_of_bytes(bytes: &[u8]) -> ContentHash {
    let digest = Sha256::digest(bytes);
    ContentHash::new(format!("{digest:x}"))
}

fn write_canonical(out: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Object(map) => write_object(out, map),
        Value::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_canonical(out, item);
            }
            out.push(b']');
        }
        scalar => {
            let bytes = serde_json::to_vec(scalar).expect("scalar serialisation cannot fail");
            out.extend_from_slice(&bytes);
        }
    }
}

fn write_object(out: &mut Vec<u8>, map: &Map<String, Value>) {
    let mut entries: Vec<(&String, &Value)> = map.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    out.push(b'{');
    for (i, (k, v)) in entries.iter().enumerate() {
        if i > 0 {
            out.push(b',');
        }
        let key_bytes = serde_json::to_vec(k).expect("string key serialisation cannot fail");
        out.extend_from_slice(&key_bytes);
        out.push(b':');
        write_canonical(out, v);
    }
    out.push(b'}');
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
    fn sha256_hex_check_rejects_wrong_length_and_case() {
        assert!(check_sha256_hex(&"a".repeat(64)).is_ok());
        assert!(check_sha256_hex(&"a".repeat(63)).is_err());
        assert!(check_sha256_hex(&"A".repeat(64)).is_err());
        assert!(check_sha256_hex(&"g".repeat(64)).is_err());
    }
}
