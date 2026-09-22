use std::cell::{Cell, RefCell};

pub struct Storage {
    pub physical: Cell<bool>,
    pub recovery: Cell<bool>,
    pub calls: RefCell<Vec<&'static str>>,
}

impl Storage {
    pub fn new() -> Self {
        Self {
            physical: Cell::new(true),
            recovery: Cell::new(true),
            calls: RefCell::new(Vec::new()),
        }
    }

    pub async fn cleanup(&self) -> Result<(), &'static str> {
        self.calls.borrow_mut().push("physical");
        self.physical.set(false);
        Ok(())
    }

    pub async fn finalize(&self) -> Result<bool, &'static str> {
        self.calls.borrow_mut().push("metadata");
        assert!(!self.physical.get());
        Ok(self.recovery.replace(false))
    }
}
