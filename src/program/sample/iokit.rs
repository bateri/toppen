//! What macOS keeps in its I/O registry that sysinfo does not read: the accelerators' statistics
//! and the storage drivers' counters. Hand-declared, so no crate for a dozen calls: IOKit and
//! CoreFoundation are linked as frameworks and a property is converted into a plain `Value`.

use std::collections::HashMap;

/// A property of a registry entry, as far as this program reads one.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Text(String),
    Dict(HashMap<String, Value>),
    Other,
}

impl Value {
    /// The number, whichever way it is kept.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn number(&self) -> Option<f64> {
        match self {
            Self::Int(number) => Some(*number as f64),
            Self::Float(number) => Some(*number),
            _ => None,
        }
    }

    #[must_use]
    pub fn whole(&self) -> Option<u64> {
        match self {
            Self::Int(number) => u64::try_from(*number).ok(),
            _ => None,
        }
    }

    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    #[must_use]
    pub fn dict(&self) -> Option<&HashMap<String, Value>> {
        match self {
            Self::Dict(dict) => Some(dict),
            _ => None,
        }
    }
}

/// A service of the registry: its own name and what it holds, and the entry above it, which is
/// where a disk's counters are kept.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub properties: HashMap<String, Value>,
    pub parent: Option<Box<Entry>>,
}

impl Entry {
    /// A property by name.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.properties.get(key)
    }
}

#[cfg(target_os = "macos")]
mod ffi {
    use std::ffi::{c_char, c_void};

    pub type Ref = *const c_void;
    pub type Port = u32;

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        pub fn IOServiceMatching(name: *const c_char) -> Ref;
        pub fn IOServiceGetMatchingServices(main: Port, matching: Ref, found: *mut Port) -> i32;
        pub fn IOIteratorNext(iterator: Port) -> Port;
        pub fn IOObjectRelease(object: Port) -> i32;
        pub fn IORegistryEntryCreateCFProperties(
            entry: Port,
            properties: *mut Ref,
            allocator: Ref,
            options: u32,
        ) -> i32;
        pub fn IORegistryEntryGetName(entry: Port, name: *mut c_char) -> i32;
        pub fn IORegistryEntryGetParentEntry(
            entry: Port,
            plane: *const c_char,
            parent: *mut Port,
        ) -> i32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        pub fn CFRelease(object: Ref);
        pub fn CFGetTypeID(object: Ref) -> usize;
        pub fn CFNumberGetTypeID() -> usize;
        pub fn CFStringGetTypeID() -> usize;
        pub fn CFDictionaryGetTypeID() -> usize;
        pub fn CFNumberIsFloatType(number: Ref) -> u8;
        pub fn CFNumberGetValue(number: Ref, kind: isize, into: *mut c_void) -> u8;
        pub fn CFStringGetLength(string: Ref) -> isize;
        pub fn CFStringGetCString(
            string: Ref,
            buffer: *mut c_char,
            size: isize,
            encoding: u32,
        ) -> u8;
        pub fn CFDictionaryGetCount(dictionary: Ref) -> isize;
        pub fn CFDictionaryGetKeysAndValues(dictionary: Ref, keys: *mut Ref, values: *mut Ref);
    }

    pub const SINT64: isize = 4;
    pub const FLOAT64: isize = 6;
    pub const UTF8: u32 = 0x0800_0100;
}

#[cfg(target_os = "macos")]
fn text_of(string: ffi::Ref) -> Option<String> {
    // SAFETY: `string` is a CFString the caller got from the registry; the buffer is as big as the
    // longest UTF-8 form of its characters, which is what the call is told.
    unsafe {
        let length = ffi::CFStringGetLength(string);
        let size = length.checked_mul(4)?.checked_add(1)?;
        let mut buffer = vec![0_u8; usize::try_from(size).ok()?];
        if ffi::CFStringGetCString(string, buffer.as_mut_ptr().cast(), size, ffi::UTF8) == 0 {
            return None;
        }
        let end = buffer
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(buffer.len());
        String::from_utf8(buffer[..end].to_vec()).ok()
    }
}

#[cfg(target_os = "macos")]
fn value_of(value: ffi::Ref, depth: usize) -> Value {
    if value.is_null() {
        return Value::Other;
    }
    // SAFETY: `value` is a live CoreFoundation object of the dictionary that holds it; each call
    // below is made on the type its type id says it is.
    unsafe {
        let kind = ffi::CFGetTypeID(value);
        if kind == ffi::CFNumberGetTypeID() {
            if ffi::CFNumberIsFloatType(value) != 0 {
                let mut number = 0.0_f64;
                if ffi::CFNumberGetValue(value, ffi::FLOAT64, std::ptr::addr_of_mut!(number).cast())
                    != 0
                {
                    return Value::Float(number);
                }
            } else {
                let mut number = 0_i64;
                if ffi::CFNumberGetValue(value, ffi::SINT64, std::ptr::addr_of_mut!(number).cast())
                    != 0
                {
                    return Value::Int(number);
                }
            }
            Value::Other
        } else if kind == ffi::CFStringGetTypeID() {
            text_of(value).map_or(Value::Other, Value::Text)
        } else if kind == ffi::CFDictionaryGetTypeID() && depth < 4 {
            Value::Dict(dict_of(value, depth + 1))
        } else {
            Value::Other
        }
    }
}

