use crate::prelude::*;

#[derive(Debug, Clone)]
/// Generic B-Tree struct implementation on the filesystem.
///
/// ```text
///       [flags]      [flags]       ...
///       [ key ]      [ key ]       ...
///       [value]      [value]       ...
/// [addr]       [addr]       [addr] ... [addr]
/// ```
///
/// Similarly to binary trees, B-trees keep order of their keys.
/// When looking for a place for some value we're searching through
/// existing records and compare keys, either finding our one or
/// going to the address specified between two different records.
///
/// Records are stored the way to fill the most of all available page space.
/// This improves IO utilization in cost of slightly worse search time.
pub struct GenericBTree<const KEY_SIZE: usize, const VALUE_SIZE: usize> {
    entry_page: u32,
    page_size: u64,
    handler: FilesystemTasksHandler
}

impl<const KEY_SIZE: usize, const VALUE_SIZE: usize> GenericBTree<KEY_SIZE, VALUE_SIZE> {
    pub const RECORD_SIZE: usize = GenericBTreeRecord::<KEY_SIZE, VALUE_SIZE>::RECORD_SIZE;
    pub const RECORD_SHIFT: usize = GenericBTreeRecord::<KEY_SIZE, VALUE_SIZE>::RECORD_SHIFT;

    #[inline]
    pub const fn new(entry_page: u32, page_size: u64, handler: FilesystemTasksHandler) -> Self {
        Self {
            entry_page,
            page_size,
            handler
        }
    }

    /// Calculate how many records can be fit into one page.
    ///
    /// ```text
    ///       [flags]      [flags]      [flags]
    ///       [ key ]      [ key ]      [ key ]
    ///       [value]      [value]      [value]
    /// [addr]       [addr]       [addr]       [addr]
    ///       ^^^^^^^ (1)  ^^^^^^^ (2)  ^^^^^^^ (3)
    /// ```
    pub fn max_records(&self) -> u64 {
        let pages = self.page_size / Self::RECORD_SHIFT as u64;

        if pages * Self::RECORD_SHIFT as u64 + (Self::RECORD_SIZE - Self::RECORD_SHIFT) as u64 <= self.page_size {
            pages
        } else {
            pages - 1
        }
    }

