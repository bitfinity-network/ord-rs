use bitcoin::key::{Keypair, Secp256k1};
use bitcoin::secp256k1::{All, SecretKey};
use bitcoin::XOnlyPublicKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaprootKeypair {
    /// Generate a keypair using a secret key
    SecretKey(SecretKey),
    /// Generate a keypair using a random number generator
    #[cfg(feature = "rand")]
    Random,
}

impl TaprootKeypair {
    pub fn generate_keypair(&self, secp: &Secp256k1<All>) -> (Keypair, XOnlyPublicKey) {
        let keypair = match self {
            Self::SecretKey(secret_key) => Keypair::from_secret_key(secp, secret_key),
            #[cfg(feature = "rand")]
            Self::Random => Keypair::new(secp, &mut rand::thread_rng()),
        };

        let x_public_key = XOnlyPublicKey::from_keypair(&keypair).0;
        (keypair, x_public_key)
    }
}
