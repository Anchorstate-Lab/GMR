use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};
use std::io::{self, Write};

#[derive(Debug, thiserror::Error)]
pub enum CanonicalizeError {
    #[error("JSON structure exceeds maximum canonicalization depth ({max})")]
    DepthExceeded { max: usize },
    #[error("JSON numbers must be finite")]
    NonFiniteNumber,
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {type_name}: {reason}")]
pub struct NewtypeError {
    pub type_name: &'static str,
    pub reason: String,
}

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
    (@shared $name:ident, $validate:expr) => {
        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, $crate::addr::NewtypeError> {
                let s = value.into();
                let check: fn(&str) -> Result<(), String> = $validate;
                check(&s).map_err(|reason| $crate::addr::NewtypeError {
                    type_name: stringify!($name),
                    reason,
                })?;
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
            type Err = $crate::addr::NewtypeError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::try_new(s)
            }
        }
    };

    (admitted $(#[$doc:meta])* $name:ident, $validate:expr) => {
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
        }

        $crate::string_newtype!(@shared $name, $validate);
    };

    (minted $(#[$doc:meta])* $name:ident, $validate:expr) => {
        $(#[$doc])*
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, ::serde::Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(String);

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                let s = <String as ::serde::Deserialize>::deserialize(deserializer)?;
                Self::try_new(s).map_err(::serde::de::Error::custom)
            }
        }

        impl $name {
            pub(crate) fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn short(&self) -> &str {
                &self.0[..$crate::addr::SHORT]
            }
        }

        $crate::string_newtype!(@shared $name, $validate);
    };
}

pub const SHORT: usize = 12;

const MAX_CANONICAL_DEPTH: usize = 1024;

string_newtype! {
    minted ContentHash, check_sha256_hex
}

pub fn canonicalize(value: &Value) -> Result<Vec<u8>, CanonicalizeError> {
    let mut out = Vec::new();
    canonical_write(&mut out, value)?;
    Ok(out)
}

pub fn canonical_write<W: Write>(out: &mut W, value: &Value) -> Result<(), CanonicalizeError> {
    let mut canonicalizer = Canonicalizer::new(out);
    canonicalizer.write(value)
}

