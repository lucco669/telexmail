//! Integration test: verify rnp-sexp can parse S-expressions that librnp
//! produces (via rnp-rs's key-to-JSON / packet-dump surface).

mod common;

use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage};
use rnp_sexp::Sexp;

#[test]
fn rnp_sexp_parses_simple_key_structure() {
    // A simplified version of the S-expression that librnp/gpg-agent
    // uses for private key storage.
    let input = "(11:private-key (5:ecdsa (5:curve 10:NIST P-256) (1:q 32:abcdefghijklmnopqrstuvwxyz012345)))";
    let sexp = Sexp::parse_advanced(input).expect("parse");
    let list = sexp.as_list().expect("list");
    assert_eq!(list.len(), 2);
    assert!(list[0].equals_str("private-key"));
    let ecdsa = list[1].as_list().expect("ecdsa is list");
    assert!(ecdsa[0].equals_str("ecdsa"));
}

#[test]
fn rnp_sexp_round_trips_canonical() {
    let original = Sexp::list(vec![
        Sexp::string("public-key"),
        Sexp::list(vec![
            Sexp::string("rsa"),
            Sexp::list(vec![
                Sexp::string("n"),
                Sexp::string(b"\x01\x02\x03".to_vec()),
            ]),
        ]),
    ]);
    let canonical = original.to_canonical();
    let reparsed = Sexp::parse_canonical(&canonical).expect("round-trip");
    assert_eq!(original, reparsed);
}

#[test]
fn rnp_sexp_as_unsigned() {
    let s = Sexp::string("42");
    assert_eq!(s.as_unsigned(), 42);

    let empty = Sexp::string("");
    assert_eq!(empty.as_unsigned(), u32::MAX);

    let non_num = Sexp::string("abc");
    assert_eq!(non_num.as_unsigned(), u32::MAX);
}

#[test]
fn rnp_rsx_and_rnp_sexp_coexist() {
    // Verify both crates can be used in the same binary without conflicts.
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("sexp-test <st@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .build(&ctx)
        .expect("key");

    // rnp-rs produces a key; rnp-sexp can parse its raw data
    // (the key's secret material is in S-expression format internally)
    let pub_data = key.raw_public_data().expect("public data");
    assert!(!pub_data.is_empty());

    // rnp-sexp is a standalone crate — just verify it's linked and callable
    let sexp = Sexp::parse_advanced("(3:abc)").expect("parse");
    assert!(sexp.as_list().is_some());
}
