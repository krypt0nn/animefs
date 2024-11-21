use std::io::{Read, Write, Seek, SeekFrom};
use std::fs::File;

pub trait FilesystemIo<T: Read + Write + Seek> {
    /// Get mutable reference to the filesystem IO interface.
    fn io(&mut self) -> &mut T;

    /// Low level direct read operation. Returns bytes vector
    /// with exactly requested amount of bytes. If reader is
    /// empty, zeros are returned.
    fn read(&mut self, offset: u64, length: usize) -> Vec<u8> {
        let mut buf = vec![0; length];

        let io = self.io();

        if io.seek(SeekFrom::Start(offset)).is_err() {
            return buf;
        }

        let _ = io.read_exact(&mut buf);

        buf
    }

    /// Low level direct write operation. Fills file with zeros
    /// if there's no content before given offset.
    fn write(&mut self, offset: u64, bytes: impl AsRef<[u8]>) {
        let io = self.io();

        let reader_offset = io.seek(SeekFrom::Start(offset))
            .unwrap_or_else(|err| {
                panic!(
                    "Failed to write {} at offset 0x{offset:08x} : seek failed : {err}",
                    std::any::type_name::<T>()
                );
            });

        if reader_offset < offset {
            // Potentially unsafe
            io.write_all(&vec![0; (offset - reader_offset) as usize])
                .unwrap_or_else(|err| {
                    panic!(
                        "Failed to write {} at offset 0x{reader_offset:08x} : write failed : {err}",
                        std::any::type_name::<T>()
                    );
                });
        }

        io.write_all(bytes.as_ref())
            .unwrap_or_else(|err| {
                panic!(
                    "Failed to write {} at offset 0x{offset:08x} : write failed : {err}",
                    std::any::type_name::<T>()
                );
            });

        io.flush().unwrap_or_else(|err| {
            panic!(
                "Failed to write {} at offset 0x{offset:08x} : flush failed : {err}",
                std::any::type_name::<T>()
            );
        });
    }

    /// Append bytes slice to the end of the IO.
    fn append(&mut self, bytes: impl AsRef<[u8]>) {
        let io = self.io();

        io.seek(SeekFrom::End(0))
            .unwrap_or_else(|err| {
                panic!(
                    "Failed to append {} : seek failed : {err}",
                    std::any::type_name::<T>()
                );
            });

        io.write_all(bytes.as_ref())
            .unwrap_or_else(|err| {
                panic!(
                    "Failed to append {} : write failed : {err}",
                    std::any::type_name::<T>()
                );
            });

        io.flush().unwrap_or_else(|err| {
            panic!(
                "Failed to append {} : flush failed : {err}",
                std::any::type_name::<T>()
            );
        });
    }

    /// Get length of the buffer.
    fn len(&mut self) -> u64 {
        self.io().seek(SeekFrom::End(0)).unwrap_or_else(|err| {
            panic!(
                "Failed to get len of {} : seek failed : {err}",
                std::any::type_name::<T>()
            );
        })
    }

    #[inline]
    /// Check if the buffer is empty.
    fn is_empty(&mut self) -> bool {
        self.len() == 0
    }
}

impl FilesystemIo<File> for File {
    #[inline]
    fn io(&mut self) -> &mut File {
        self
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use super::FilesystemIo;

    fn get_io(name: &str) -> (File, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(".animefs-io-test-{name}"));

        let fs = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .expect("Failed to open file");

        (fs, path)
    }

    #[test]
    fn read() {
        let (mut io, path) = get_io("read");

        assert!(path.metadata().unwrap().len() == 0);

        assert_eq!(io.read(0, 4), &[0, 0, 0, 0]);
        assert_eq!(io.read(8, 4), &[0, 0, 0, 0]);
    }

    #[test]
    fn write() {
        let (mut io, path) = get_io("write");

        assert!(path.metadata().unwrap().len() == 0);

        io.write(0, [1, 2, 3, 4]);
        io.write(8, [5, 6, 7, 8]);

        assert_eq!(io.read(0, 16), &[1, 2, 3, 4, 0, 0, 0, 0, 5, 6, 7, 8, 0, 0, 0, 0]);

        io.write(2, [9, 8, 7, 6, 5, 4, 3, 2]);

        assert_eq!(io.read(0, 12), &[1, 2, 9, 8, 7, 6, 5, 4, 3, 2, 7, 8]);
    }

    #[test]
    fn append() {
        let (mut io, path) = get_io("append");

        assert!(path.metadata().unwrap().len() == 0);

        io.append([1, 2]);

        assert_eq!(io.read(0, 4), &[1, 2, 0, 0]);

        io.append([3, 4]);

        assert_eq!(io.read(0, 4), &[1, 2, 3, 4]);
    }

    #[test]
    fn len() {
        let (mut io, path) = get_io("len");

        assert!(path.metadata().unwrap().len() == 0);

        assert!(io.is_empty());
        assert_eq!(io.len(), 0);

        io.append([1, 2, 3]);

        assert!(!io.is_empty());
        assert_eq!(io.len(), 3);

        io.write(1, [0]);

        assert!(!io.is_empty());
        assert_eq!(io.len(), 3);

        io.write(3, [0, 2]);

        assert!(!io.is_empty());
        assert_eq!(io.len(), 5);
    }
}
