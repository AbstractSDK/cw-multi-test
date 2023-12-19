use cosmwasm_std::{Order, Record, Storage};

pub struct StorageWrapper<'a> {
    storage: &'a mut dyn Storage,
}

impl<'a> StorageWrapper<'a> {
    pub fn new(storage: &'a mut dyn Storage) -> Self {
        StorageWrapper { storage }
    }
}

impl<'a> Storage for StorageWrapper<'a> {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.storage.get(key)
    }

    fn set(&mut self, key: &[u8], value: &[u8]) {
        self.storage.set(key, value)
    }

    fn remove(&mut self, key: &[u8]) {
        self.storage.remove(key)
    }

    /// range allows iteration over a set of keys, either forwards or backwards
    /// uses standard rust range notation, and eg db.range(b"foo"..b"bar") also works reverse
    fn range<'b>(
        &'b self,
        start: Option<&[u8]>,
        end: Option<&[u8]>,
        order: Order,
    ) -> Box<dyn Iterator<Item = Record> + 'b> {
        self.storage.range(start, end, order)
    }
}

pub struct ReadonlyStorageWrapper<'a> {
    storage: &'a dyn Storage,
}

impl<'a> ReadonlyStorageWrapper<'a> {
    pub fn new(storage: &'a dyn Storage) -> Self {
        ReadonlyStorageWrapper { storage }
    }
}

impl<'a> Storage for ReadonlyStorageWrapper<'a> {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.storage.get(key)
    }

    fn set(&mut self, _key: &[u8], _value: &[u8]) {
        unimplemented!()
    }

    fn remove(&mut self, _key: &[u8]) {
        unimplemented!()
    }

    /// range allows iteration over a set of keys, either forwards or backwards
    /// uses standard rust range notation, and eg db.range(b"foo"..b"bar") also works reverse
    fn range<'b>(
        &'b self,
        start: Option<&[u8]>,
        end: Option<&[u8]>,
        order: Order,
    ) -> Box<dyn Iterator<Item = Record> + 'b> {
        self.storage.range(start, end, order)
    }
}
