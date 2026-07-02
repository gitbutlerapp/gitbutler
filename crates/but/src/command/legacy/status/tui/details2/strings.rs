use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

/// A string interner that avoids allocating commonly used strings over and over.
#[derive(Debug, Clone)]
pub struct Strings {
    shared: Arc<Mutex<SharedStrings>>,
}

impl Default for Strings {
    fn default() -> Self {
        Self {
            shared: Arc::new(Mutex::new(SharedStrings {
                storage: Default::default(),
                u32s: Default::default(),
                spaces: Default::default(),
            })),
        }
    }
}

impl Strings {
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn get(&self, s: String) -> &'static str {
        self.lock().get(s)
    }

    #[expect(dead_code)]
    pub fn get_u32(&self, n: u32) -> &'static str {
        self.lock().get_u32(n)
    }

    #[expect(dead_code)]
    pub fn get_spaces(&self, n: usize) -> &'static str {
        self.lock().get_spaces(n)
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, SharedStrings> {
        self.shared.lock().unwrap()
    }
}

#[derive(Debug)]
pub struct SharedStrings {
    storage: HashSet<&'static str>,
    u32s: HashMap<u32, &'static str>,
    spaces: HashMap<usize, &'static str>,
}

impl SharedStrings {
    pub fn len(&self) -> usize {
        let Self {
            storage,
            u32s,
            spaces,
        } = self;
        storage.len() + u32s.len() + spaces.len()
    }

    pub fn get(&mut self, s: String) -> &'static str {
        let storage = &mut self.storage;
        if let Some(value) = storage.get(&*s) {
            return value;
        }
        let static_s = s.leak();
        storage.insert(static_s);
        static_s
    }

    pub fn get_u32(&mut self, n: u32) -> &'static str {
        let storage = &mut self.u32s;
        if let Some(value) = storage.get(&n) {
            return value;
        }
        let static_s = n.to_string().leak();
        storage.insert(n, static_s);
        static_s
    }

    pub fn get_spaces(&mut self, n: usize) -> &'static str {
        let storage = &mut self.spaces;
        if let Some(value) = storage.get(&n) {
            return value;
        }
        let static_s = " ".repeat(n).leak();
        storage.insert(n, static_s);
        static_s
    }
}
