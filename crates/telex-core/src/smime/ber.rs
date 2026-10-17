//! BER made DER, so RustCrypto's `der` (strict DER) can read what mail programs write.
//!
//! CMS and PKCS #12 may be written in BER: Thunderbird (NSS) and Windows stream their output
//! with indefinite lengths (`30 80 … 00 00`) and cut long octet strings into chunks (a
//! constructed `24 80 04 … 04 … 00 00`). This rewrites such input with definite lengths and
//! each octet string in one piece. It reads at most [`MAX_DEPTH`] levels and [`MAX_NODES`]
//! elements, so hostile input can't run it out of stack or time.

/// How deep elements may nest (a certificate is about 10 levels inside CMS).
const MAX_DEPTH: usize = 40;
/// How many elements one input may have.
const MAX_NODES: usize = 200_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Invalid;

/// One element: its identifier octets (the tag, with the constructed bit) and what it holds.
#[derive(Debug)]
struct Node {
    tag: Vec<u8>,
    content: Content,
}

#[derive(Debug)]
enum Content {
    Primitive(Vec<u8>),
    Constructed(Vec<Node>),
}

/// `data` (BER or DER) as DER, as far as lengths and octet strings go. Input that is DER
/// already comes back the same.
pub fn to_der(data: &[u8]) -> Result<Vec<u8>, Invalid> {
    let mut nodes = 0;
    let (mut node, used) = parse(data, 0, &mut nodes)?;
    // Anything after the first element is ignored (a trailing newline in a file).
    let _ = used;
    fix(&mut node);
    let mut out = Vec::with_capacity(data.len());
    write(&node, &mut out);
    Ok(out)
}

fn parse(data: &[u8], depth: usize, nodes: &mut usize) -> Result<(Node, usize), Invalid> {
    if depth > MAX_DEPTH {
        return Err(Invalid);
    }
    *nodes += 1;
    if *nodes > MAX_NODES {
        return Err(Invalid);
    }
    let first = *data.first().ok_or(Invalid)?;
    let mut at = 1;
    if first & 0x1f == 0x1f {
        // A tag number over 30: base-128 octets follow, the last without the top bit.
        loop {
            let b = *data.get(at).ok_or(Invalid)?;
            at += 1;
            if at > 6 {
                return Err(Invalid);
            }
            if b & 0x80 == 0 {
                break;
            }
        }
    }
    let tag = data[..at].to_vec();
    let constructed = first & 0x20 != 0;
    let len_byte = *data.get(at).ok_or(Invalid)?;
    at += 1;
    if len_byte == 0x80 {
        // Indefinite: elements until an end-of-contents (00 00).
        if !constructed {
            return Err(Invalid);
        }
        let mut kids = Vec::new();
        loop {
            let rest = data.get(at..).ok_or(Invalid)?;
            if rest.len() >= 2 && rest[0] == 0 && rest[1] == 0 {
                at += 2;
                break;
            }
            let (kid, used) = parse(rest, depth + 1, nodes)?;
            kids.push(kid);
            at += used;
        }
        return Ok((
            Node {
                tag,
                content: Content::Constructed(kids),
            },
            at,
        ));
    }
    let len = if len_byte & 0x80 == 0 {
        usize::from(len_byte)
    } else {
        let count = usize::from(len_byte & 0x7f);
        if count == 0 || count > 4 {
            return Err(Invalid);
        }
        let mut len = 0usize;
        for _ in 0..count {
            len = (len << 8) | usize::from(*data.get(at).ok_or(Invalid)?);
            at += 1;
        }
        len
    };
    let end = at.checked_add(len).ok_or(Invalid)?;
    let body = data.get(at..end).ok_or(Invalid)?;
    if !constructed {
        return Ok((
            Node {
                tag,
                content: Content::Primitive(body.to_vec()),
            },
            end,
        ));
    }
    let mut kids = Vec::new();
    let mut i = 0;
    while i < body.len() {
        let (kid, used) = parse(&body[i..], depth + 1, nodes)?;
        kids.push(kid);
        i += used;
    }
    Ok((
        Node {
            tag,
            content: Content::Constructed(kids),
        },
        end,
    ))
}

