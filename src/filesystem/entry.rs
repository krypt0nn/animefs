pub use crate::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FilesystemEntry {
    /// Hash of the entry's name.
    pub name: u64,

    /// Address within the book of the first entry's sibling.
    pub sibling_addr: u64,

    /// Address within the book of the first entry's child.
    pub child_addr: u64
}

impl FilesystemEntry {
    pub const LENGTH: usize = 24;

    #[inline]
    /// Create new entry without children and siblings.
    pub const fn new(name: u64) -> Self {
        Self {
            name,
            sibling_addr: 0,
            child_addr: 0
        }
    }

    #[inline]
    /// Check if the current entry is unset (empty).
    pub fn is_empty(&self) -> bool {
        (self.name | self.sibling_addr | self.child_addr) == 0
    }

    pub fn from_bytes(bytes: &[u8; Self::LENGTH]) -> Self {
        let mut name = [0; 8];
        let mut sibling_addr = [0; 8];
        let mut child_addr = [0; 8];

        name.copy_from_slice(&bytes[..8]);
        sibling_addr.copy_from_slice(&bytes[8..16]);
        child_addr.copy_from_slice(&bytes[16..24]);

        Self {
            name: u64::from_be_bytes(name),
            sibling_addr: u64::from_be_bytes(sibling_addr),
            child_addr: u64::from_be_bytes(child_addr)
        }
    }

    pub fn to_bytes(&self) -> [u8; Self::LENGTH] {
        let mut entry = [0; Self::LENGTH];

        entry[..8].copy_from_slice(&self.name.to_be_bytes());
        entry[8..16].copy_from_slice(&self.sibling_addr.to_be_bytes());
        entry[16..24].copy_from_slice(&self.child_addr.to_be_bytes());

        entry
    }
}

#[derive(Debug, Clone)]
pub struct FilesystemTreeReader<const BUF_SIZE: u64> {
    book: Book,
    offset: u64,
    buf_offset: u64,
    buf: Vec<u8>
}

impl<const BUF_SIZE: u64> Iterator for FilesystemTreeReader<BUF_SIZE> {
    type Item = (u64, FilesystemEntry);

    fn next(&mut self) -> Option<Self::Item> {
        if self.buf.is_empty() || (self.offset > self.buf_offset && self.offset - self.buf_offset > BUF_SIZE - FilesystemEntry::LENGTH as u64) || self.buf_offset > self.offset {
            self.buf_offset = self.offset;

            self.buf = self.book.read(self.offset, BUF_SIZE);
        }

        let i = (self.offset - self.buf_offset) as usize;

        let mut entry = [0; FilesystemEntry::LENGTH];

        entry.copy_from_slice(&self.buf[i..i + FilesystemEntry::LENGTH]);

        let entry = FilesystemEntry::from_bytes(&entry);
        let offset = self.offset;

        self.offset += FilesystemEntry::LENGTH as u64;

        (!entry.is_empty()).then_some((offset, entry))
    }
}

impl<const BUF_SIZE: u64> std::iter::FusedIterator for FilesystemTreeReader<BUF_SIZE> {}

#[derive(Debug, Clone)]
pub struct FilesystemTree {
    book: Book,
    last_entry_addr: u64
}

impl FilesystemTree {
    /// Offset of the root entry. Use it to read the root's children and siblings
    /// or to insert new ones.
    pub const ROOT_OFFSET: u64 = 8;

    #[inline]
    /// Open filesystem tree reader from the given book.
    pub fn open(book: Book) -> Self {
        let mut last_entry_addr = [0; 8];

        last_entry_addr.copy_from_slice(&book.read(0, 8));

        let last_entry_addr = u64::from_be_bytes(last_entry_addr);

        Self {
            book,

            last_entry_addr: if last_entry_addr == 0 {
                Self::ROOT_OFFSET
            } else {
                last_entry_addr
            }
        }
    }

    #[inline]
    /// Read root filesystem entries.
    pub fn read_root<const BUF_SIZE: u64>(&self) -> FilesystemTreeReader<BUF_SIZE> {
        self.read(Self::ROOT_OFFSET)
    }

