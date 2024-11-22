use std::fs::File;

use crate::prelude::*;

#[derive(Debug)]
/// Struct that listens to incoming tasks and executed them.
pub struct FilesystemWorker {
    file: File,
    scheduler: Option<FilesystemTasksScheduler>,
    handler: FilesystemTasksHandler,

    /// Hot cache of the filesystem header.
    header: FilesystemHeader,

    // Hot cache of the filesystem pages.
    // pages: HashMap<u32, Vec<u8>>
}

impl FilesystemWorker {
    pub fn new(mut file: File, scheduler: FilesystemTasksScheduler, handler: FilesystemTasksHandler) -> Self {
        let mut header = [0; FilesystemHeader::LENGTH];

        header.copy_from_slice(&file.read(0, FilesystemHeader::LENGTH));

        Self {
            file,
            scheduler: Some(scheduler),
            handler,

            header: FilesystemHeader::from_bytes(&header),
            // pages: HashMap::new()
        }
    }

    #[inline]
    pub const fn handler(&self) -> &FilesystemTasksHandler {
        &self.handler
    }

    #[inline]
    /// Spawn new thread and run worker updates in a loop.
    pub fn daemonize(mut self) -> std::thread::JoinHandle<()> {
        if let Some(scheduler) = self.scheduler.take() {
            scheduler.daemonize();
        }

        std::thread::spawn(move || {
            loop {
                if let Err(err) = self.update() {
                    panic!("Failed to execute filesystem task : filesystem closed : {err}");
                }
            }
        })
    }

    /// Poll filesystem task from the scheduler and execute it.
    ///
    /// Returns error on scheduler failure.
    pub fn update(&mut self) -> anyhow::Result<()> {
        if let Some(scheduler) = self.scheduler.as_mut() {
            if !scheduler.update() {
                anyhow::bail!("failed to update filesystem tasks scheduler because all the handlers are closed");
            }
        }

        match self.handler.poll()? {
            FilesystemTask::ReadFilesystemHeader { response_sender } => {
                // let mut header = [0; FilesystemHeader::LENGTH];

                // header.copy_from_slice(&self.file.read(0, FilesystemHeader::LENGTH));

                // let _ = response_sender.send(FilesystemHeader::from_bytes(&header));

                let _ = response_sender.send(self.header);
            }

            FilesystemTask::WriteFilesystemHeader { header } => {
                self.header = header;

                // TODO: can safely be delayed.
                self.file.write(0, header.to_bytes());
            }

            FilesystemTask::CreatePage { parent_page_number, response_sender } => {
                let page_header = PageHeader {
                    prev_page_number: parent_page_number.unwrap_or_default(),
                    next_page_number: 0,

                    has_prev: parent_page_number.is_some(),
                    has_next: false
                };

                let len = self.file.len();

                self.file.append(page_header.to_bytes());
                self.file.append(vec![0; self.header.page_size as usize]);

                if len < FilesystemHeader::LENGTH as u64 {
                    let page = Page::new(0, self.handler.clone());

                    let _ = response_sender.send(page);
                }

                else {
                    let last_page_number = (len - FilesystemHeader::LENGTH as u64) / (PageHeader::LENGTH as u64 + self.header.page_size);

                    let page = Page::new(last_page_number as u32 + 1, self.handler.clone());

                    let _ = response_sender.send(page);
                }
            }

            FilesystemTask::LinkPages { page_number, next_page_number } => {
                todo!("Pages linking is not done yet. Attempted to link 0x{page_number:08x} with 0x{next_page_number:08x}");
            }

            FilesystemTask::ReadPageHeader { page_number, response_sender } => {
                let mut page_header = [0; PageHeader::LENGTH];

                let page_pos = FilesystemHeader::LENGTH as u64 + page_number as u64 * (PageHeader::LENGTH as u64 + self.header.page_size);

                page_header.copy_from_slice(&self.file.read(page_pos, PageHeader::LENGTH));

                let _ = response_sender.send(PageHeader::from_bytes(&page_header));
            }

            FilesystemTask::WritePageHeader { page_number, header } => {
                let page_pos = FilesystemHeader::LENGTH as u64 + page_number as u64 * (PageHeader::LENGTH as u64 + self.header.page_size);

                self.file.write(page_pos, header.to_bytes());
            }

            FilesystemTask::ReadPage { page_number, offset, length, response_sender } => {
                if offset >= self.header.page_size || length == 0 {
                    let _ = response_sender.send(vec![]);
                }

                // else if let Some(bytes) = self.pages.get(&page_number) {
                //     let offset = offset as usize;
                //     let length = length as usize;

                //     let bytes = if offset + length > self.header.page_size as usize {
                //         // offset < page_size
                //         &bytes[offset..]
                //     } else {
                //         &bytes[offset..offset + length]
                //     };

                //     let _ = response_sender.send(bytes.to_vec());
                // }

                else {
                    let page_pos = FilesystemHeader::LENGTH as u64 + page_number as u64 * (PageHeader::LENGTH as u64 + self.header.page_size) + PageHeader::LENGTH as u64;

                    let bytes = if offset + length > self.header.page_size {
                        // offset < page_size
                        self.file.read(page_pos + offset, (self.header.page_size - offset) as usize)
                    } else {
                        self.file.read(page_pos + offset, length as usize)
                    };

                    let _ = response_sender.send(bytes);
                }
            }

            FilesystemTask::WritePage { page_number, offset, bytes, response_sender } => {
                let len = bytes.len() as u64;

                if offset >= self.header.page_size {
                    if let Some(response_sender) = response_sender {
                        let _ = response_sender.send(bytes);
                    }
                }

                else if len > 0 {
                    let page_pos = FilesystemHeader::LENGTH as u64 + page_number as u64 * (PageHeader::LENGTH as u64 + self.header.page_size) + PageHeader::LENGTH as u64;

                    if offset + len > self.header.page_size {
                        //  page: [        ]
                        // bytes:       [     ]
                        //              ^ offset
                        //                 ^ page_size
                        //
                        let split = (self.header.page_size - offset) as usize;

                        self.file.write(page_pos + offset, &bytes[..split]);

                        if let Some(response_sender) = response_sender {
                            let _ = response_sender.send(bytes[split..].to_vec());
                        }
                    }

                    else {
                        self.file.write(page_pos + offset, bytes);
                    }
                }
            }
        }

        Ok(())
    }
}
