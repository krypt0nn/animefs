use tinyrand::{Probability, Rand};

pub const STANDARD_CODE: CorrectionCode<3, 1> = CorrectionCode::new([
    // Check byte 1
    [
        [0b00010001, 0b00010001, 0b00010001],
        [0b00100010, 0b00100010, 0b00100010],
        [0b01000100, 0b01000100, 0b01000100],
        [0b10001000, 0b10001000, 0b10001000],
        [0b00010001, 0b00010001, 0b00010001],
        [0b00100010, 0b00100010, 0b00100010],
        [0b01000100, 0b01000100, 0b01000100],
        [0b10001000, 0b10001000, 0b10001000]
    ]
]);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CorrectionCode<const DATA_BYTES: usize, const CHECK_BYTES: usize> {
    connections: [[[u8; DATA_BYTES]; 8]; CHECK_BYTES]
}

impl<const DATA_BYTES: usize, const CHECK_BYTES: usize> CorrectionCode<DATA_BYTES, CHECK_BYTES> {
    /// Total number of bytes in the code.
    ///
    /// ```text
    /// [data bytes][check bytes]
    /// ```
    pub const TOTAL_BYTES: usize = DATA_BYTES + CHECK_BYTES;

    #[inline]
    /// Build new correction code from the given
    /// check connections map.
    ///
    /// ```text
    ///               Data bits
    ///            ┌─────────────┐
    ///            │ 1     1   1 │
    /// Check bits │    1     1  │
    ///            │  1     1    │
    ///            └─────────────┘
    /// ```
    pub const fn new(connections: [[[u8; DATA_BYTES]; 8]; CHECK_BYTES]) -> Self {
        Self {
            connections
        }
    }

    /// Form correction code of given size with standard pattern.
    pub fn standard() -> Self {
        let mut connections = [[[0; DATA_BYTES]; 8]; CHECK_BYTES];

        #[allow(clippy::needless_range_loop)]
        for check_bits in &mut connections {
            for i in 0..8 {
                for data_byte in &mut check_bits[i] {
                    *data_byte = (1 << (i % 4)) | (1 << (i % 4 + 4));
                }
            }
        }

        Self {
            connections
        }
    }

    /// Form correction code of given size with
    /// none of data bits connected to none of the
    /// check bits.
    ///
    /// Made for test purposes.
    pub fn zero() -> Self {
        Self {
            connections: [[[0; DATA_BYTES]; 8]; CHECK_BYTES]
        }
    }

    /// Form correction code of given size with
    /// all the data bits connected to all the
    /// check bits.
    ///
    /// Made for test purposes.
    pub fn full() -> Self {
        Self {
            connections: [[[u8::MAX; DATA_BYTES]; 8]; CHECK_BYTES]
        }
    }

    /// Create random correction code with given density.
    ///
    /// Density represents percent amount of check connections
    /// for each data bit. The higher the value, the more
    /// check connections there will be, and more difficult
    /// it will be to resolve the code. Lower values will
    /// lose data correction quality.
    pub fn random(density: impl Into<Probability>) -> Self {
        let mut rng = tinyrand::Wyrand::default();
        let density = density.into();

        let mut connections = [[[0; DATA_BYTES]; 8]; CHECK_BYTES];

        for check_bits in &mut connections {
            for data_bytes in check_bits {
                for data_byte in data_bytes {
                    for i in 0..8 {
                        if rng.next_bool(density) {
                            *data_byte |= 1 << i;
                        }
                    }
                }
            }
        }

        Self {
            connections
        }
    }

    #[allow(clippy::needless_range_loop)]
    pub fn encode_block(&self, data: &[u8; DATA_BYTES]) -> [u8; CHECK_BYTES] {
        let mut parity_bytes = [0; CHECK_BYTES];

        for data_byte in 0..DATA_BYTES {
            for data_bit_in_byte in 0..8 {
                let data_shift = 1 << data_bit_in_byte;

                for check_byte in 0..CHECK_BYTES {
                    for check_bit_in_byte in 0..8 {
                        let parity_shift = 1 << check_bit_in_byte;

                        // Check if the current data bit is connected with the current
                        // check (parity) bit.
                        if self.connections[check_byte][check_bit_in_byte][data_byte] & data_shift == data_shift {
                            // Check if the current data bit is 1.
                            if data[data_byte] & data_shift == data_shift {
                                // Flip the current parity bit.
                                parity_bytes[check_byte] ^= parity_shift;
                            }
                        }
                    }
                }
            }
        }

        parity_bytes
    }

