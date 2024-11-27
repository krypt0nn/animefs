use crate::prelude::*;

// TODO: implement pages cache in addition to the IO buffer.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageHeader {
    pub prev_page_number: u32,
    pub next_page_number: u32,

    pub has_prev: bool,
    pub has_next: bool
}

impl PageHeader {
    pub const LENGTH: usize = 9;

    pub const FLAG_HAS_PREV: u8 = 0b00000001;
    pub const FLAG_HAS_NEXT: u8 = 0b00000010;

    /// Parse page header from the given bytes slice.
    pub fn from_bytes(bytes: &[u8; Self::LENGTH]) -> Self {
        let mut prev_page_number = [0; 4];
        let mut next_page_number = [0; 4];

        prev_page_number.copy_from_slice(&bytes[0..4]);
        next_page_number.copy_from_slice(&bytes[4..8]);

        Self {
            prev_page_number: u32::from_le_bytes(prev_page_number),
            next_page_number: u32::from_le_bytes(next_page_number),

            has_prev: bytes[8] & Self::FLAG_HAS_PREV == Self::FLAG_HAS_PREV,
            has_next: bytes[8] & Self::FLAG_HAS_NEXT == Self::FLAG_HAS_NEXT
        }
    }

    /// Encode page header into the bytes slice.
    pub fn to_bytes(&self) -> [u8; Self::LENGTH] {
        let mut bytes = [0; Self::LENGTH];

        bytes[0..4].copy_from_slice(&self.prev_page_number.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.next_page_number.to_le_bytes());

        if self.has_prev {
            bytes[8] |= Self::FLAG_HAS_PREV;
        }

        if self.has_next {
            bytes[8] |= Self::FLAG_HAS_NEXT;
        }

        bytes
    }
}

#[derive(Debug, Clone)]
pub struct Page {
    page_number: u32,
    handler: FilesystemTasksHandler
}

impl Page {
    #[inline]
    pub const fn new(number: u32, handler: FilesystemTasksHandler) -> Self {
        Self {
            page_number: number,
            handler
        }
    }

    #[inline]
    pub const fn number(&self) -> u32 {
        self.page_number
    }

    /// Convert current page into a book.
    pub fn into_book(self) -> Book {
        let (response_sender, response_receiver) = flume::bounded(1);

        self.handler.send_normal(FilesystemTask::ReadFilesystemHeader { response_sender })
            .unwrap_or_else(|err| {
                panic!("Failed to read filesystem header : filesystem closed : {err}");
            });

        let header = response_receiver.recv()
            .unwrap_or_else(|err| {
                panic!("Failed to read filesystem header : filesystem closed : {err}");
            });

        Book::open(self, header.page_size)
    }

    /// Read header of the page.
    pub fn read_header(&self) -> PageHeader {
        let (response_sender, response_receiver) = flume::bounded(1);

        self.handler.send_normal(FilesystemTask::ReadPageHeader {
            page_number: self.page_number,
            response_sender
        }).unwrap_or_else(|err| {
            panic!(
                "Failed to read header of page 0x{:08x} : filesystem closed : {err}",
                self.page_number
            );
        });

        response_receiver.recv()
            .unwrap_or_else(|err| {
                panic!(
                    "Failed to read header of page 0x{:08x} : filesystem closed : {err}",
                    self.page_number
                );
            })
    }

    /// Write header of the page.
    pub fn write_header(&self, header: PageHeader) {
        self.handler.send_normal(FilesystemTask::WritePageHeader {
            page_number: self.page_number,
            header
        }).unwrap_or_else(|err| {
            panic!(
                "Failed to write header of page 0x{:08x} : filesystem closed : {err}",
                self.page_number
            );
        });
    }

    /// Try reading the next page if it exists.
    pub fn read_next_page(&self) -> Option<Page> {
        let header = self.read_header();

        if !header.has_next {
            return None;
        }

        Some(Self {
            page_number: header.next_page_number,
            handler: self.handler.clone()
        })
    }

    /// Try reading the previous page if it exists.
    pub fn read_prev_page(&self) -> Option<Page> {
        let header = self.read_header();

        if !header.has_prev {
            return None;
        }

        Some(Self {
            page_number: header.prev_page_number,
            handler: self.handler.clone()
        })
    }

    /// Read next page if it exists or create a new one
    /// and link it with the current one.
    pub fn create_next_page(&self) -> Page {
        if let Some(page) = self.read_next_page() {
            return page;
        }

        let (response_sender, response_receiver) = flume::bounded(1);

        self.handler.send_normal(FilesystemTask::CreatePage {
            parent_page_number: Some(self.page_number),
            response_sender
        }).unwrap_or_else(|err| {
            panic!("Failed to create page : filesystem closed : {err}");
        });

        let page = response_receiver.recv()
            .unwrap_or_else(|err| {
                panic!("Failed to create page : filesystem closed : {err}");
            });

        self.handler.send_normal(FilesystemTask::LinkPageForward {
            page_number: self.page_number,
            next_page_number: page.page_number
        }).unwrap_or_else(|err| {
            panic!(
                "Failed to link page {:08x} with the newly created {:08x} : filesystem closed : {err}",
                self.page_number,
                page.page_number
            );
        });

        page
    }

    /// Read page body with given offset and length.
    ///
    /// This method will return zeros if there's no content
    /// on given offset.
    pub fn read(&self, offset: u64, length: u64) -> Vec<u8> {
        let (response_sender, response_receiver) = flume::bounded(1);

        self.handler.send_normal(FilesystemTask::ReadPage {
            page_number: self.page_number,
            offset,
            length,
            response_sender
        }).unwrap_or_else(|err| {
            panic!(
                "Failed to read page 0x{:08x} : filesystem closed : {err}",
                self.page_number
            );
        });

        response_receiver.recv()
            .unwrap_or_else(|err| {
                panic!(
                    "Failed to read page 0x{:08x} : filesystem closed : {err}",
                    self.page_number
                );
            })
    }

    /// Write bytes to the page, returning bytes that weren't written
    /// if the end of the page was reached.
    ///
    /// This method will overwrite existing data.
    pub fn write(&self, offset: u64, bytes: impl Into<Vec<u8>>) -> Vec<u8> {
        let (response_sender, response_receiver) = flume::bounded(1);

        self.handler.send_normal(FilesystemTask::WritePage {
            page_number: self.page_number,
            offset,
            bytes: bytes.into(),
            response_sender: Some(response_sender)
        }).unwrap_or_else(|err| {
            panic!(
                "Failed to write page 0x{:08x} : filesystem closed : {err}",
                self.page_number
            );
        });

        response_receiver.recv().unwrap_or_default()
    }
}
