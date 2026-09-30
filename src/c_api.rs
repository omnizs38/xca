use super::{compress, decompress, decompress_into, frame_info};
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
        2 => b"output buffer has the wrong size\0",
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

fn clear_buffer(output: *mut XcaBuffer) {
    // SAFETY: Callers check that `output` is non-null before using this helper.
    unsafe {
        ptr::write(
            output,
            XcaBuffer {
                data: ptr::null_mut(),
                len: 0,
            },
        );
    }
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
    clear_buffer(output);
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
    clear_buffer(output);
    let Some(input) = input_slice(data, len) else {
        return 1;
    };
    match decompress(input) {
        Ok(bytes) => return_buffer(bytes, output),
        Err(_) => 3,
    }
}

/// Returns the exact decoded size required by `xca_decompress_into`.
///
/// # Safety
/// `data` must reference `len` readable bytes when `len` is non-zero, and
/// `output_len` must point to writable `size_t` storage.
#[no_mangle]
pub unsafe extern "C" fn xca_decompressed_size(
    data: *const u8,
    len: usize,
    output_len: *mut usize,
) -> i32 {
    if output_len.is_null() {
        return 1;
    }
    let Some(input) = input_slice(data, len) else {
        return 1;
    };
    match frame_info(input) {
        Ok(info) => {
            // SAFETY: output_len was checked and the caller guarantees writability.
            unsafe { output_len.write(info.original_size) };
            0
        }
        Err(_) => 3,
    }
}

/// Decompresses directly into caller-owned memory without an allocation or copy.
///
/// Returns 0 on success, 1 for invalid pointers, 2 for a wrong output size,
/// and 3 for invalid input.
///
/// # Safety
/// `data` must reference `len` readable bytes, and `output` must reference
/// `output_len` writable bytes. Input and output must not overlap.
#[no_mangle]
pub unsafe extern "C" fn xca_decompress_into(
    data: *const u8,
    len: usize,
    output: *mut u8,
    output_len: usize,
) -> i32 {
    let Some(input) = input_slice(data, len) else {
        return 1;
    };
    let target = if output_len == 0 {
        &mut []
    } else if output.is_null() {
        return 1;
    } else {
        // SAFETY: The caller contract guarantees writable non-overlapping memory.
        unsafe { slice::from_raw_parts_mut(output, output_len) }
    };
    match decompress_into(input, target) {
        Ok(_) => 0,
        Err(super::Error::LengthMismatch { .. }) => 2,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_buffer_calls_clear_the_output() {
        let mut output = XcaBuffer {
            data: ptr::dangling_mut(),
            len: usize::MAX,
        };
        let code = unsafe { xca_compress(ptr::null(), 1, &mut output) };
        assert_eq!(code, 1);
        assert!(output.data.is_null());
        assert_eq!(output.len, 0);

        output.data = ptr::dangling_mut();
        output.len = usize::MAX;
        let invalid = b"not an XCA archive";
        let code = unsafe { xca_decompress(invalid.as_ptr(), invalid.len(), &mut output) };
        assert_eq!(code, 3);
        assert!(output.data.is_null());
        assert_eq!(output.len, 0);

        unsafe { xca_buffer_free(&mut output) };
    }
}
