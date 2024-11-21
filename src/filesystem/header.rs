use crate::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FilesystemHeader {
    /// Size in bytes of the page's body.
    /// Physical size of the page equals to
    /// page_size + page_header_size.
    pub page_size: u64,

    pub names_checksum: Checksum,
    pub names_compression: Compression,
    pub names_compression_level: CompressionLevel
}

impl FilesystemHeader {
    pub const LENGTH: usize = 10;

    pub const FLAG_NAMES_CHECKSUM_MASK: u16    = 0b00000000_00000011;
    pub const FLAG_NAMES_CHECKSUM_NONE: u16    = 0b00000000_00000000;
    pub const FLAG_NAMES_CHECKSUM_SEAHASH: u16 = 0b00000000_00000001;
    pub const FLAG_NAMES_CHECKSUM_SIPHASH: u16 = 0b00000000_00000010;
    pub const FLAG_NAMES_CHECKSUM_XXH3: u16    = 0b00000000_00000011;

    pub const FLAG_NAMES_COMPRESSION_MASK: u16   = 0b00000000_00001100;
    pub const FLAG_NAMES_COMPRESSION_NONE: u16   = 0b00000000_00000000;
    pub const FLAG_NAMES_COMPRESSION_LZ4: u16    = 0b00000000_00000100;
    pub const FLAG_NAMES_COMPRESSION_BROTLI: u16 = 0b00000000_00001000;
    pub const FLAG_NAMES_COMPRESSION_ZSTD: u16   = 0b00000000_00001100;

    pub const FLAG_NAMES_COMPRESSION_LEVEL_MASK: u16     = 0b00000000_00110000;
    pub const FLAG_NAMES_COMPRESSION_LEVEL_AUTO: u16     = 0b00000000_00000000;
    pub const FLAG_NAMES_COMPRESSION_LEVEL_FAST: u16     = 0b00000000_00010000;
    pub const FLAG_NAMES_COMPRESSION_LEVEL_BALANCED: u16 = 0b00000000_00100000;
    pub const FLAG_NAMES_COMPRESSION_LEVEL_MAX: u16      = 0b00000000_00110000;

    /// Parse filesystem header from the given bytes slice.
    pub fn from_bytes(bytes: &[u8; Self::LENGTH]) -> Self {
        let page_size = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5], bytes[6], bytes[7]
        ]);

        let flags = u16::from_le_bytes([bytes[8], bytes[9]]);

        let names_checksum = flags & Self::FLAG_NAMES_CHECKSUM_MASK;
        let names_compression = flags & Self::FLAG_NAMES_COMPRESSION_MASK;
        let names_compression_level = flags & Self::FLAG_NAMES_COMPRESSION_LEVEL_MASK;

        Self {
            page_size,

            names_checksum: match names_checksum {
                Self::FLAG_NAMES_CHECKSUM_NONE    => Checksum::None,
                Self::FLAG_NAMES_CHECKSUM_SEAHASH => Checksum::Seahash,
                Self::FLAG_NAMES_CHECKSUM_SIPHASH => Checksum::Siphash,
                Self::FLAG_NAMES_CHECKSUM_XXH3    => Checksum::Xxh3,

                _ => unreachable!()
            },

            names_compression: match names_compression {
                Self::FLAG_NAMES_COMPRESSION_NONE   => Compression::None,
                Self::FLAG_NAMES_COMPRESSION_LZ4    => Compression::Lz4,
                Self::FLAG_NAMES_COMPRESSION_BROTLI => Compression::Brotli,
                Self::FLAG_NAMES_COMPRESSION_ZSTD   => Compression::Zstd,

                _ => unreachable!()
            },

            names_compression_level: match names_compression_level {
                Self::FLAG_NAMES_COMPRESSION_LEVEL_AUTO     => CompressionLevel::Auto,
                Self::FLAG_NAMES_COMPRESSION_LEVEL_FAST     => CompressionLevel::Fast,
                Self::FLAG_NAMES_COMPRESSION_LEVEL_BALANCED => CompressionLevel::Balanced,
                Self::FLAG_NAMES_COMPRESSION_LEVEL_MAX      => CompressionLevel::Max,

                _ => unreachable!()
            }
        }
    }

    /// Encode filesystem header into the bytes slice.
    pub fn to_bytes(&self) -> [u8; Self::LENGTH] {
        let mut bytes = [0; Self::LENGTH];

        bytes[..8].copy_from_slice(&self.page_size.to_le_bytes());

        let mut flags = 0;

        match self.names_checksum {
            Checksum::None    => flags |= Self::FLAG_NAMES_CHECKSUM_NONE,
            Checksum::Seahash => flags |= Self::FLAG_NAMES_CHECKSUM_SEAHASH,
            Checksum::Siphash => flags |= Self::FLAG_NAMES_CHECKSUM_SIPHASH,
            Checksum::Xxh3    => flags |= Self::FLAG_NAMES_CHECKSUM_XXH3
        }

        match self.names_compression {
            Compression::None   => flags |= Self::FLAG_NAMES_COMPRESSION_NONE,
            Compression::Lz4    => flags |= Self::FLAG_NAMES_COMPRESSION_LZ4,
            Compression::Brotli => flags |= Self::FLAG_NAMES_COMPRESSION_BROTLI,
            Compression::Zstd   => flags |= Self::FLAG_NAMES_COMPRESSION_ZSTD
        }

        match self.names_compression_level {
            CompressionLevel::Auto     => flags |= Self::FLAG_NAMES_COMPRESSION_LEVEL_AUTO,
            CompressionLevel::Fast     => flags |= Self::FLAG_NAMES_COMPRESSION_LEVEL_FAST,
            CompressionLevel::Balanced => flags |= Self::FLAG_NAMES_COMPRESSION_LEVEL_BALANCED,
            CompressionLevel::Max      => flags |= Self::FLAG_NAMES_COMPRESSION_LEVEL_MAX
        }

        bytes[8..10].copy_from_slice(&flags.to_le_bytes());

        bytes
    }
}
