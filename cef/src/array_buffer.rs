//! ArrayBuffers from byte slices.
//!
//! Safe forms of CEF's two ways to put bytes in an ArrayBuffer. A copy is
//! made where V8 runs; a backing store may be written on any thread first.

use crate::rc::Rc;
use crate::{v8_value_create_array_buffer_with_copy, ImplV8BackingStore, V8BackingStore, V8Value};

/// Creates an ArrayBuffer holding a copy of `bytes`, on a thread where V8
/// runs, as `v8_value_create_array_buffer_with_copy` requires.
pub fn v8_value_create_array_buffer_from_bytes(bytes: &[u8]) -> Option<V8Value> {
    // CEF copies from the buffer and never writes it
    v8_value_create_array_buffer_with_copy(bytes.as_ptr().cast_mut(), bytes.len())
}

impl V8BackingStore {
    /// Copies `bytes` into the store, on any thread.
    ///
    /// Writes nothing and returns false unless this handle is the store's
    /// only reference, the store is not consumed and `bytes` fills it
    /// exactly.
    pub fn write(&mut self, bytes: &[u8]) -> bool {
        if !self.has_one_ref() || self.byte_length() != bytes.len() {
            return false;
        }
        let data = self.data().cast::<u8>();
        if data.is_null() {
            return false;
        }
        // SAFETY: CEF lets any thread write a backing store's `byte_length()`
        // bytes until the store is consumed, which takes a reference to it.
        // This handle is the only reference and is borrowed mutably, so
        // nothing reads, writes or consumes the store during the copy
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len()) };
        true
    }
}
