use super::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// Wrapper structure for raw storage IOs that implements
/// read-write buffer of the first N bytes of this IO.
pub struct BufStorageIO<T> {
    io: T,
    buf: Vec<u8>,
    size: usize
}

impl<T: StorageIO> BufStorageIO<T> {
    /// Wrap given IO to buffer read/write operations.
    pub fn new(mut io: T, size: usize) -> Self {
        let len = io.len();

        let buf = if len >= size as u64 {
            io.read(0, size)
        } else {
            let mut buf = Vec::with_capacity(size);

            let file = io.read(0, len as usize);

            buf[..len as usize].copy_from_slice(&file);

            buf
        };

        Self {
            io,
            buf,
            size
        }
    }
}

impl<T> StorageIO for BufStorageIO<T> where T: StorageIO {
    type Reader = T::Reader;

    #[inline]
    fn io(&mut self) -> &mut Self::Reader {
        self.io.io()
    }

    // TODO: optimize integer conversions when usize = u64
    // #[cfg(target_pointer_width = "64")]

    fn read(&mut self, offset: u64, length: usize) -> Vec<u8> {
        if let Ok(offset) = usize::try_from(offset) {
            let n = self.buf.len();

            // [       ]
            //    ^ offset (within the buffer)
            if offset < n {
                if let Some(end) = offset.checked_add(length) {
                    // buf: [ ______   ]
                    //        ^    ^   ^ n
                    //        |    | end
                    //        | offset
                    if n >= end {
                        // Read the whole buffer if it's available.
                        return self.buf[offset..end].to_vec();
                    }

                    // buf: [ ______ ]
                    //        ^      ^    ^ end
                    //        |      | n
                    //        | offset
                    else {
                        let mut result = Vec::with_capacity(length);

                        // Read whole available buffer.
                        result.extend_from_slice(&self.buf[offset..]);

                        // Read remaining bytes from the IO.
                        result.extend(T::read(&mut self.io, n as u64, end - n));

                        return result;
                    }
                }
            }
        }

        // Read bytes directly from the IO.
        T::read(&mut self.io, offset, length)
    }

    fn write(&mut self, offset: u64, bytes: impl AsRef<[u8]>) {
        let bytes = bytes.as_ref();

        if let Ok(offset) = usize::try_from(offset) {
            // [       ]
            //    ^ offset (within the buffer)
            if offset < self.size {
                let mut n = self.buf.len();
                let m = bytes.len();

                if let Some(end) = offset.checked_add(m) {
                    // We can already fill buffer with all the bytes.
                    //
                    // buf: [ ______    ]
                    //        ^    ^    ^ n
                    //        |    | end
                    //        | offset
                    if n >= end {
                        self.buf[offset..end].copy_from_slice(bytes);
                    }

                    // buf: [ ______    ]
                    //        ^         ^       ^ end
                    //        |         | n
                    //        | offset
                    else {
                        // Fill buffer with zeros if offset overceeds it.
                        if offset > n {
                            self.buf.extend(vec![0; offset - n]);

                            n = offset;
                        }

                        let k = n - offset;

                        // Copy all the bytes that can be moved to the already allocated buffer.
                        if n > offset {
                            self.buf[offset..n].copy_from_slice(&bytes[..k]);
                        }

                        // If we can allocate more bytes for the buffer.
                        if self.size > n {
                            // Store all the bytes if they fit into the buffer.
                            if end <= self.size {
                                self.buf.extend_from_slice(&bytes[k..]);
                            }

                            // Otherwise store only part of them.
                            else {
                                self.buf.extend_from_slice(&bytes[k..self.size]);
                            }
                        }
                    }
                }
            }
        }

        T::write(&mut self.io, offset, bytes)
    }

    fn append(&mut self, bytes: impl AsRef<[u8]>) {
        let bytes = bytes.as_ref();

        let n = self.buf.len();

        if n < self.size {
            let m = bytes.len();
            let k = self.size - n;

            if k > m {
                self.buf.extend_from_slice(bytes);
            } else {
                self.buf.extend_from_slice(&bytes[..k]);
            }
        }

        T::append(&mut self.io, bytes)
    }

    #[inline]
    fn len(&mut self) -> u64 {
        if self.size != 0 && self.buf.is_empty() {
            return 0;
        }

        T::len(&mut self.io)
    }

    #[inline]
    fn is_empty(&mut self) -> bool {
        if self.size == 0 {
            T::is_empty(&mut self.io)
        } else {
            self.buf.is_empty()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use tinyrand::{Rand, Wyrand};

    use super::{StorageIO, BufStorageIO};

    fn with_io(name: &str, size: usize, callback: impl FnOnce(File, BufStorageIO<File>)) {
        let path_1 = std::env::temp_dir().join(format!(".animefs-buf-io-test-with-{name}"));
        let path_2 = std::env::temp_dir().join(format!(".animefs-buf-io-test-without-{name}"));

        let file_1 = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path_1)
            .expect("Failed to open file");

        let file_2 = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path_2)
            .expect("Failed to open file");

        callback(file_1, BufStorageIO::new(file_2, size));

        std::fs::remove_file(path_1).unwrap();
        std::fs::remove_file(path_2).unwrap();
    }

    fn test_write_buf(size: usize) {
        with_io(&format!("write{size}"), size, |mut file, mut buf| {
            assert_eq!(file.len(), 0);
            assert_eq!(buf.len(), 0);

            assert!(file.is_empty());
            assert!(buf.is_empty());

            let mut rand = Wyrand::default();

            for _ in 0..1000 {
                let offset = rand.next_lim_u64(256);
                let bytes = vec![rand.next_lim_u16(256) as u8; rand.next_lim_usize(256)];

                file.write(offset, &bytes);
                buf.write(offset, bytes);
            }

            assert!(!file.is_empty());
            assert!(!buf.is_empty());

            let len_1 = file.len() as usize;
            let len_2 = buf.len() as usize;

            assert_eq!(len_1, len_2);
            assert_eq!(file.read(0, len_1), buf.read(0, len_2));
        });
    }

    #[test]
    fn write0() {
        test_write_buf(0);
    }

    #[test]
    fn write128() {
        test_write_buf(128);
    }

    #[test]
    fn write1024() {
        test_write_buf(1024);
    }
}
