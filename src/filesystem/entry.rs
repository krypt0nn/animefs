pub use crate::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FilesystemEntry {
    /// Hash of the entry's name.
    pub name: u64,

    /// ID of the entry, 0 if it's not readable.
    pub inode: u64,

    /// Address within the book of the first entry's sibling.
    pub sibling_addr: u64,

    /// Address within the book of the first entry's child.
    pub child_addr: u64
}

impl FilesystemEntry {
    pub const LENGTH: usize = 32;

    #[inline]
    /// Create new entry without children and siblings.
    ///
    /// If inode is 0, then the entry is not readable.
    pub const fn new(name: u64, inode: u64) -> Self {
        Self {
            name,
            inode,
            sibling_addr: 0,
            child_addr: 0
        }
    }

    #[inline]
    /// Check if the current entry is unset (empty).
    pub const fn is_empty(&self) -> bool {
        (self.name | self.sibling_addr | self.child_addr) == 0
    }

    #[inline]
    /// Check if the current entry is readable.
    ///
    /// Readable entries have non-zero inode which
    /// could be looked up in the metadata B-Tree
    /// to read the file's content.
    pub const fn is_readable(&self) -> bool {
        self.inode != 0
    }

    pub fn from_bytes(bytes: &[u8; Self::LENGTH]) -> Self {
        let mut name = [0; 8];
        let mut inode = [0; 8];
        let mut sibling_addr = [0; 8];
        let mut child_addr = [0; 8];

        name.copy_from_slice(&bytes[..8]);
        inode.copy_from_slice(&bytes[8..16]);
        sibling_addr.copy_from_slice(&bytes[16..24]);
        child_addr.copy_from_slice(&bytes[24..32]);

        Self {
            name: u64::from_be_bytes(name),
            inode: u64::from_be_bytes(inode),
            sibling_addr: u64::from_be_bytes(sibling_addr),
            child_addr: u64::from_be_bytes(child_addr)
        }
    }