/// The octets of a constructed string's pieces, in order.
fn pieces(kids: &[Node], out: &mut Vec<u8>) -> bool {
    for kid in kids {
        match &kid.content {
            Content::Primitive(b) if kid.tag == [0x04] => out.extend_from_slice(b),
            Content::Constructed(k) if kid.tag == [0x24] => {
                if !pieces(k, out) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

fn fix(node: &mut Node) {
    let Content::Constructed(kids) = &mut node.content else {
        return;
    };
    for kid in kids.iter_mut() {
        fix(kid);
    }
    // A constructed OCTET STRING: one primitive one.
    if node.tag == [0x24] {
        let mut joined = Vec::new();
        if pieces(kids, &mut joined) {
            node.tag = vec![0x04];
            node.content = Content::Primitive(joined);
        }
        return;
    }
    // EncryptedContentInfo (CMS's EnvelopedData, PKCS #12's EncryptedData): SEQUENCE { OID,
    // AlgorithmIdentifier, [0] IMPLICIT OCTET STRING }. Streamed, the [0] is constructed, its
    // octets in pieces: made primitive (80) here, as DER has it.
    if node.tag == [0x30]
        && kids.len() == 3
        && kids[0].tag == [0x06]
        && kids[1].tag == [0x30]
        && kids[2].tag == [0xa0]
    {
        if let Content::Constructed(parts) = &kids[2].content {
            let mut joined = Vec::new();
            if !parts.is_empty() && pieces(parts, &mut joined) {
                kids[2].tag = vec![0x80];
                kids[2].content = Content::Primitive(joined);
            }
        }
    }
}

fn write(node: &Node, out: &mut Vec<u8>) {
    out.extend_from_slice(&node.tag);
    match &node.content {
        Content::Primitive(b) => {
            length(b.len(), out);
            out.extend_from_slice(b);
        }
        Content::Constructed(kids) => {
            let mut inner = Vec::new();
            for kid in kids {
                write(kid, &mut inner);
            }
            length(inner.len(), out);
            out.extend_from_slice(&inner);
        }
    }
}

fn length(len: usize, out: &mut Vec<u8>) {
    if len < 0x80 {
        out.push(len as u8);
        return;
    }
    let bytes = len.to_be_bytes();
    let skip = bytes.iter().take_while(|&&b| b == 0).count();
    out.push(0x80 | (bytes.len() - skip) as u8);
    out.extend_from_slice(&bytes[skip..]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indefinite_lengths_and_chunked_strings_become_der() {
        // SEQUENCE (indefinite) { OCTET STRING in two chunks, INTEGER 5 }
        let ber = [
            0x30, 0x80, 0x24, 0x80, 0x04, 0x02, b'a', b'b', 0x04, 0x01, b'c', 0x00, 0x00, 0x02,
            0x01, 0x05, 0x00, 0x00,
        ];
        assert_eq!(
            to_der(&ber).unwrap(),
            [0x30, 0x08, 0x04, 0x03, b'a', b'b', b'c', 0x02, 0x01, 0x05]
        );
    }

    #[test]
    fn der_stays_as_it_is() {
        let der = [0x30, 0x06, 0x06, 0x01, 0x2a, 0x02, 0x01, 0x07];
        assert_eq!(to_der(&der).unwrap(), der);
        let long: Vec<u8> = [0x04, 0x82, 0x01, 0x00]
            .into_iter()
            .chain(std::iter::repeat_n(7, 256))
            .collect();
        assert_eq!(to_der(&long).unwrap(), long);
    }

    #[test]
    fn streamed_encrypted_content_is_made_primitive() {
        // SEQUENCE { OID, SEQUENCE {}, [0] (indefinite) { 04 01 x, 04 01 y } }
        let ber = [
            0x30, 0x80, 0x06, 0x01, 0x2a, 0x30, 0x00, 0xa0, 0x80, 0x04, 0x01, b'x', 0x04, 0x01,
            b'y', 0x00, 0x00, 0x00, 0x00,
        ];
        assert_eq!(
            to_der(&ber).unwrap(),
            [0x30, 0x09, 0x06, 0x01, 0x2a, 0x30, 0x00, 0x80, 0x02, b'x', b'y']
        );
    }

    #[test]
    fn hostile_input_is_refused_not_followed() {
        // Nested far too deep.
        let deep: Vec<u8> = std::iter::repeat_n([0x30u8, 0x80], 10_000)
            .flatten()
            .collect();
        assert_eq!(to_der(&deep), Err(Invalid));
        // A length past the end, and a five-byte length.
        assert_eq!(to_der(&[0x04, 0x05, 1, 2]), Err(Invalid));
        assert_eq!(to_der(&[0x04, 0x85, 1, 0, 0, 0, 0]), Err(Invalid));
        // An indefinite primitive, and no end-of-contents.
        assert_eq!(to_der(&[0x04, 0x80, 0, 0]), Err(Invalid));
        assert_eq!(to_der(&[0x30, 0x80, 0x02, 0x01, 0x01]), Err(Invalid));
        assert_eq!(to_der(&[]), Err(Invalid));
    }
}
