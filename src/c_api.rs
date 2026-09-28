use super::{compress, decompress};
use std::{ffi::c_char, ptr, slice};

#[repr(C)]
pub struct XcaBuffer {
    pub data: *mut u8,
    pub len: usize,
}

#[no_mangle]
pub extern "C" fn xca_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn xca_error_string(code: i32) -> *const c_char {
    let message: &'static [u8] = match code {
        0 => b"success\0",
        1 => b"invalid pointer\0",
        3 => b"invalid or corrupted XCA input\0",
        _ => b"unknown XCA error\0",
    };
    message.as_ptr().cast()
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
/// Returns 0 on success and 1 for invalid pointers.
/// The returned buffer must be released with `xca_buffer_free` or `xca_free`.
///
/// # Safety
/// `data` must reference `len` readable bytes when `len` is non-zero, and
/// `output` must reference writable memory for one `XcaBuffer`.
#[no_mangle]
pub unsafe extern "C" fn xca_compress(data: *const u8, len: usize, output: *mut XcaBuffer) -> i32 {
    if output.is_null() {
        return 1;
    }
    let Some(input) = input_slice(data, len) else {
        return 1;
    };
    return_buffer(compress(input), output)
}

/// Decompresses an XCA frame into an allocated byte buffer.
///
/// Returns 0 on success, 1 for invalid pointers, and 3 for invalid input.
/// The returned buffer must be released with `xca_buffer_free` or `xca_free`.
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

/// Releases and clears an `XcaBuffer` returned by XCA.
///
/// # Safety
/// `buffer` must be null or point to a writable `XcaBuffer` returned by XCA.
#[no_mangle]
pub unsafe extern "C" fn xca_buffer_free(buffer: *mut XcaBuffer) {
    if buffer.is_null() {
        return;
    }
    // SAFETY: The caller guarantees a writable XcaBuffer pointer.
    let buffer = unsafe { &mut *buffer };
    // SAFETY: The pair was returned by XCA and is cleared immediately.
    unsafe { xca_free(buffer.data, buffer.len) };
    buffer.data = ptr::null_mut();
    buffer.len = 0;
}