#[cfg(target_os = "macos")]
fn dict_of(dictionary: ffi::Ref, depth: usize) -> HashMap<String, Value> {
    // SAFETY: `dictionary` is a live CFDictionary; the arrays are as long as its count says.
    unsafe {
        let count = usize::try_from(ffi::CFDictionaryGetCount(dictionary)).unwrap_or(0);
        let mut keys: Vec<ffi::Ref> = vec![std::ptr::null(); count];
        let mut values: Vec<ffi::Ref> = vec![std::ptr::null(); count];
        ffi::CFDictionaryGetKeysAndValues(dictionary, keys.as_mut_ptr(), values.as_mut_ptr());
        let mut found = HashMap::new();
        for (key, value) in keys.into_iter().zip(values) {
            if key.is_null() || ffi::CFGetTypeID(key) != ffi::CFStringGetTypeID() {
                continue;
            }
            if let Some(name) = text_of(key) {
                found.insert(name, value_of(value, depth));
            }
        }
        found
    }
}

#[cfg(target_os = "macos")]
fn name_of(entry: ffi::Port) -> String {
    let mut buffer = [0 as std::ffi::c_char; 128];
    // SAFETY: the buffer is the 128 bytes the call is documented to need.
    let status = unsafe { ffi::IORegistryEntryGetName(entry, buffer.as_mut_ptr()) };
    if status != 0 {
        return String::new();
    }
    // SAFETY: the call wrote a NUL-terminated string into the buffer.
    unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

/// One entry of the registry, with the entry above it when `up` is true.
#[cfg(target_os = "macos")]
fn entry_of(entry: ffi::Port, up: bool) -> Entry {
    let mut properties: ffi::Ref = std::ptr::null();
    // SAFETY: the dictionary is made for us and released below.
    let got = unsafe {
        ffi::IORegistryEntryCreateCFProperties(entry, &raw mut properties, std::ptr::null(), 0)
    };
    let held = if got == 0 && !properties.is_null() {
        dict_of(properties, 0)
    } else {
        HashMap::new()
    };
    if !properties.is_null() {
        // SAFETY: made above, let go once.
        unsafe { ffi::CFRelease(properties) };
    }
    let mut parent: ffi::Port = 0;
    let above = if up {
        // SAFETY: `parent` is where the call says what it found; it is released below.
        unsafe { ffi::IORegistryEntryGetParentEntry(entry, c"IOService".as_ptr(), &raw mut parent) }
    } else {
        -1
    };
    let over = (above == 0).then(|| {
        let over = entry_of(parent, false);
        // SAFETY: made by the call above, let go once.
        unsafe { ffi::IOObjectRelease(parent) };
        Box::new(over)
    });
    Entry {
        name: name_of(entry),
        properties: held,
        parent: over,
    }
}

/// The services of the registry of `class`, with their properties.
#[cfg(target_os = "macos")]
#[must_use]
pub fn services(class: &std::ffi::CStr) -> Vec<Entry> {
    let mut found = Vec::new();
    let mut iterator: ffi::Port = 0;
    // SAFETY: the matching dictionary is made by the call below and handed on (it is consumed);
    // `iterator` is where the call says what it found.
    let status = unsafe {
        ffi::IOServiceGetMatchingServices(
            0,
            ffi::IOServiceMatching(class.as_ptr()),
            &raw mut iterator,
        )
    };
    if status != 0 {
        return found;
    }
    loop {
        // SAFETY: `iterator` is the one the call above made.
        let entry = unsafe { ffi::IOIteratorNext(iterator) };
        if entry == 0 {
            break;
        }
        found.push(entry_of(entry, true));
        // SAFETY: made by the iterator, let go once.
        unsafe { ffi::IOObjectRelease(entry) };
    }
    // SAFETY: the iterator is let go once.
    unsafe { ffi::IOObjectRelease(iterator) };
    found
}

#[cfg(not(target_os = "macos"))]
#[must_use]
pub fn services(_class: &std::ffi::CStr) -> Vec<Entry> {
    Vec::new()
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn the_registry_is_read_without_harm_and_says_what_this_mac_has() {
        for class in [c"IOAccelerator", c"IOMedia"] {
            for entry in services(class) {
                eprintln!(
                    "{class:?} {} <- {:?}",
                    entry.name,
                    entry.parent.as_ref().map(|parent| parent.name.as_str())
                );
                for key in [
                    "model",
                    "IOClass",
                    "BSD Name",
                    "Whole",
                    "Removable",
                    "Ejectable",
                ] {
                    if let Some(value) = entry.get(key) {
                        eprintln!("    {key}: {value:?}");
                    }
                }
            }
        }
    }
}