    #[inline]
    /// Read filesystem entries starting from the specified offset.
    ///
    /// `BUF_SIZE` specifies amount of bytes to read from the disk at once.
    ///
    /// **WARNING**: First bytes of the page are used to hold metadata about the entries tree.
    /// Make sure to use `FilesystemTree::ROOT_OFFSET` as the first entry offset.
    pub fn read<const BUF_SIZE: u64>(&self, offset: u64) -> FilesystemTreeReader<BUF_SIZE> {
        FilesystemTreeReader {
            book: self.book.clone(),
            offset,
            buf_offset: 0,
            buf: vec![]
        }
    }

    /// Insert given entry as a child of the entry under the provided offset.
    ///
    /// If entry under the offset already has a child - function will iterate
    /// to the latest one and link it with the inserted entry.
    ///
    /// Return offset of the inserted entry.
    ///
    /// **WARNING**: First bytes of the page are used to hold metadata about the entries tree.
    /// Make sure to use `FilesystemTree::ROOT_OFFSET` as the first entry offset.
    /// Also be accurate to not to create cycle references.
    pub fn insert_child(&mut self, mut offset: u64, entry: FilesystemEntry) -> u64 {
        let mut parent = [0; FilesystemEntry::LENGTH];

        parent.copy_from_slice(&self.book.read(offset, FilesystemEntry::LENGTH as u64));

        let mut parent = FilesystemEntry::from_bytes(&parent);

        while parent.child_addr != 0 {
            let mut bytes = [0; FilesystemEntry::LENGTH];

            bytes.copy_from_slice(&self.book.read(parent.child_addr, FilesystemEntry::LENGTH as u64));

            offset = parent.child_addr;
            parent = FilesystemEntry::from_bytes(&bytes);
        }

        let i = self.last_entry_addr + FilesystemEntry::LENGTH as u64;

        parent.child_addr = i;

        self.book.write(i, entry.to_bytes());
        self.book.write(offset, parent.to_bytes());

        self.last_entry_addr = i;

        self.book.write(0, self.last_entry_addr.to_be_bytes());

        i
    }

    /// Insert given entry as a sibling of the entry under the provided offset.
    ///
    /// If entry under the offset already has a sibling - function will iterate
    /// to the latest one and link it with the inserted entry.
    ///
    /// Return offset of the inserted entry.
    ///
    /// **WARNING**: First bytes of the page are used to hold metadata about the entries tree.
    /// Make sure to use `FilesystemTree::ROOT_OFFSET` as the first entry offset.
    /// Also be accurate to not to create cycle references.
    pub fn insert_sibling<const BUF_SIZE: u64>(&mut self, offset: u64, entry: FilesystemEntry) -> u64 {
        let reader = self.read::<BUF_SIZE>(offset);

        match reader.last() {
            Some((offset, mut parent)) => {
                let i = self.last_entry_addr + FilesystemEntry::LENGTH as u64;

                parent.sibling_addr = i;

                self.book.write(i, entry.to_bytes());
                self.book.write(offset, parent.to_bytes());

                self.last_entry_addr = i;

                self.book.write(0, self.last_entry_addr.to_be_bytes());

                i
            }

            // There's no entry under the offset so we can freely make a new one.
            None => {
                self.book.write(offset, entry.to_bytes());

                offset
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use crate::filesystem::driver::tests::with_fs;

    use super::*;

    #[test]
    fn children() {
        with_fs("entry-children", |fs, _| {
            let book = Page::new(0, fs.handler().clone()).into_book();

            let mut tree = FilesystemTree::open(book);
            let mut offset = FilesystemTree::ROOT_OFFSET;

            for i in 0..1000 {
                let entry = FilesystemEntry::new(i);

                offset = tree.insert_child(offset, entry);
            }

            assert_eq!(offset, FilesystemTree::ROOT_OFFSET + 1000 * FilesystemEntry::LENGTH as u64);
            assert_eq!(tree.read::<32>(offset).next(), Some((offset, FilesystemEntry::new(999))));

            for i in 1000..1100 {
                let entry = FilesystemEntry::new(i);

                offset = tree.insert_child(FilesystemTree::ROOT_OFFSET, entry);
            }

            assert_eq!(offset, FilesystemTree::ROOT_OFFSET + 1100 * FilesystemEntry::LENGTH as u64);
            assert_eq!(tree.read::<32>(offset).next(), Some((offset, FilesystemEntry::new(1099))));
        });
    }
}