    /// Insert provided value under the given key to the filesystem.
    pub fn insert(&self, key: &[u8; KEY_SIZE], value: [u8; VALUE_SIZE]) {
        let mut curr_page = self.entry_page;

        loop {
            let mut i = 0;

            let (response_sender, response_receiver) = flume::bounded(1);

            self.handler.send_normal(FilesystemTask::ReadPage {
                page_number: curr_page,
                offset: 0,
                length: self.page_size,
                response_sender
            }).unwrap_or_else(|err| {
                panic!("Failed to read body of page 0x{curr_page:08x} : filesystem closed : {err}");
            });

            let page = response_receiver.recv()
                .unwrap_or_else(|err| {
                    panic!("Failed to read body of page 0x{curr_page:08x} : filesystem closed : {err}");
                });

            let mut page = page.as_slice();
            let mut prev_record = None;
            let mut jump_to_page = false;

            while let Some((mut record, remaining)) = GenericBTreeRecord::<KEY_SIZE, VALUE_SIZE>::from_bytes(page) {
                page = remaining;

                match record.key.as_ref() {
                    None => {
                        let record = GenericBTreeRecord::<KEY_SIZE, VALUE_SIZE>::new(*key, value);

                        self.handler.send_normal(FilesystemTask::WritePage {
                            page_number: curr_page,
                            offset: i,
                            bytes: record.to_bytes(),
                            response_sender: None
                        }).unwrap_or_else(|err| {
                            panic!("Failed to create new B-Tree record on page 0x{curr_page:08x}, offset 0x{i:08X} : filesystem closed : {err}");
                        });

                        return;
                    }

                    Some(record_key) if record_key == key => {
                        record.value = Some(value);

                        self.handler.send_normal(FilesystemTask::WritePage {
                            page_number: curr_page,
                            offset: i,
                            bytes: record.to_bytes(),
                            response_sender: None
                        }).unwrap_or_else(|err| {
                            panic!("Failed to update B-Tree record value on page 0x{curr_page:08x}, offset 0x{i:08X} : filesystem closed : {err}");
                        });

                        return;
                    }

                    Some(record_key) if record_key > key => {
                        if let Some(left_addr) = record.left_addr {
                            curr_page = left_addr;
                            jump_to_page = true;

                            break;
                        }

                        else {
                            let (response_sender, response_receiver) = flume::bounded(1);

                            self.handler.send_normal(FilesystemTask::CreatePage {
                                parent_page_number: None,
                                response_sender
                            }).unwrap_or_else(|err| {
                                panic!("Failed to create page : filesystem closed : {err}");
                            });

                            let new_page = response_receiver.recv()
                                .unwrap_or_else(|err| {
                                    panic!("Failed to create page : filesystem closed : {err}");
                                });

                            record.left_addr = Some(new_page.number());

                            self.handler.send_normal(FilesystemTask::WritePage {
                                page_number: curr_page,
                                offset: i,
                                bytes: record.to_bytes(),
                                response_sender: None
                            }).unwrap_or_else(|err| {
                                panic!("Failed to update left B-Tree leaf address on page 0x{curr_page:08x}, offset {i:08x} : filesystem closed : {err}");
                            });

                            curr_page = new_page.number();
                            jump_to_page = true;

                            break;
                        }
                    }

                    _ => ()
                }

                prev_record = Some((i, record));

                i += Self::RECORD_SHIFT as u64;
            }

            if !jump_to_page {
                if let Some((i, mut record)) = prev_record.take() {
                    if let Some(right_addr) = record.right_addr {
                        curr_page = right_addr;
                    }

                    else if i + Self::RECORD_SHIFT as u64 + Self::RECORD_SHIFT as u64 <= self.page_size {
                        // It's safe to write this empty record directly because it will overlap
                        // the previous one only on the right_addr field while this field is empty
                        // and unset.
                        let new_record = GenericBTreeRecord::new(*key, value);

                        self.handler.send_normal(FilesystemTask::WritePage {
                            page_number: curr_page,
                            offset: i + Self::RECORD_SHIFT as u64,
                            bytes: new_record.to_bytes(),
                            response_sender: None
                        }).unwrap_or_else(|err| {
                            panic!("Failed to write initial B-Tree record on page 0x{curr_page:08x} : filesystem closed : {err}");
                        });
                    }

                    else {
                        let (response_sender, response_receiver) = flume::bounded(1);

                        self.handler.send_normal(FilesystemTask::CreatePage {
                            parent_page_number: None,
                            response_sender
                        }).unwrap_or_else(|err| {
                            panic!("Failed to create page : filesystem closed : {err}");
                        });

                        let new_page = response_receiver.recv()
                            .unwrap_or_else(|err| {
                                panic!("Failed to create page : filesystem closed : {err}");
                            });

                        record.right_addr = Some(new_page.number());

                        self.handler.send_normal(FilesystemTask::WritePage {
                            page_number: curr_page,
                            offset: i,
                            bytes: record.to_bytes(),
                            response_sender: None
                        }).unwrap_or_else(|err| {
                            panic!("Failed to update left B-Tree leaf address on page 0x{curr_page:08x}, offset {i:08x} : filesystem closed : {err}");
                        });

                        curr_page = new_page.number();
                    }
                }

                else {
                    let new_record = GenericBTreeRecord::new(*key, value);

                    self.handler.send_normal(FilesystemTask::WritePage {
                        page_number: curr_page,
                        offset: 0,
                        bytes: new_record.to_bytes(),
                        response_sender: None
                    }).unwrap_or_else(|err| {
                        panic!("Failed to write initial B-Tree record on page 0x{curr_page:08x} : filesystem closed : {err}");
                    });

                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::fs::File;

    use crate::filesystem::driver::tests::with_fs;

    use super::*;

    fn with_btree(name: &str, callback: impl FnOnce(BTree64, FilesystemDriver<BufStorageIO<File>>, PathBuf)) {
        with_fs(name, |fs, path| {
            let handler = fs.handler().clone();
            let header = fs.read_header();

            let (response_sender, response_receiver) = flume::bounded(1);

            handler.send(FilesystemTask::CreatePage { parent_page_number: None, response_sender }, FilesystemTaskPriority::High).unwrap();

            let page = response_receiver.recv().unwrap();

            let btree = BTree64::new(page.number(), header.page_size, handler);

            callback(btree, fs, path);
        });
    }

    #[test]
    fn insert() {
        // TODO: check pages vector value

        const RECORDS: u64 = 128;

        with_btree("btree-linear-asc-insert", |btree, _, path| {
            for i in 0..RECORDS {
                let value = seahash::hash(&i.to_be_bytes());

                btree.insert(&i.to_be_bytes(), value.to_be_bytes());
            }

            let pages = (path.metadata().unwrap().len() - FilesystemHeader::LENGTH as u64) / (PageHeader::LENGTH as u64 + btree.page_size);

            // keys[n + 1] < keys[n] => records will fill whole pages space.
            assert_eq!(pages, (RECORDS as f64 / btree.max_records() as f64).ceil() as u64);
        });

        with_btree("btree-linear-desc-insert", |btree, _, path| {
            for i in 0..RECORDS {
                let i = RECORDS - i;
                let value = seahash::hash(&i.to_be_bytes());

                btree.insert(&i.to_be_bytes(), value.to_be_bytes());
            }

            let pages = (path.metadata().unwrap().len() - FilesystemHeader::LENGTH as u64) / (PageHeader::LENGTH as u64 + btree.page_size);

            // keys[n + 1] > keys[n] => all the records will be put
            // on new pages.
            assert_eq!(pages, RECORDS);
        });

        // FIXME: infinite loop?

        // use_btree("btree-random-insert", |btree, _, _| {
        //     use tinyrand::Rand;

        //     let mut rand = tinyrand::Wyrand::default();

        //     for _ in 0..btree.page_size {
        //         let key = rand.next_u64();
        //         let value = seahash::hash(&key.to_be_bytes());

        //         btree.insert(&key.to_be_bytes(), value.to_be_bytes());
        //     }
        // });
    }
}