    #[allow(clippy::needless_range_loop)]
    /// Return corrected data block, or none
    /// if couldn't correct the block.
    pub fn decode_block(&self, mut data: [u8; DATA_BYTES], mut check: [u8; CHECK_BYTES]) -> Option<[u8; DATA_BYTES]> {
        loop {
            let mut data_connections = [[0_isize; 8]; DATA_BYTES];
            let mut check_connections = [[0_isize; 8]; CHECK_BYTES];

            let mut has_error = false;

            for check_byte in 0..CHECK_BYTES {
                for check_bit_in_byte in 0..8 {
                    let mut data_parity = false;

                    for data_byte in 0..DATA_BYTES {
                        for data_bit_in_byte in 0..8 {
                            let data_shift = 1 << data_bit_in_byte;

                            // If current data bit is connected
                            // with the current check bit.
                            if self.connections[check_byte][check_bit_in_byte][data_byte] & data_shift == data_shift {
                                // Change the data parity if the data bit is 1.
                                if data[data_byte] & data_shift == data_shift {
                                    data_parity = !data_parity;
                                }
                            }
                        }
                    }

                    let check_shift = 1 << check_bit_in_byte;
                    let check_parity = check[check_byte] & check_shift == check_shift;

                    if check_parity == data_parity {
                        check_connections[check_byte][check_bit_in_byte] += 1;
                    } else {
                        check_connections[check_byte][check_bit_in_byte] -= 1;

                        has_error = true;
                    }

                    for data_byte in 0..DATA_BYTES {
                        for data_bit_in_byte in 0..8 {
                            let data_shift = 1 << data_bit_in_byte;

                            // If current data bit is connected
                            // with the current check bit.
                            if self.connections[check_byte][check_bit_in_byte][data_byte] & data_shift == data_shift {
                                if check_parity == data_parity {
                                    data_connections[data_byte][data_bit_in_byte] += 1;
                                } else {
                                    data_connections[data_byte][data_bit_in_byte] -= 1;
                                }
                            }
                        }
                    }
                }
            }

            // Return the data if no errors detected.
            if !has_error {
                dbg!(data_connections);

                return Some(data);
            }

            let mut most_negative_data_index = (0, 0);
            let mut most_negative_data_value = data_connections[0][0];

            let mut most_negative_check_index = (0, 0);
            let mut most_negative_check_value = check_connections[0][0];

            for data_byte in 0..DATA_BYTES {
                for data_bit_in_byte in 0..8 {
                    if data_connections[data_byte][data_bit_in_byte] < most_negative_data_value {
                        most_negative_data_index = (data_byte, data_bit_in_byte);
                        most_negative_data_value = data_connections[data_byte][data_bit_in_byte];
                    }
                }
            }

            for check_byte in 0..CHECK_BYTES {
                for check_bit_in_byte in 0..8 {
                    if check_connections[check_byte][check_bit_in_byte] < most_negative_check_value {
                        most_negative_check_index = (check_byte, check_bit_in_byte);
                        most_negative_check_value = check_connections[check_byte][check_bit_in_byte];
                    }
                }
            }

            // Can't correct the error if there's
            // not enough connections.
            if most_negative_data_value == 0 || most_negative_check_value == 0 {
                return None;
            }

            // There's higher chance that the error is in data
            // because there's more data bits than check bits.
            if most_negative_data_value <= most_negative_check_value {
                let data_shift = 1 << most_negative_data_index.1;

                // Flip the data bit.
                data[most_negative_data_index.0] ^= data_shift;

                dbg!(most_negative_data_index);
            }

            else {
                let check_shift = 1 << most_negative_check_index.1;

                // Flip the check bit.
                check[most_negative_check_index.0] ^= check_shift;

                dbg!(most_negative_check_index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_form() {
        assert_eq!(STANDARD_CODE, CorrectionCode::standard());
    }

    #[test]
    fn test_zero_code() {
        let zero_code = CorrectionCode::<1, 1>::zero();

        assert_eq!(zero_code.encode_block(&[u8::MIN]), [u8::MIN]);
        assert_eq!(zero_code.encode_block(&[u8::MAX]), [u8::MIN]);
        assert_eq!(zero_code.encode_block(&[0b10101010]), [u8::MIN]);
        assert_eq!(zero_code.encode_block(&[0b11111110]), [u8::MIN]);

        let zero_code = CorrectionCode::<2, 1>::zero();

        assert_eq!(zero_code.encode_block(&[u8::MIN, u8::MIN]), [u8::MIN]);
        assert_eq!(zero_code.encode_block(&[u8::MAX, u8::MAX]), [u8::MIN]);
        assert_eq!(zero_code.encode_block(&[0b10101010, 0b11001100]), [u8::MIN]);
        assert_eq!(zero_code.encode_block(&[0b11111110, 0b00000001]), [u8::MIN]);

        let zero_code = CorrectionCode::<1, 2>::zero();

        assert_eq!(zero_code.encode_block(&[u8::MIN]), [u8::MIN, u8::MIN]);
        assert_eq!(zero_code.encode_block(&[u8::MAX]), [u8::MIN, u8::MIN]);
        assert_eq!(zero_code.encode_block(&[0b10101010]), [u8::MIN, u8::MIN]);
        assert_eq!(zero_code.encode_block(&[0b11111110]), [u8::MIN, u8::MIN]);
    }

    #[test]
    fn test_full_code() {
        let full_code = CorrectionCode::<1, 1>::full();

        assert_eq!(full_code.encode_block(&[u8::MIN]), [u8::MIN]);
        assert_eq!(full_code.encode_block(&[u8::MAX]), [u8::MIN]);
        assert_eq!(full_code.encode_block(&[0b10101010]), [u8::MIN]);
        assert_eq!(full_code.encode_block(&[0b11111110]), [u8::MAX]);

        let full_code = CorrectionCode::<2, 1>::full();

        assert_eq!(full_code.encode_block(&[u8::MIN, u8::MIN]), [u8::MIN]);
        assert_eq!(full_code.encode_block(&[u8::MAX, u8::MAX]), [u8::MIN]);
        assert_eq!(full_code.encode_block(&[0b10101010, 0b11001100]), [u8::MIN]);
        assert_eq!(full_code.encode_block(&[0b11111110, 0b00000001]), [u8::MIN]);
        assert_eq!(full_code.encode_block(&[0b11111111, 0b00000001]), [u8::MAX]);

        let full_code = CorrectionCode::<1, 2>::full();

        assert_eq!(full_code.encode_block(&[u8::MIN]), [u8::MIN, u8::MIN]);
        assert_eq!(full_code.encode_block(&[u8::MAX]), [u8::MIN, u8::MIN]);
        assert_eq!(full_code.encode_block(&[0b10101010]), [u8::MIN, u8::MIN]);
        assert_eq!(full_code.encode_block(&[0b11111110]), [u8::MAX, u8::MAX]);
    }

    #[test]
    fn test_error_correction() {
        let code = CorrectionCode::<2, 1>::random(0.9);

        let tests = [
            [u8::MIN, u8::MIN],
            [u8::MAX, u8::MAX],
            [0b01010101, 0b11001100]
        ];

        for test in tests {
            let data = test;
            let check = code.encode_block(&data);

            assert_eq!(code.decode_block(data, check), Some(test));

            let data = [test[0] ^ 0b00010000, test[1]];
            let check = code.encode_block(&data);

            assert_eq!(code.decode_block(data, check), Some(test));

            let data = [test[0], test[1] ^ 0b01000000];
            let check = code.encode_block(&data);

            assert_eq!(code.decode_block(data, check), Some(test));

            let data = [test[0] ^ 0b00000010, test[1] ^ 0b01000000];
            let check = code.encode_block(&data);

            assert_eq!(code.decode_block(data, check), Some(test));
        }
    }
}
