use bitcoin_hashes::hash160;
use bitcoin_hashes::Hash;

pub fn h160sum(bytes: &[u8]) -> Vec<u8> {
    hash160::Hash::hash(&bytes).to_byte_array().to_vec()
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_should_compute_h160() {
        assert_eq!(
            "98c615784ccb5fe5936fbc0cbe9dfdb408d92f0f".as_bytes(),
            h160sum("hello world".as_bytes())
        );
    }
}
