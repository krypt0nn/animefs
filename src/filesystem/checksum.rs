use std::hash::Hasher;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Checksum {
    Seahash,
    Siphash,
    Xxh3
}

impl Checksum {
    pub fn checksum(&self, data: impl AsRef<[u8]>) -> u64 {
        match self {
            Self::Seahash => {
                let mut hasher = seahash::SeaHasher::new();

                hasher.write(data.as_ref());

                hasher.finish()
            }

            Self::Siphash => {
                let hasher = siphasher::sip::SipHasher::new();

                hasher.hash(data.as_ref())
            }

            Self::Xxh3 => {
                let mut hasher = xxhash_rust::xxh3::Xxh3::new();

                hasher.write(data.as_ref());

                hasher.finish()
            }
        }
    }
}
