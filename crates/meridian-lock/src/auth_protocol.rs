use std::io::{self, Read};

pub const MAX_PASSWORD_BYTES: usize = 1024;

pub fn encode_password(password: &str) -> Option<Vec<u8>> {
    let bytes = password.as_bytes();
    if bytes.len() > MAX_PASSWORD_BYTES {
        return None;
    }

    let mut request = Vec::with_capacity(4 + bytes.len());
    request.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    request.extend_from_slice(bytes);
    Some(request)
}

pub fn read_password(mut reader: impl Read) -> io::Result<Vec<u8>> {
    let mut length = [0_u8; 4];
    reader.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length > MAX_PASSWORD_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "password request exceeds limit",
        ));
    }

    let mut password = vec![0_u8; length];
    reader.read_exact(&mut password)?;
    Ok(password)
}

#[cfg(test)]
mod tests {
    use super::{encode_password, read_password, MAX_PASSWORD_BYTES};

    #[test]
    fn password_request_round_trips_without_text_delimiters() {
        let request = encode_password("line one\nline two").expect("encode request");
        assert_eq!(
            read_password(request.as_slice()).unwrap(),
            b"line one\nline two"
        );
    }

    #[test]
    fn oversized_password_is_rejected_on_both_sides() {
        assert!(encode_password(&"x".repeat(MAX_PASSWORD_BYTES + 1)).is_none());

        let mut request = ((MAX_PASSWORD_BYTES + 1) as u32).to_be_bytes().to_vec();
        request.extend(std::iter::repeat_n(b'x', MAX_PASSWORD_BYTES + 1));
        assert!(read_password(request.as_slice()).is_err());
    }

    #[test]
    fn truncated_password_request_is_rejected() {
        let request = [0, 0, 0, 4, b'a', b'b'];
        assert!(read_password(request.as_slice()).is_err());
    }
}
