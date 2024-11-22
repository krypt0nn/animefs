use flume::Sender;

use crate::prelude::*;

#[derive(Debug, Clone)]
/// Low-level filesystem operation task.
pub enum FilesystemTask {
    ReadFilesystemHeader {
        response_sender: Sender<FilesystemHeader>
    },

    WriteFilesystemHeader {
        header: FilesystemHeader
    },

    /// Create new filesystem page. It will be assigned to the next
    /// available number, so if the last page has number N - the new
    /// one will have number N + 1.
    CreatePage {
        /// Number of the parent page to link the new one with.
        /// Parent page will not be linked with this one so you have
        /// to call `LinkPages` operation.
        ///
        /// When parent page is not given the new page will not be linked
        /// to anything.
        parent_page_number: Option<u32>,

        /// Where to send the created page.
        response_sender: Sender<Page>
    },

    LinkPages {
        page_number: u32,
        next_page_number: u32
    },

    ReadPageHeader {
        page_number: u32,
        response_sender: Sender<PageHeader>
    },

    WritePageHeader {
        page_number: u32,
        header: PageHeader
    },

    /// Read bytes from the page's body.
    ///
    /// This operation will read requested amount of bytes
    /// from the offset relative to the page's body.
    ///
    /// If the offset is larger than the page's body size -
    /// empty vector will be returned.
    ///
    /// If requested length + offset is larger than the body
    /// size - only available bytes will be returned.
    ReadPage {
        page_number: u32,
        offset: u64,
        length: u64,
        response_sender: Sender<Vec<u8>>
    },

    /// Write bytes to the page's body.
    ///
    /// This operation will write provided bytes slice
    /// to the given page with given offset. Offset is
    /// relative to the page's body. If more bytes given
    /// than page's body can store (page size) - remaining
    /// bytes are returned back to the `response_sender`.
    WritePage {
        page_number: u32,
        offset: u64,
        bytes: Vec<u8>,
        response_sender: Option<Sender<Vec<u8>>>
    }
}
