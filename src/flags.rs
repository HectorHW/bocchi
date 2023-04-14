use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum FlagValue {
    String(String),
    Int(u32),
}

impl From<u32> for FlagValue {
    fn from(value: u32) -> Self {
        FlagValue::Int(value)
    }
}

impl From<String> for FlagValue {
    fn from(value: String) -> Self {
        FlagValue::String(value)
    }
}

#[derive(Clone, Debug)]
pub struct Flags {
    inner: HashMap<String, FlagValue>,
}

impl Flags {
    pub fn new(items: HashMap<String, FlagValue>) -> Self {
        Flags { inner: items }
    }

    pub fn put<T: Into<FlagValue>>(&mut self, key: String, value: T) {
        self.inner.insert(key, value.into());
    }

    pub fn get(&self, key: &str) -> Option<String> {
        match self.inner.get(key) {
            Some(FlagValue::String(s)) => Some(s.to_string()),
            Some(FlagValue::Int(n)) => Some(n.to_string()),
            _ => None,
        }
    }

    pub fn get_int(&self, key: &str) -> Option<Result<u32, String>> {
        match self.inner.get(key) {
            Some(FlagValue::Int(n)) => Some(Ok(*n)),
            Some(FlagValue::String(s)) => Some(s.parse().map_err(|_| s.clone())),
            _ => None,
        }
    }
}
