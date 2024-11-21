use crate::prelude::*;

#[derive(Debug, Clone)]
/// Book is a meta-structure that allows you to
/// read and write data in a filesystem using
/// linked pages.
///
/// Pages can be physically located at different parts
/// of the disk. This structure will logically
/// merge them into a single read-write buffer and
/// automatically create new ones when needed.
///
/// ```text
/// +---+
/// |   |
/// |   |  <-- offset
/// +---+   |
///   |     |
/// +---+   |
/// |   |   | length
/// |   |   | Read / write operation
/// +---+   |
///   |     |
/// +---+   |
/// |   |  <-- offset + length
/// |   |
/// +---+
/// ```
pub struct Book {
    entry_page: Page,
    page_size: u64
}

impl Book {
    #[inline]
    pub fn entry_page(&self) -> &Page {
        &self.entry_page
    }

    /// Read body with given offset and length.
    ///
    /// This method will return zeros if there's no content
    /// on given offset.
    pub fn read(&self, mut offset: u64, length: u64) -> Vec<u8> {
        let mut page = self.entry_page.clone();

        // Locate page at given offset.
        while offset >= self.page_size {
            page = page.create_next_page();

            offset -= self.page_size;
        }

        // Read the bytes from it.
        page.read(offset, length)
    }

    /// Write data to the given offset.
    ///
    /// This method will overwrite existing data.
    pub fn write(&self, mut offset: u64, bytes: impl Into<Vec<u8>>) {
        let mut page = self.entry_page.clone();

        // Locate page at given offset.
        while offset >= self.page_size {
            page = page.create_next_page();

            offset -= self.page_size;
        }

        // Write the bytes to it.
        let mut tail = page.write(offset, bytes);

        // If some bytes weren't written - keep locating
        // next pages and writing them there.
        while !tail.is_empty() {
            page = page.create_next_page();
            tail = page.write(0, tail);
        }
    }
}