    pub fn to_bytes(&self) -> [u8; Self::LENGTH] {
        let mut entry = [0; Self::LENGTH];

        entry[..8].copy_from_slice(&self.name.to_be_bytes());
        entry[8..16].copy_from_slice(&self.inode.to_be_bytes());
        entry[16..24].copy_from_slice(&self.sibling_addr.to_be_bytes());
        entry[24..32].copy_from_slice(&self.child_addr.to_be_bytes());

        entry
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Filesystem reader mode.
pub enum FilesystemTreeReaderMode {
    /// Read all the siblings of the entry.
    ///
    /// # Example:
    ///
    /// ```text
    /// /
    /// ├── a1
    /// │   ├── b1
    /// │   │   └── c1
    /// │   │       └── d1
    /// │   ├── b2
    /// │   │   └── c2
    /// │   └── b3
    /// └── a2
    /// ```
    ///
    /// Reading childs of `a1` will return offsets of:
    /// - a1
    /// - a2
    ///
    /// Reading childs of `b1` will return offsets of:
    /// - b1
    /// - b2
    /// - b3
    Sibling,

    /// Read first childs of the entries.
    ///
    /// # Example:
    ///
    /// ```text
    /// /
    /// ├── a1
    /// │   ├── b1
    /// │   │   └── c1
    /// │   │       └── d1
    /// │   ├── b2
    /// │   │   └── c2
    /// │   └── b3
    /// └── a2
    /// ```
    ///
    /// Reading childs of `a1` will return offsets of:
    /// - a1
    /// - b1
    /// - c1
    /// - d1
    ///
    /// Reading childs of `b2` will return offsets of:
    ///
    /// - b2
    /// - c2
    Child
}

#[derive(Debug, Clone)]
/// Reader of the filesystem entry's childs or siblings tree.
/// Read `FilesystemTreeReaderMode` for working mode details.
///
/// This iterator has an inner buffer of the filesystem tree
/// to optimize disk reads. It's recommended to give it
/// values multiple by the `FilesystemEntry::LENGTH`.
///
/// E.g. `BUF_SIZE = 32 * FilesystemEntry::LENGTH`.
pub struct FilesystemTreeReader<const BUF_SIZE: u64> {
    book: Book,
    offset: u64,
    buf_offset: u64,
    buf: Vec<u8>,
    mode: FilesystemTreeReaderMode
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

        if entry.is_empty() {
            return None;
        }

        let offset = self.offset;

        match self.mode {
            FilesystemTreeReaderMode::Sibling if entry.sibling_addr != 0 => {
                self.offset = entry.sibling_addr;

                Some((offset, entry))
            }

            FilesystemTreeReaderMode::Child if entry.child_addr != 0 => {
                self.offset = entry.child_addr;

                Some((offset, entry))
            }

            _ => None
        }
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

    /// Read filesystem entry at the given offset.
    pub fn read(&self, offset: u64) -> FilesystemEntry {
        let mut entry = [0; FilesystemEntry::LENGTH];

        entry.copy_from_slice(&self.book.read(offset, FilesystemEntry::LENGTH as u64));

        FilesystemEntry::from_bytes(&entry)
    }

    #[inline]
    /// Write filesystem entry to the given offset.
    pub fn write(&self, offset: u64, entry: FilesystemEntry) {
        self.book.write(offset, entry.to_bytes());
    }

    #[inline]
    /// Read root filesystem entries.
    ///
    /// Returns reader of the root entry in the
    /// `FilesystemTreeReaderMode::Sibling` mode.
    pub fn read_root<const BUF_SIZE: u64>(&self) -> FilesystemTreeReader<BUF_SIZE> {
        self.reader(Self::ROOT_OFFSET, FilesystemTreeReaderMode::Sibling)
    }

    #[inline]
    /// Read filesystem entries starting from the specified offset.
    ///
    /// `BUF_SIZE` specifies amount of bytes to read from the disk at once.
    ///
    /// **WARNING**: First bytes of the page are used to hold metadata about the entries tree.
    /// Make sure to use `FilesystemTree::ROOT_OFFSET` as the first entry offset.
    pub fn reader<const BUF_SIZE: u64>(&self, offset: u64, mode: FilesystemTreeReaderMode) -> FilesystemTreeReader<BUF_SIZE> {
        FilesystemTreeReader {
            book: self.book.clone(),
            offset,
            buf_offset: 0,
            buf: vec![],
            mode
        }
    }

    /// Insert given entry as a child of the entry under the provided offset.
    ///
    /// ```text
    /// /
    /// ├── a1
    /// │   ├── b1
    /// │   └── **b2** <-- Inserting child entry b2 to the entry a1
    /// └── a2
    /// ```
    ///
    /// If entry under the offset already has a child - function will iterate
    /// to the latest one and link it with the inserted entry.
    ///
    /// Return offset of the inserted entry.
    ///
    /// `BUF_SIZE` specifies amount of bytes to read from the disk at once.
    ///
    /// **WARNING**: First bytes of the page are used to hold metadata about the entries tree.
    /// Make sure to use `FilesystemTree::ROOT_OFFSET` as the first entry offset.
    /// Also be accurate to not to create cycle references.
    pub fn insert_child<const BUF_SIZE: u64>(&mut self, offset: u64, entry: FilesystemEntry) -> u64 {
        let mut parent = self.read(offset);

        // Write entry node to the disk.
        let i = self.last_entry_addr + FilesystemEntry::LENGTH as u64;

        self.last_entry_addr = i;

        self.book.write(i, entry.to_bytes());
        self.book.write(0, self.last_entry_addr.to_be_bytes());

        // If parent entry doesn't have any children yet - just
        // update its first reference.
        if parent.child_addr == 0 {
            parent.child_addr = i;

            self.write(offset, parent);
        }

        // Otherwise read to the last child of the given entry
        // and update its sibling address.
        else {
            let reader = self.reader::<BUF_SIZE>(parent.child_addr, FilesystemTreeReaderMode::Sibling);

            match reader.last() {
                Some((offset, mut child)) => {
                    child.sibling_addr = i;

                    self.write(offset, child);
                }

                // Must be impossible but whatever.
                None => {
                    parent.child_addr = i;

                    self.write(offset, parent);
                }
            }
        }

        i
    }

    /// Insert given entry as a sibling of the entry under the provided offset.
    ///
    /// ```text
    /// /
    /// ├── a1
    /// │   ├── b1
    /// │   └── b2
    /// └── **a2** <-- Inserting sibling entry a2 to the entry a1
    /// ```
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
        let reader = self.reader::<BUF_SIZE>(offset, FilesystemTreeReaderMode::Sibling);

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
                self.write(offset, entry);

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

            for i in 1..128 {
                let entry = FilesystemEntry::new(i, 0);

                offset = tree.insert_child::<1024>(FilesystemTree::ROOT_OFFSET, entry);
            }

            assert_eq!(offset, FilesystemTree::ROOT_OFFSET + 127 * FilesystemEntry::LENGTH as u64);

            let root = tree.read(FilesystemTree::ROOT_OFFSET);

            let (offset, last_child) = tree.reader::<1024>(root.child_addr, FilesystemTreeReaderMode::Sibling).last().unwrap();

            assert_eq!(offset, FilesystemTree::ROOT_OFFSET + 127 * FilesystemEntry::LENGTH as u64);
            assert_eq!(last_child, FilesystemEntry::new(127, 0));
        });
    }
}
