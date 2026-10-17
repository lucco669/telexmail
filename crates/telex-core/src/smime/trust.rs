//! Whether a certificate chains to an authority the system trusts, for S/MIME signatures and
//! for which certificates mail may be encrypted to.
//!
//! On Windows, Windows decides (CryptoAPI's chain engine, the user's and the machine's
//! certificate stores, enterprise policy and distrusted certificates included). Elsewhere
//! rustls-webpki builds the chain to the system's authorities (/etc/ssl/certs and the like,
//! through rustls-native-certs); without any, a chain can't be confirmed. Nothing is fetched:
//! no issuer certificates from the addresses a certificate names (a sender could learn when
//! the mail is opened), no revocation lists, no root updates.

use std::time::Duration;

use rustls_pki_types::{CertificateDer, UnixTime};

/// What the system says about a certificate's chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    /// It chains to an authority the system trusts, for email protection.
    Trusted,
    /// No such chain: self-signed, an unknown authority, a broken chain, not for mail.
    Untrusted,
    /// This system has no authorities to check against.
    Unknown,
    /// The system knows it was revoked (from what it has already, nothing is fetched).
    Revoked,
}

/// Extended key usage "email protection" (1.3.6.1.5.5.7.3.4), DER contents.
const EMAIL_PROTECTION: &[u8] = &[0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x04];

#[cfg(test)]
thread_local! {
    /// Authorities for the tests instead of the system's.
    pub static TEST_ANCHORS: std::cell::RefCell<Option<Vec<Vec<u8>>>> =
        const { std::cell::RefCell::new(None) };
}

/// The chain of `leaf` (DER) at `at` (Unix seconds), with `others` (certificates that came
/// with it) as possible issuers.
pub fn chain(leaf: &[u8], others: &[Vec<u8>], at: i64) -> Trust {
    #[cfg(test)]
    if let Some(anchors) = TEST_ANCHORS.with(|a| a.borrow().clone()) {
        return with_anchors(leaf, others, &anchors, at);
    }
    system(leaf, others, at)
}

/// The chain checked by rustls-webpki against `anchors` (DER).
pub fn with_anchors(leaf: &[u8], others: &[Vec<u8>], anchors: &[Vec<u8>], at: i64) -> Trust {
    let anchor_ders: Vec<CertificateDer<'_>> = anchors
        .iter()
        .map(|a| CertificateDer::from(a.as_slice()))
        .collect();
    let anchors: Vec<_> = anchor_ders
        .iter()
        .filter_map(|a| webpki::anchor_from_trusted_cert(a).ok())
        .collect();
    if anchors.is_empty() {
        return Trust::Unknown;
    }
    let leaf = CertificateDer::from(leaf);
    let Ok(end) = webpki::EndEntityCert::try_from(&leaf) else {
        return Trust::Untrusted;
    };
    let intermediates: Vec<CertificateDer<'_>> = others
        .iter()
        .take(16)
        .map(|c| CertificateDer::from(c.as_slice()))
        .collect();
    let algs = [
        webpki::ring::ECDSA_P256_SHA256,
        webpki::ring::ECDSA_P256_SHA384,
        webpki::ring::ECDSA_P384_SHA256,
        webpki::ring::ECDSA_P384_SHA384,
        webpki::ring::RSA_PKCS1_2048_8192_SHA256,
        webpki::ring::RSA_PKCS1_2048_8192_SHA384,
        webpki::ring::RSA_PKCS1_2048_8192_SHA512,
        webpki::ring::RSA_PKCS1_2048_8192_SHA256_ABSENT_PARAMS,
        webpki::ring::RSA_PKCS1_2048_8192_SHA384_ABSENT_PARAMS,
        webpki::ring::RSA_PKCS1_2048_8192_SHA512_ABSENT_PARAMS,
        webpki::ring::RSA_PSS_2048_8192_SHA256_LEGACY_KEY,
        webpki::ring::RSA_PSS_2048_8192_SHA384_LEGACY_KEY,
        webpki::ring::RSA_PSS_2048_8192_SHA512_LEGACY_KEY,
        webpki::ring::ED25519,
    ];
    let time = UnixTime::since_unix_epoch(Duration::from_secs(at.max(0) as u64));
    match end.verify_for_usage(
        &algs,
        &anchors,
        &intermediates,
        time,
        webpki::KeyUsage::required_if_present(EMAIL_PROTECTION),
        None,
        None,
    ) {
        Ok(_) => Trust::Trusted,
        Err(webpki::Error::CertRevoked) => Trust::Revoked,
        Err(_) => Trust::Untrusted,
    }
}

