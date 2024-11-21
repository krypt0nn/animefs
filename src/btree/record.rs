#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GenericBTreeRecord<const KEY_SIZE: usize, const VALUE_SIZE: usize> {
    pub key: Option<[u8; KEY_SIZE]>,
    pub value: Option<[u8; VALUE_SIZE]>,
    pub left_addr: Option<u32>,
    pub right_addr: Option<u32>
}

impl<const KEY_SIZE: usize, const VALUE_SIZE: usize> GenericBTreeRecord<KEY_SIZE, VALUE_SIZE> {
    pub const FLAG_LEFT_ADDR_SET: u8  = 0b0000_1000;
    pub const FLAG_KEY_SET: u8        = 0b0000_0010;
    pub const FLAG_VALUE_SET: u8      = 0b0000_0100;
    pub const FLAG_RIGHT_ADDR_SET: u8 = 0b0000_0001;

    pub const RECORD_SIZE: usize = KEY_SIZE + VALUE_SIZE + 9;

    pub const LEFT_ADDR_OFFSET: usize  = 0;
    pub const FLAG_OFFSET: usize       = Self::LEFT_ADDR_OFFSET + 4;
    pub const KEY_OFFSET: usize        = Self::FLAG_OFFSET + 1;
    pub const VALUE_OFFSET: usize      = Self::KEY_OFFSET + KEY_SIZE;
    pub const RIGHT_ADDR_OFFSET: usize = Self::VALUE_OFFSET + VALUE_SIZE;

    #[inline]
    pub const fn new(key: [u8; KEY_SIZE], value: [u8; VALUE_SIZE]) -> Self {
        Self {
            key: Some(key),
            value: Some(value),
            left_addr: None,
            right_addr: None
        }
    }

    /// Try to read generic B-Tree record from the bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<(Self, &[u8])> {
        if bytes.len() < Self::RECORD_SIZE {
            return None;
        }

        let flags = bytes[Self::FLAG_OFFSET];

        let mut key = None;
        let mut value = None;
        let mut left_addr = None;
        let mut right_addr = None;

        if flags & Self::FLAG_KEY_SET == Self::FLAG_KEY_SET {
            let mut raw_key = [0; KEY_SIZE];

            raw_key.copy_from_slice(&bytes[Self::KEY_OFFSET..Self::VALUE_OFFSET]);

            key = Some(raw_key);
        }

        if flags & Self::FLAG_VALUE_SET == Self::FLAG_VALUE_SET {
            let mut raw_value = [0; VALUE_SIZE];

            raw_value.copy_from_slice(&bytes[Self::VALUE_OFFSET..Self::RIGHT_ADDR_OFFSET]);

            value = Some(raw_value);
        }

        if flags & Self::FLAG_LEFT_ADDR_SET == Self::FLAG_LEFT_ADDR_SET {
            let mut raw_left_addr = [0; 4];

            raw_left_addr.copy_from_slice(&bytes[Self::LEFT_ADDR_OFFSET..Self::KEY_OFFSET]);

            left_addr = Some(u32::from_be_bytes(raw_left_addr));
        }

        if flags & Self::FLAG_RIGHT_ADDR_SET == Self::FLAG_RIGHT_ADDR_SET {
            let mut raw_right_addr = [0; 4];

            raw_right_addr.copy_from_slice(&bytes[Self::RIGHT_ADDR_OFFSET..Self::RECORD_SIZE]);

            right_addr = Some(u32::from_be_bytes(raw_right_addr));
        }

        let record = Self {
            key,
            value,
            left_addr,
            right_addr
        };

        if bytes.len() >= Self::RECORD_SIZE - 4 {
            Some((record, &bytes[Self::RECORD_SIZE - 4..]))
        } else {
            Some((record, &[]))
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut record = vec![0; Self::RECORD_SIZE];

        if let Some(key) = &self.key {
            record[Self::FLAG_OFFSET] |= Self::FLAG_KEY_SET;

            record[Self::KEY_OFFSET..Self::VALUE_OFFSET].copy_from_slice(key);
        }

        if let Some(value) = &self.value {
            record[Self::FLAG_OFFSET] |= Self::FLAG_VALUE_SET;

            record[Self::VALUE_OFFSET..Self::RIGHT_ADDR_OFFSET].copy_from_slice(value);
        }

        if let Some(left_addr) = &self.left_addr {
            record[Self::FLAG_OFFSET] |= Self::FLAG_LEFT_ADDR_SET;

            record[Self::LEFT_ADDR_OFFSET..Self::KEY_OFFSET].copy_from_slice(&left_addr.to_be_bytes());
        }

        if let Some(right_addr) = &self.right_addr {
            record[Self::FLAG_OFFSET] |= Self::FLAG_RIGHT_ADDR_SET;

            record[Self::RIGHT_ADDR_OFFSET..Self::RECORD_SIZE].copy_from_slice(&right_addr.to_be_bytes());
        }

        record
    }
}
