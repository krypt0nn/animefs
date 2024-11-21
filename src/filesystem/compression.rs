#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompressionLevel {
    Auto,
    Fast,
    Balanced,
    Max
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Compression {
    None,
    Lz4,
    Brotli,
    Zstd
}

impl Compression {
    pub fn compress(&self, data: impl AsRef<[u8]>, level: CompressionLevel) -> std::io::Result<Vec<u8>> {
        match self {
            Self::None => Ok(data.as_ref().to_vec()),

            Self::Lz4 => Ok(lz4_flex::compress(data.as_ref())),

            Self::Brotli => {
                let mut data = data.as_ref();
                let mut buf = Vec::with_capacity(data.len() >> 1);

                let mut params = brotli::enc::BrotliEncoderParams::default();

                match level {
                    CompressionLevel::Auto => (),

                    CompressionLevel::Fast => {
                        params.quality = 0;
                        params.favor_cpu_efficiency = true;
                    }

                    CompressionLevel::Balanced => {
                        params.quality = 6;
                    }

                    CompressionLevel::Max => {
                        params.quality = 11;
                        params.large_window = true;
                    }
                }

                brotli::BrotliCompress(&mut data, &mut buf, &params)?;

                Ok(buf)
            }

            Self::Zstd => {
                let max = zstd::compression_level_range()
                    .max()
                    .unwrap_or(zstd::DEFAULT_COMPRESSION_LEVEL);

                let min = zstd::compression_level_range()
                    .min()
                    .unwrap_or(0);

                let level = match level {
                    CompressionLevel::Auto     => zstd::DEFAULT_COMPRESSION_LEVEL,
                    CompressionLevel::Fast     => min,
                    CompressionLevel::Balanced => (max + min) / 2,
                    CompressionLevel::Max      => max
                };

                zstd::encode_all(data.as_ref(), level)
            }
        }
    }

    pub fn decompress(&self, data: impl AsRef<[u8]>) -> std::io::Result<Vec<u8>> {
        match self {
            Self::None => Ok(data.as_ref().to_vec()),

            Self::Lz4 => {
                let data = data.as_ref();

                lz4_flex::decompress(data, data.len())
                    .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
            }

            Self::Brotli => {
                let mut data = data.as_ref();
                let mut buf = Vec::with_capacity(data.len() << 1);

                brotli::BrotliDecompress(&mut data, &mut buf)?;

                Ok(buf)
            }

            Self::Zstd => zstd::decode_all(data.as_ref())
        }
    }
}
