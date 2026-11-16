//! Signing parity with `@solana/kit` (the official Jupiter CLI's signer). tests/vectors.json is
//! produced by tests/gen_vectors.mjs; never regenerate it to make a change pass.

use roshar_jupiter_perps::signer::Keypair;
use serde_json::Value;

fn vectors() -> Value {
    let path = format!("{}/tests/vectors.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn keypair(v: &Value) -> Keypair {
    let hex = v["seed_hex"].as_str().unwrap();
    let seed: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    Keypair::from_seed(&seed.try_into().unwrap())
}

#[test]
fn address_matches_kit() {
    let v = vectors();
    assert_eq!(keypair(&v).address(), v["address"].as_str().unwrap());
}

#[test]
fn signed_transactions_match_kit() {
    let v = vectors();
    let key = keypair(&v);
    let cases = v["cases"].as_array().unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let signed = key
            .sign_transaction_base64(case["unsigned_b64"].as_str().unwrap())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            signed.to_base64(),
            case["signed_b64"].as_str().unwrap(),
            "{name}"
        );
        assert_eq!(
            signed.signature_base58(),
            case["signature_b58"].as_str().unwrap(),
            "{name}"
        );
    }
}

#[test]
fn signing_an_already_signed_transaction_is_idempotent() {
    let v = vectors();
    let key = keypair(&v);
    let case = &v["cases"][0];
    let signed = key
        .sign_transaction_base64(case["signed_b64"].as_str().unwrap())
        .unwrap();
    assert_eq!(signed.to_base64(), case["signed_b64"].as_str().unwrap());
}

#[test]
fn non_signer_is_rejected_like_kit() {
    let v = vectors();
    let key = keypair(&v);
    let rejected = v["rejected"].as_array().unwrap();
    assert!(!rejected.is_empty());
    for case in rejected {
        let name = case["name"].as_str().unwrap();
        let result = key.sign_transaction_base64(case["unsigned_b64"].as_str().unwrap());
        assert!(
            result.is_err(),
            "{name}: signed a transaction the key is not a signer of"
        );
    }
}

#[test]
fn malformed_transactions_are_rejected() {
    let v = vectors();
    let key = keypair(&v);
    assert!(key.sign_transaction(&[]).is_err());
    assert!(key.sign_transaction(&[1]).is_err());
    assert!(key.sign_transaction(&[1; 65]).is_err());
    assert!(key.sign_transaction_base64("not base64!").is_err());
}

#[test]
fn keypair_bytes_round_trip() {
    let v = vectors();
    let key = keypair(&v);
    let hex = v["seed_hex"].as_str().unwrap();
    let mut bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    bytes.extend_from_slice(&key.pubkey());
    let b58 = bs58::encode(&bytes).into_string();
    assert_eq!(Keypair::from_base58(&b58).unwrap().address(), key.address());
    bytes[63] ^= 1;
    assert!(Keypair::from_keypair_bytes(&bytes).is_err());
}
