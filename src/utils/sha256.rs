use bitcoin_hashes::{sha256, Hash};

/// Compute sha256 hash of bytes
pub fn sha256sum(bytes: &[u8]) -> Vec<u8> {
    sha256::Hash::hash(&bytes).to_byte_array().to_vec()
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_should_compute_sha256sum() {
        assert_eq!(
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9".as_bytes(),
            sha256sum("hello world".as_bytes())
        );
    }
}
