//! A software passkey: just enough of a WebAuthn authenticator and browser to answer the API's
//! challenges, as `navigator.credentials.*` and `PublicKeyCredential.toJSON()` would.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as BASE64URL};
use p256::ecdsa::{Signature, SigningKey, signature::Signer};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::ORIGIN;

pub struct Authenticator {
    key: SigningKey,
    pub credential_id: Vec<u8>,
    counter: u32,
}

/// A CBOR byte string header for `len` < 65536 bytes.
fn cbor_bytes(len: usize) -> Vec<u8> {
    match u8::try_from(len) {
        Ok(len) if len < 24 => vec![0x40 | len],
        Ok(len) => vec![0x58, len],
        Err(_) => [
            vec![0x59],
            u16::try_from(len).unwrap().to_be_bytes().to_vec(),
        ]
        .concat(),
    }
}

impl Authenticator {
    /// A passkey whose key and 16-byte credential id derive from `seed`.
    pub fn new(seed: u8) -> Self {
        Self {
            key: SigningKey::from_slice(&[seed; 32]).unwrap(),
            credential_id: vec![seed; 16],
            counter: 0,
        }
    }

    /// The same credential id with another key, as a forger would hold.
    pub fn forged(&self) -> Self {
        Self {
            key: SigningKey::from_slice(&[0xee; 32]).unwrap(),
            credential_id: self.credential_id.clone(),
            counter: self.counter,
        }
    }

    fn client_data(kind: &str, options: &Value) -> Vec<u8> {
        let challenge = &options["publicKey"]["challenge"];
        json!({ "type": kind, "challenge": challenge, "origin": ORIGIN, "crossOrigin": false })
            .to_string()
            .into_bytes()
    }

    fn authenticator_data(&self, rp_id: &str, flags: u8) -> Vec<u8> {
        [
            Sha256::digest(rp_id).to_vec(),
            vec![flags],
            self.counter.to_be_bytes().to_vec(),
        ]
        .concat()
    }

    fn credential(&self, response: &Value) -> Value {
        let id = BASE64URL.encode(&self.credential_id);
        json!({ "id": id, "rawId": id, "type": "public-key", "response": response, "clientExtensionResults": {} })
    }

    /// Creates the credential for a registration challenge, with no attestation.
    pub fn register(&self, options: &Value) -> Value {
        let rp_id = options["publicKey"]["rp"]["id"].as_str().unwrap();
        let point = self.key.verifying_key().to_sec1_point(false);
        // COSE_Key {1: 2 (EC2), 3: -7 (ES256), -1: 1 (P-256), -2: x, -3: y}
        let cose = [
            &[0xa5, 0x01, 0x02, 0x03, 0x26, 0x20, 0x01, 0x21, 0x58, 0x20][..],
            point.x().unwrap(),
            &[0x22, 0x58, 0x20],
            point.y().unwrap(),
        ]
        .concat();
        let attested = [
            vec![0; 16], // AAGUID
            u16::try_from(self.credential_id.len())
                .unwrap()
                .to_be_bytes()
                .to_vec(),
            self.credential_id.clone(),
            cose,
        ]
        .concat();
        // Flags: user present, user verified, attested credential data.
        let data = [self.authenticator_data(rp_id, 0x45), attested].concat();
        // {"fmt": "none", "attStmt": {}, "authData": data}
        let attestation = [
            &b"\xa3\x63fmt\x64none\x67attStmt\xa0\x68authData"[..],
            &cbor_bytes(data.len()),
            &data,
        ]
        .concat();
        self.credential(&json!({
            "attestationObject": BASE64URL.encode(attestation),
            "clientDataJSON": BASE64URL.encode(Self::client_data("webauthn.create", options)),
            "transports": ["internal"],
        }))
    }

    /// Signs an authentication challenge, counting the use.
    pub fn sign_in(&mut self, options: &Value) -> Value {
        self.counter += 1;
        let rp_id = options["publicKey"]["rpId"].as_str().unwrap();
        // Flags: user present, user verified.
        let data = self.authenticator_data(rp_id, 0x05);
        let client_data = Self::client_data("webauthn.get", options);
        let signed = [data.clone(), Sha256::digest(&client_data).to_vec()].concat();
        let signature: Signature = self.key.sign(&signed);
        self.credential(&json!({
            "authenticatorData": BASE64URL.encode(data),
            "clientDataJSON": BASE64URL.encode(client_data),
            "signature": BASE64URL.encode(signature.to_der().as_bytes()),
            "userHandle": null,
        }))
    }
}
