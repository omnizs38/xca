use super::{compress_with_level, decompress};
use std::{ptr, slice};

#[repr(C)]
pub struct XcaBuffer {
    pub data: *mut u8,
    pub len: usize,
}

fn input_slice<'a>(data: *const u8, len: usize) -> Option<&'a [u8]> {
    if len == 0 {
        Some(&[])
    } else if data.is_null() {
        None
    } else {
        // SAFETY: The caller contract requires `data` to reference `len` readable bytes.
        Some(unsafe { slice::from_raw_parts(data, len) })
    }
}

fn return_buffer(bytes: Vec<u8>, output: *mut XcaBuffer) -> i32 {
    let boxed = bytes.into_boxed_slice();
    let len = boxed.len();
    let data = Box::into_raw(boxed).cast::<u8>();
    // SAFETY: The caller supplied a non-null writable XcaBuffer pointer.
    unsafe { ptr::write(output, XcaBuffer { data, len }) };
    0
}

/// Compresses a byte buffer into an allocated XCA frame.
///
/// Returns 0 on success, 1 for invalid pointers, and 2 for an invalid level.
/// The returned buffer must be released with `xca_free`.
///
/// # Safety
/// `data` must reference `len` readable bytes when `len` is non-zero, and
/// `output` must reference writable memory for one `XcaBuffer`.
#[no_mangle]
pub unsafe extern "C" fn xca_compress(
    data: *const u8,
    len: usize,
    level: u8,
    output: *mut XcaBuffer,
) -> i32 {
    if output.is_null() {
        return 1;
    }
    let Some(input) = input_slice(data, len) else {
        return 1;
    };
    match compress_with_level(input, level) {
        Ok(bytes) => return_buffer(bytes, output),
        Err(_) => 2,
    }
}

/// Decompresses an XCA frame into an allocated byte buffer.
///
/// Returns 0 on success, 1 for invalid pointers, and 3 for invalid input.
/// The returned buffer must be released with `xca_free`.
///
/// # Safety
/// `data` must reference `len` readable bytes when `len` is non-zero, and
/// `output` must reference writable memory for one `XcaBuffer`.
#[no_mangle]
pub unsafe extern "C" fn xca_decompress(
    data: *const u8,
    len: usize,
    output: *mut XcaBuffer,
) -> i32 {
    if output.is_null() {
        return 1;
    }
    let Some(input) = input_slice(data, len) else {
        return 1;
    };
    match decompress(input) {
        Ok(bytes) => return_buffer(bytes, output),
        Err(_) => 3,
    }
}

/// Releases a buffer returned by XCA.
///
/// # Safety
/// `data` and `len` must be an unchanged pair returned by `xca_compress` or
/// `xca_decompress`, and the buffer must not have been freed previously.
#[no_mangle]
pub unsafe extern "C" fn xca_free(data: *mut u8, len: usize) {
    if data.is_null() {
        return;
    }
    let slice = ptr::slice_from_raw_parts_mut(data, len);
    // SAFETY: The caller contract guarantees this is the original boxed slice.
    drop(unsafe { Box::from_raw(slice) });
}