#[cfg(not(windows))]
fn system(leaf: &[u8], others: &[Vec<u8>], at: i64) -> Trust {
    use std::sync::OnceLock;
    static ROOTS: OnceLock<Vec<Vec<u8>>> = OnceLock::new();
    let roots = ROOTS.get_or_init(|| {
        rustls_native_certs::load_native_certs()
            .certs
            .into_iter()
            .map(|c| c.as_ref().to_vec())
            .collect()
    });
    with_anchors(leaf, others, roots, at)
}

#[cfg(windows)]
fn system(leaf: &[u8], others: &[Vec<u8>], at: i64) -> Trust {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::Security::Cryptography::*;

    let encoding = X509_ASN_ENCODING | PKCS_7_ASN_ENCODING;
    let Ok(leaf_len) = u32::try_from(leaf.len()) else {
        return Trust::Untrusted;
    };
    // SAFETY: every pointer passed points into a buffer that outlives the call; every
    // context, store and chain made here is freed before returning, once.
    unsafe {
        let context = CertCreateCertificateContext(encoding, leaf.as_ptr(), leaf_len);
        if context.is_null() {
            return Trust::Untrusted;
        }
        let store = CertOpenStore(CERT_STORE_PROV_MEMORY, 0, 0, 0, null());
        if !store.is_null() {
            for other in others.iter().take(16) {
                if let Ok(len) = u32::try_from(other.len()) {
                    CertAddEncodedCertificateToStore(
                        store,
                        encoding,
                        other.as_ptr(),
                        len,
                        CERT_STORE_ADD_ALWAYS,
                        null_mut(),
                    );
                }
            }
        }
        let mut usage_oid = *b"1.3.6.1.5.5.7.3.4\0";
        let mut usage = usage_oid.as_mut_ptr();
        let para = CERT_CHAIN_PARA {
            cbSize: std::mem::size_of::<CERT_CHAIN_PARA>() as u32,
            RequestedUsage: CERT_USAGE_MATCH {
                dwType: USAGE_MATCH_TYPE_AND,
                Usage: CTL_USAGE {
                    cUsageIdentifier: 1,
                    rgpszUsageIdentifier: &mut usage,
                },
            },
            ..Default::default()
        };
        // FILETIME: 100-nanosecond steps since 1601.
        let ticks = (at.max(0) as u64 + 11_644_473_600) * 10_000_000;
        let time = FILETIME {
            dwLowDateTime: ticks as u32,
            dwHighDateTime: (ticks >> 32) as u32,
        };
        let mut chain: *mut CERT_CHAIN_CONTEXT = null_mut();
        let ok = CertGetCertificateChain(
            null_mut(),
            context,
            &time,
            store,
            &para,
            CERT_CHAIN_CACHE_ONLY_URL_RETRIEVAL | CERT_CHAIN_DISABLE_AUTH_ROOT_AUTO_UPDATE,
            null(),
            &mut chain,
        );
        let trust = if ok == 0 || chain.is_null() {
            Trust::Untrusted
        } else {
            let errors = (*chain).TrustStatus.dwErrorStatus;
            let broken = CERT_TRUST_IS_NOT_TIME_VALID
                | CERT_TRUST_IS_NOT_SIGNATURE_VALID
                | CERT_TRUST_IS_UNTRUSTED_ROOT
                | CERT_TRUST_IS_PARTIAL_CHAIN
                | CERT_TRUST_IS_NOT_VALID_FOR_USAGE
                | CERT_TRUST_IS_CYCLIC
                | CERT_TRUST_IS_EXPLICIT_DISTRUST
                | CERT_TRUST_INVALID_EXTENSION
                | CERT_TRUST_INVALID_POLICY_CONSTRAINTS
                | CERT_TRUST_INVALID_BASIC_CONSTRAINTS
                | CERT_TRUST_INVALID_NAME_CONSTRAINTS;
            if errors & CERT_TRUST_IS_REVOKED != 0 {
                Trust::Revoked
            } else if errors & broken != 0 {
                Trust::Untrusted
            } else {
                Trust::Trusted
            }
        };
        if !chain.is_null() {
            CertFreeCertificateChain(chain);
        }
        if !store.is_null() {
            CertCloseStore(store, 0);
        }
        CertFreeCertificateContext(context);
        trust
    }
}
