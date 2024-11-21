use std::path::Path;
use std::fs::File;

use crate::prelude::*;

#[derive(Debug)]
pub struct FilesystemDriver {
    worker: Option<FilesystemWorker>,
    handler: FilesystemTasksHandler
}

impl FilesystemDriver {
    pub fn open(file: impl AsRef<Path>) -> std::io::Result<Self> {
        let mut file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(file)?;

        // If file was just created - put header in it.
        if file.len() < FilesystemHeader::LENGTH as u64 {
            file.write(0, FilesystemHeader::default().to_bytes());
        }

        let (scheduler, handler) = FilesystemTasksScheduler::new();

        let worker = FilesystemWorker::new(file, scheduler, handler.clone());

        Ok(Self {
            worker: Some(worker),
            handler
        })
    }

    #[inline]
    pub const fn handler(&self) -> &FilesystemTasksHandler {
        &self.handler
    }

    #[inline]
    /// Daemonize filesystem worker.
    pub fn daemonize(&mut self) -> Option<std::thread::JoinHandle<()>> {
        self.worker.take().map(FilesystemWorker::daemonize)
    }

    /// Perform filesystem worker update.
    pub fn update(&mut self) -> anyhow::Result<bool> {
        if let Some(worker) = &mut self.worker {
            worker.update()?;

            return Ok(true);
        }

        Ok(false)
    }

    /// Read header of the filesystem.
    pub fn read_header(&self) -> FilesystemHeader {
        let (response_sender, response_receiver) = flume::bounded(1);

        self.handler.send_high(FilesystemTask::ReadFilesystemHeader { response_sender })
            .unwrap_or_else(|err| {
                panic!("Failed to read filesystem header : filesystem closed : {err}");
            });

        response_receiver.recv()
            .unwrap_or_else(|err| {
                panic!("Failed to read filesystem header : filesystem closed : {err}");
            })
    }

    /// Write header of the filesystem.
    pub fn write_header(&self, header: FilesystemHeader) {
        self.handler.send_high(FilesystemTask::WriteFilesystemHeader { header })
            .unwrap_or_else(|err| {
                panic!("Failed to write filesystem header : filesystem closed : {err}");
            });
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::PathBuf;

    use super::*;

    pub fn use_fs(name: &str, callback: impl FnOnce(FilesystemDriver, PathBuf)) {
        let path = std::env::temp_dir().join(format!(".animefs-test-{name}"));

        if path.exists() {
            std::fs::remove_file(&path)
                .expect("Failed to open filesystem");
        }

        let mut fs = FilesystemDriver::open(&path)
            .expect("Failed to open filesystem");

        fs.daemonize();

        callback(fs, path.clone());

        std::fs::remove_file(path).expect("Failed to delete filesystem");
    }

    #[test]
    fn header() {
        use_fs("header", |fs, _| {
            let header = fs.read_header();

            assert_eq!(header.names_checksum, Checksum::Seahash);
            assert_eq!(header.names_compression, None);
            assert_eq!(header.names_compression_level, CompressionLevel::Auto);

            fs.write_header(FilesystemHeader {
                page_size: 123,
                names_checksum: Checksum::Siphash,
                names_compression: Some(Compression::Lz4),
                names_compression_level: CompressionLevel::Balanced
            });

            let header = fs.read_header();

            assert_eq!(header.page_size, 123);
            assert_eq!(header.names_checksum, Checksum::Siphash);
            assert_eq!(header.names_compression, Some(Compression::Lz4));
            assert_eq!(header.names_compression_level, CompressionLevel::Balanced);
        });
    }
}
