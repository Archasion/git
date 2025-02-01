use anyhow::Context;

const HEX_CHARS: &[u8] = b"0123456789abcdef";

/// Convert a binary slice to a hex slice.
pub(crate) fn encode_in_place(bytes: &mut Vec<u8>) {
    for _ in 0..bytes.len() {
        let byte = bytes.remove(0);
        bytes.push(HEX_CHARS[(byte >> 4) as usize]);
        bytes.push(HEX_CHARS[(byte & 0xf) as usize]);
    }
}

/// Convert a hex slice to a binary slice.
#[allow(dead_code)]
pub(crate) fn decode(hex: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(hex.len() / 2);

    if hex.len() & 1 != 0 {
        anyhow::bail!("invalid hex string");
    }

    for chunk in hex.chunks(2) {
        let high = (chunk[0] as char)
            .to_digit(16)
            .context("invalid hex character")?;
        let low = (chunk[1] as char)
            .to_digit(16)
            .context("invalid hex character")?;
        bytes.push(((high << 4) | low) as u8);
    }

    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use crate::utils::hex;

    #[test]
    fn hex_encode_in_place() {
        let mut binary = vec![0x00, 0x01, 0x02, 0x03];
        hex::encode_in_place(&mut binary);
        assert_eq!(binary, b"00010203");
    }

    #[test]
    fn hex_decode() {
        let hex = b"00010203";
        let binary = hex::decode(hex);
        assert!(binary.is_ok());
        assert_eq!(binary.unwrap(), vec![0x00, 0x01, 0x02, 0x03]);
    }
}
