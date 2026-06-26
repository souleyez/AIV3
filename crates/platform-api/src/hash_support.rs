use sha2::{Digest, Sha256};

pub(crate) fn sha256_hex<const N: usize>(parts: [&[u8]; N]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    bytes_to_lower_hex(hasher.finalize().as_slice())
}

fn bytes_to_lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_hashes_single_part() {
        assert_eq!(
            sha256_hex([b"datamax".as_slice()]),
            "7951252e96ca812022b306ce9ed046aa5e6dd757d00c7878226628a5297a8c2d"
        );
    }

    #[test]
    fn sha256_hex_hashes_parts_in_order() {
        assert_eq!(
            sha256_hex([b"data".as_slice(), b":", b"max"]),
            sha256_hex([b"data:max".as_slice()])
        );
    }
}
