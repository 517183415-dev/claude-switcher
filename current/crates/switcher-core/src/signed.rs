//! Signed UTF-8 payloads. Verify the exact bytes before deserializing data.
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{
    Deserialize, Serialize,
    de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor},
};
use sha2::{Digest, Sha256};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub payload: String,
    pub signature: Vec<u8>,
}

pub fn digest(bytes: &[u8]) -> String {
    // ponytail: the standard byte formatter avoids a hex dependency.
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn verify<T: DeserializeOwned>(
    wire: &[u8],
    key: &VerifyingKey,
    limit: usize,
) -> Result<(T, String), &'static str> {
    if wire.len() > limit {
        return Err("envelope_too_large");
    }
    let envelope: Envelope = serde_json::from_slice(wire).map_err(|_| "envelope_json")?;
    let signature = Signature::from_slice(&envelope.signature).map_err(|_| "signature_shape")?;
    key.verify_strict(envelope.payload.as_bytes(), &signature)
        .map_err(|_| "signature")?;
    let data = strict_json(envelope.payload.as_bytes(), limit)?;
    Ok((data, digest(envelope.payload.as_bytes())))
}

/// All nested DTOs must also use `deny_unknown_fields`.
/// Parsing through this visitor rejects duplicate keys before serde can discard them.
pub fn strict_json<T: DeserializeOwned>(bytes: &[u8], limit: usize) -> Result<T, &'static str> {
    if bytes.len() > limit {
        return Err("payload_too_large");
    }
    let value: UniqueValue = serde_json::from_slice(bytes).map_err(|_| "payload_json")?;
    serde_json::from_value(value.0).map_err(|_| "payload_json")
}

struct UniqueValue(serde_json::Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueValue;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("JSON without duplicate keys")
            }
            fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(UniqueValue(value.into()))
            }
            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(UniqueValue(value.into()))
            }
            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(UniqueValue(value.into()))
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(|n| UniqueValue(n.into()))
                    .ok_or_else(|| E::custom("invalid number"))
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(UniqueValue(value.into()))
            }
            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(UniqueValue(value.into()))
            }
            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(serde_json::Value::Null))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(serde_json::Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = sequence.next_element::<UniqueValue>()? {
                    values.push(value.0);
                }
                Ok(UniqueValue(values.into()))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(de::Error::custom("duplicate key"));
                    }
                    values.insert(key, map.next_value::<UniqueValue>()?.0);
                }
                Ok(UniqueValue(values.into()))
            }
        }
        deserializer.deserialize_any(UniqueVisitor)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sequence {
    pub highest: u64,
    pub payload_hash: Option<String>,
}

impl Sequence {
    pub fn check(&self, sequence: u64, hash: &str) -> Result<(), &'static str> {
        if sequence == 0 || sequence < self.highest {
            return Err("old_sequence");
        }
        if sequence == self.highest && self.payload_hash.as_deref() != Some(hash) {
            return Err("sequence_conflict");
        }
        Ok(())
    }
    pub fn remember(&mut self, sequence: u64, hash: String) -> Result<(), &'static str> {
        self.check(sequence, &hash)?;
        self.highest = sequence;
        self.payload_hash = Some(hash);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct Data {
        message: String,
    }

    #[test]
    fn sha256_is_fixed_lowercase_hex_with_leading_zeroes() {
        assert_eq!(
            digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            digest(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn exact_payload_roundtrip_tampering_wrong_key_and_limit() {
        // Public, synthetic fixture. Never a release signing key.
        let signer = SigningKey::from_bytes(&[0x11; 32]);
        let payload = "{\"message\":\"本地数据\"}";
        let mut envelope = Envelope {
            payload: payload.into(),
            signature: signer.sign(payload.as_bytes()).to_bytes().to_vec(),
        };
        let wire = serde_json::to_vec(&envelope).unwrap();
        let (data, hash) = verify::<Data>(&wire, &signer.verifying_key(), 4096).unwrap();
        assert_eq!(data.message, "本地数据");
        assert_eq!(hash, digest(payload.as_bytes()));
        assert_eq!(
            verify::<Data>(&wire, &signer.verifying_key(), 1).unwrap_err(),
            "envelope_too_large"
        );
        let other = SigningKey::from_bytes(&[0x22; 32]);
        assert_eq!(
            verify::<Data>(&wire, &other.verifying_key(), 4096).unwrap_err(),
            "signature"
        );
        envelope.payload.push(' ');
        assert_eq!(
            verify::<Data>(
                &serde_json::to_vec(&envelope).unwrap(),
                &signer.verifying_key(),
                4096
            )
            .unwrap_err(),
            "signature"
        );
    }

    #[test]
    fn duplicate_nested_keys_unknown_fields_and_replay_are_rejected() {
        for input in [
            r#"{"a":{"x":1,"x":2}}"#,
            r#"{"a":[{"x":1,"x":2}]}"#,
            r#"{"a":1,"a":2}"#,
        ] {
            assert!(strict_json::<serde_json::Value>(input.as_bytes(), 4096).is_err());
        }
        assert!(strict_json::<Data>(br#"{"message":"ok","unknown":1}"#, 4096).is_err());
        assert!(strict_json::<Data>(br#"{"message":"ok"} {}"#, 4096).is_err());
        let mut sequence = Sequence::default();
        sequence.remember(2, "hash".into()).unwrap();
        assert!(sequence.check(1, "hash").is_err());
        assert!(sequence.check(2, "other").is_err());
        sequence.check(2, "hash").unwrap();
        assert!(sequence.remember(0, "zero".into()).is_err());
        assert_eq!(sequence.highest, 2);
    }
}
