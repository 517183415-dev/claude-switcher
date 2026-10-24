use ed25519_dalek::{Signer, SigningKey};
use p0_protocol_fixture::{Envelope, PinnedKey, verify};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Body { value: u32 }

#[test]
fn exact_raw_bytes_are_signed_before_strict_payload_parsing() {
    let test_key = SigningKey::from_bytes(&[0x11; 32]);
    let pin = PinnedKey { id: "test-only", public: test_key.verifying_key() };
    let raw = "{\"value\":42}";
    let mut envelope = Envelope { payload: raw.into(), signature: test_key.sign(raw.as_bytes()).to_bytes().to_vec() };
    let wire = serde_json::to_vec(&envelope).unwrap();
    assert_eq!(verify::<Body>(&wire, &pin).unwrap().0.value, 42);
    envelope.payload = "{ \"value\":42}".into();
    assert_eq!(verify::<Body>(&serde_json::to_vec(&envelope).unwrap(), &pin).unwrap_err(), "signature");
    for bad in ["{\"value\":42,\"value\":43}", "{\"value\":42,\"command\":\"ignored\"}"] {
        envelope.payload = bad.into();
        envelope.signature = test_key.sign(bad.as_bytes()).to_bytes().to_vec();
        assert_eq!(verify::<Body>(&serde_json::to_vec(&envelope).unwrap(), &pin).unwrap_err(), "payload_json");
    }
}