pub fn content_hash_of(value: &Value) -> Result<ContentHash, CanonicalizeError> {
    let mut hasher = Sha256::new();
    canonical_write(&mut hasher, value)?;
    let digest = hasher.finalize();
    Ok(ContentHash::new(format!("{digest:x}")))
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

    fn write(&mut self, value: &Value) -> Result<(), CanonicalizeError> {
        self.write_value(value)
    }

    fn write_value(&mut self, value: &Value) -> Result<(), CanonicalizeError> {
        if self.depth > MAX_CANONICAL_DEPTH {
            return Err(CanonicalizeError::DepthExceeded {
                max: MAX_CANONICAL_DEPTH,
            });
        }

        match value {
            Value::Object(map) => self.write_object(map),
            Value::Array(items) => self.write_array(items),
            Value::Number(number) => self.write_number(number),
            Value::String(_) | Value::Bool(_) | Value::Null => self.write_json_scalar(value),
        }
    }

    fn write_json_scalar(&mut self, value: &Value) -> Result<(), CanonicalizeError> {
        let bytes = serde_json::to_vec(value).expect("scalar serialisation cannot fail");
        self.out.write_all(&bytes)?;
        Ok(())
    }

    fn write_number(&mut self, number: &Number) -> Result<(), CanonicalizeError> {
        let text = Self::canonical_number_string(number)?;
        self.out.write_all(text.as_bytes())?;
        Ok(())
    }

    fn write_array(&mut self, items: &[Value]) -> Result<(), CanonicalizeError> {
        self.depth += 1;
        let result = (|| {
            self.out.write_all(b"[")?;
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    self.out.write_all(b",")?;
                }
                self.write_value(item)?;
            }
            self.out.write_all(b"]")?;
            Ok(())
        })();
        self.depth -= 1;
        result
    }

    fn write_object(&mut self, map: &Map<String, Value>) -> Result<(), CanonicalizeError> {
        self.depth += 1;
        let result = (|| {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            self.out.write_all(b"{")?;
            for (i, (k, v)) in entries.iter().enumerate() {
                if i > 0 {
                    self.out.write_all(b",")?;
                }
                let key_bytes =
                    serde_json::to_vec(k).expect("string key serialisation cannot fail");
                self.out.write_all(&key_bytes)?;
                self.out.write_all(b":")?;
                self.write_value(v)?;
            }
            self.out.write_all(b"}")?;
            Ok(())
        })();
        self.depth -= 1;
        result
    }

    fn canonical_number_string(number: &Number) -> Result<String, CanonicalizeError> {
        if number.is_i64() {
            return Ok(number.as_i64().unwrap().to_string());
        }

        if number.is_u64() {
            return Ok(number.as_u64().unwrap().to_string());
        }

        if let Some(f) = number.as_f64() {
            if !f.is_finite() {
                return Err(CanonicalizeError::NonFiniteNumber);
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

        unreachable!("serde_json::Number::as_f64 is total without arbitrary_precision")
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
        assert_eq!(canonicalize(&a).unwrap(), canonicalize(&b).unwrap());
    }

    #[test]
    fn nested_keys_sorted() {
        let value = json!({ "outer": {"z": 1, "a": 2}, "list": [{"y": 1, "x": 2}] });
        let rendered = String::from_utf8(canonicalize(&value).unwrap()).unwrap();
        assert_eq!(
            rendered,
            r#"{"list":[{"x":2,"y":1}],"outer":{"a":2,"z":1}}"#
        );
    }

    #[test]
    fn array_order_preserved() {
        assert_ne!(
            canonicalize(&json!([3, 1, 2])).unwrap(),
            canonicalize(&json!([1, 2, 3])).unwrap()
        );
    }

    #[test]
    fn whitespace_in_source_does_not_matter() {
        let a: Value = serde_json::from_str(r#"{ "a": 1, "b": 2 }"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap();
        assert_eq!(canonicalize(&a).unwrap(), canonicalize(&b).unwrap());
    }

    #[test]
    fn empty_object_and_array() {
        assert_eq!(canonicalize(&json!({})).unwrap(), b"{}");
        assert_eq!(canonicalize(&json!([])).unwrap(), b"[]");
    }

    #[test]
    fn content_hash_is_key_order_independent() {
        let a = json!({"x": 1, "y": [1, 2]});
        let b = json!({"y": [1, 2], "x": 1});
        assert_eq!(content_hash_of(&a).unwrap(), content_hash_of(&b).unwrap());
        assert_ne!(
            content_hash_of(&a).unwrap(),
            content_hash_of(&json!({"x": 2, "y": [1, 2]})).unwrap()
        );
    }

    #[test]
    fn content_hash_validates_as_sha256_hex() {
        let h = content_hash_of(&json!({"k": "v"})).unwrap();
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

    #[test]
    fn try_new_failure_names_the_newtype_it_came_from() {
        let err = ContentHash::try_new("not a hash").unwrap_err();
        assert_eq!(err.type_name, "ContentHash");
        assert!(err.reason.contains("64 hex chars"));
    }

    fn nested_array(depth: usize) -> Value {
        let mut v = json!([]);
        for _ in 0..depth {
            v = json!([v]);
        }
        v
    }

    #[test]
    fn depth_within_the_limit_still_canonicalizes() {
        assert!(canonicalize(&nested_array(MAX_CANONICAL_DEPTH)).is_ok());
    }

    #[test]
    fn depth_over_the_limit_is_a_typed_error_not_a_panic() {
        let value = nested_array(MAX_CANONICAL_DEPTH + 1);
        assert!(matches!(
            canonicalize(&value),
            Err(CanonicalizeError::DepthExceeded { max }) if max == MAX_CANONICAL_DEPTH
        ));
        assert!(matches!(
            content_hash_of(&value),
            Err(CanonicalizeError::DepthExceeded { .. })
        ));
    }

    #[test]
    fn canonical_form_is_pinned_against_library_drift() {
        let value = json!({"b": 1, "a": [1, 2.5, -0.0, "héllo"], "z": true, "n": null});
        let bytes = canonicalize(&value).unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            r#"{"a":[1,2.5,0,"héllo"],"b":1,"n":null,"z":true}"#
        );
        assert_eq!(
            content_hash_of(&value).unwrap().as_str(),
            "e33b884a2b4ae7c112a63950164e2b781a8ad08c6a5dea3c7e8848cfb32dcf25"
        );
    }

    fn refuses(json: &str) -> bool {
        serde_json::from_str::<ContentHash>(json).is_err()
            && serde_json::from_str::<crate::probe::ProbeVersion>(json).is_err()
            && serde_json::from_str::<crate::probe::FactAddress>(json).is_err()
    }

    #[test]
    fn a_minted_address_cannot_be_forged_through_the_wire() {
        assert!(refuses(r#""not-a-hash""#));
        assert!(refuses(r#""""#));
        assert!(refuses(&format!(r#""{}""#, "A".repeat(64))));
        assert!(refuses(&format!(r#""{}""#, "a".repeat(63))));

        let good = format!(r#""{}""#, "a".repeat(64));
        assert!(serde_json::from_str::<ContentHash>(&good).is_ok());
        assert!(serde_json::from_str::<crate::probe::ProbeVersion>(&good).is_ok());
        assert!(serde_json::from_str::<crate::probe::FactAddress>(&good).is_ok());
    }

    #[test]
    fn short_is_safe_because_the_type_is_the_one_guaranteeing_it() {
        let h = content_hash_of(&json!({})).unwrap();
        assert_eq!(h.short().len(), SHORT);
        assert!(h.as_str().starts_with(h.short()));
    }

    #[test]
    fn an_admitted_name_is_not_refused_on_the_way_back_out_of_the_store() {
        let long = format!(r#""{}""#, "k".repeat(400));
        assert!(
            serde_json::from_str::<crate::anchor::AnchorKey>(&long).is_ok(),
            "an admission limit belongs at the door a value comes in through, never at the \
             one it comes back out of: a journal is append-only, so a value already written \
             is a fact about the past. Refusing to read it back turns a limit somebody \
             tightened into a store nobody can open, and the entries behind the offending \
             one go with it"
        );
        assert!(
            crate::anchor::AnchorKey::try_new("k".repeat(400)).is_err(),
            "the same value must still be refused at the door — otherwise nothing enforces \
             the limit anywhere and the check is decorative"
        );
    }
}
