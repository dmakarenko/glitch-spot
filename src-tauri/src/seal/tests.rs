// ABOUTME: Contract tests: Rust opens the Swift-sealed fixture and makes the pairing link that Swift parses.

mod ios_contract {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use hpke::kem::X25519HkdfSha256;
    use hpke::{Deserializable, Kem, Serializable};
    use serde_json::Value;

    use crate::seal::{self, pairing, SealError};
    use crate::snapshot::wire::Delivery;

    const CONTRACT: &str = include_str!("../../../tests/fixtures/ios-contract/contract.json");
    const KEY: &str = include_str!("../../../tests/fixtures/ios-contract/desktop-test-key.json");
    const INPUTS: &str = include_str!("../../../tests/fixtures/ios-contract/pairing-inputs.json");
    const LINK: &str = include_str!("../../../tests/fixtures/ios-contract/pairing-link.txt");
    const WIRE: &str = include_str!("../../../tests/fixtures/ios-contract/wire-snapshot.json");
    const SEALED: &[u8] =
        include_bytes!("../../../tests/fixtures/ios-contract/sealed-snapshot.bin");

    fn json(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    fn bytes32(fixture: &str, field: &str) -> [u8; 32] {
        let encoded = json(fixture)[field].as_str().unwrap().to_owned();
        URL_SAFE_NO_PAD.decode(encoded).unwrap().try_into().unwrap()
    }

    fn sender_id() -> String {
        json(INPUTS)["sender_id"].as_str().unwrap().to_owned()
    }

    fn open(sealed: &[u8]) -> Result<Vec<u8>, SealError> {
        seal::open(
            sealed,
            &bytes32(KEY, "private_key"),
            &sender_id(),
            &bytes32(INPUTS, "sender_secret"),
        )
    }

    #[test]
    fn constants_match_contract_json() {
        let contract = json(CONTRACT);
        assert_eq!(
            seal::INFO,
            contract["hpke"]["info"].as_str().unwrap().as_bytes()
        );
        assert_eq!(contract["hpke"]["aad"], "");
        assert_eq!(contract["sealed_body"]["version_byte"], seal::VERSION_BYTE);
        assert_eq!(
            contract["sealed_body"]["encapsulated_key_bytes"],
            seal::ENCAPSULATED_KEY_BYTES
        );
        assert_eq!(contract["sealed_body"]["tag_bytes"], seal::TAG_BYTES);
        assert_eq!(contract["sealed_body"]["max_bytes"], seal::MAX_SEALED_BYTES);
        assert_eq!(contract["payload"]["v"], crate::snapshot::wire::VERSION);
        assert_eq!(
            contract["payload"]["max_image_side"],
            crate::snapshot::wire::MAX_IMAGE_SIDE
        );
    }

    #[test]
    fn test_key_pair_is_consistent() {
        let private_key =
            <X25519HkdfSha256 as Kem>::PrivateKey::from_bytes(&bytes32(KEY, "private_key"))
                .unwrap();
        let public_key = X25519HkdfSha256::sk_to_pk(&private_key);
        assert_eq!(
            public_key.to_bytes().as_slice(),
            bytes32(KEY, "public_key").as_slice()
        );
    }

    #[test]
    fn makes_pinned_pairing_link() {
        let inputs = json(INPUTS);
        let link = pairing::link(
            inputs["receive_url"].as_str().unwrap(),
            &sender_id(),
            &bytes32(INPUTS, "sender_secret"),
            &bytes32(KEY, "public_key"),
        );
        assert_eq!(link, LINK);
    }

    #[test]
    fn pairing_link_percent_encodes_every_reserved_byte() {
        let link = pairing::link(
            "https://h.example/a b+c?d=e&f~g",
            "snd 1",
            &[0; 32],
            &[0; 32],
        );
        assert!(
            link.starts_with("glitchspot-pair://v1?url=https%3A%2F%2Fh.example%2Fa%20b%2Bc%3Fd%3De%26f~g&sender=snd%201&"),
            "{link}"
        );
    }

    #[test]
    fn opens_swift_sealed_fixture() {
        let opened = open(SEALED).expect("the Swift-sealed fixture must open");
        let payload: Value = serde_json::from_slice(&opened).unwrap();
        assert_eq!(
            payload,
            json(WIRE),
            "opened payload differs from wire-snapshot.json"
        );

        let delivery = Delivery::from_slice(&opened).unwrap();
        let expected = json(WIRE);
        assert_eq!(
            delivery.delivery_id.to_string().to_uppercase(),
            expected["delivery_id"]
        );
        let snapshot = &delivery.snapshot;
        let wire = &expected["snapshot"];
        assert_eq!(snapshot.created_at, wire["created_at"]);
        assert_eq!(snapshot.annotations.len(), 1);
        assert_eq!(snapshot.annotations[0].note, wire["annotations"][0]["note"]);
        assert_eq!(snapshot.annotations[0].coords.w, 5.0);
        assert_eq!(snapshot.redactions.len(), 1);
        assert_eq!(snapshot.context.screen.width, 8);
        assert_eq!(snapshot.context.screen.scale_factor, 3.0);
        assert_eq!(
            snapshot.context.os.platform,
            wire["context"]["os"]["platform"]
        );
        assert_eq!(
            snapshot.context.os.version,
            wire["context"]["os"]["version"]
        );
        assert_eq!(snapshot.context.app.name, wire["context"]["app"]["name"]);
        assert_eq!(
            snapshot.context.app.version,
            wire["context"]["app"]["version"]
        );
        assert_eq!(snapshot.context.app.build, wire["context"]["app"]["build"]);
        assert_eq!(
            Value::Object(snapshot.context.app.metadata.clone().unwrap()),
            wire["context"]["app"]["metadata"]
        );
        assert_eq!(
            snapshot.context.device.model,
            wire["context"]["device"]["model"]
        );
        assert_eq!(snapshot.images.clean_base64, wire["images"]["clean_base64"]);
        assert_eq!(
            snapshot.images.annotated_base64,
            wire["images"]["annotated_base64"]
        );
    }

    #[test]
    fn rejects_tampered_sealed_fixture() {
        for index in [1, 20, SEALED.len() / 2, SEALED.len() - 1] {
            let mut tampered = SEALED.to_vec();
            tampered[index] ^= 0x01;
            assert_eq!(
                open(&tampered),
                Err(SealError::Open),
                "flipped byte {index}"
            );
        }
    }

    #[test]
    fn rejects_wrong_key() {
        let (other, _) = X25519HkdfSha256::derive_keypair(b"another desktop key for this test");
        let other: [u8; 32] = other.to_bytes().into();
        let result = seal::open(
            SEALED,
            &other,
            &sender_id(),
            &bytes32(INPUTS, "sender_secret"),
        );
        assert_eq!(result, Err(SealError::Open));
    }

    #[test]
    fn rejects_wrong_psk() {
        let key = bytes32(KEY, "private_key");
        let secret = bytes32(INPUTS, "sender_secret");
        let mut wrong_secret = secret;
        wrong_secret[0] ^= 0x01;
        assert_eq!(
            seal::open(SEALED, &key, &sender_id(), &wrong_secret),
            Err(SealError::Open)
        );
        assert_eq!(
            seal::open(SEALED, &key, "snd_another", &secret),
            Err(SealError::Open)
        );
    }

    #[test]
    fn rejects_unknown_version_byte() {
        let mut other_version = SEALED.to_vec();
        other_version[0] = 0x02;
        assert_eq!(open(&other_version), Err(SealError::UnknownVersion(0x02)));
    }

    #[test]
    fn rejects_short_and_oversized_bodies() {
        assert_eq!(open(&[]), Err(SealError::TooShort));
        assert_eq!(open(&SEALED[..1 + 32 + 15]), Err(SealError::TooShort));
        let oversized = vec![seal::VERSION_BYTE; seal::MAX_SEALED_BYTES + 1];
        assert_eq!(
            open(&oversized),
            Err(SealError::TooLarge(seal::MAX_SEALED_BYTES + 1))
        );
    }
}
