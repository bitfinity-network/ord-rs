use crate::Nft;

pub(crate) fn create_nft(content_type: &str, body: impl AsRef<[u8]>) -> Nft {
    Nft::new(Some(content_type.into()), Some(body.as_ref().into()))
}
