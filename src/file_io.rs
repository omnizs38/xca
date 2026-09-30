use crate::Error;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

fn ioe(error: std::io::Error) -> Error {
    Error::Io(error.to_string())
}

pub(crate) fn write_all(path: &Path, data: &[u8]) -> Result<(), Error> {
    let mut file = File::create(path).map_err(ioe)?;
    file.set_len(data.len() as u64).map_err(ioe)?;
    file.write_all(data).map_err(ioe)
}

pub(crate) struct ReadMap {
    inner: platform::Mapping,
}

impl ReadMap {
    pub(crate) fn open(path: &Path) -> Result<Self, Error> {
        Ok(Self {
            inner: platform::Mapping::open(path).map_err(ioe)?,
        })
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        self.inner.as_slice()
    }
}

pub(crate) struct WriteMap {
    inner: platform::Mapping,
}

impl WriteMap {
    pub(crate) fn create(path: &Path, len: usize) -> Result<Self, Error> {
        Ok(Self {
            inner: platform::Mapping::create(path, len).map_err(ioe)?,
        })
    }

    pub(crate) fn as_mut_slice(&mut self) -> &mut [u8] {
        self.inner.as_mut_slice()
    }
}

pub(crate) fn remove_failed_output(path: &Path) {
    let _ = fs::remove_file(path);
}

pub(crate) fn paths_refer_to_same_file(first: &Path, second: &Path) -> Result<bool, Error> {
    let first = fs::canonicalize(first).map_err(ioe)?;
    match fs::canonicalize(second) {
        Ok(second) => Ok(first == second),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ioe(error)),
    }
}

#[cfg(unix)]
mod platform {
    use std::ffi::c_void;
    use std::fs::{File, OpenOptions};
    use std::os::fd::AsRawFd;
    use std::path::Path;
    use std::{io, ptr, slice};

    const PROT_READ: i32 = 1;
    const PROT_WRITE: i32 = 2;
    const MAP_SHARED: i32 = 1;

    unsafe extern "C" {
        fn mmap(
            address: *mut c_void,
            length: usize,
            protection: i32,
            flags: i32,
            fd: i32,
            offset: i64,
        ) -> *mut c_void;
        fn munmap(address: *mut c_void, length: usize) -> i32;
    }

    pub(super) struct Mapping {
        _file: File,
        pointer: *mut u8,
        length: usize,
        writable: bool,
    }

    impl Mapping {
        pub(super) fn open(path: &Path) -> io::Result<Self> {
            let file = File::open(path)?;
            let length = usize::try_from(file.metadata()?.len())
                .map_err(|_| io::Error::other("file is too large to map"))?;
            Self::map(file, length, false)
        }

        pub(super) fn create(path: &Path, length: usize) -> io::Result<Self> {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)?;
            file.set_len(length as u64)?;
            Self::map(file, length, true)
        }

        fn map(file: File, length: usize, writable: bool) -> io::Result<Self> {
            if length == 0 {
                return Ok(Self {
                    _file: file,
                    pointer: ptr::null_mut(),
                    length,
                    writable,
                });
            }
            let protection = PROT_READ | if writable { PROT_WRITE } else { 0 };
            // SAFETY: The file descriptor remains owned by the mapping and the
            // requested range is bounded by the file length.
            let pointer = unsafe {
                mmap(
                    ptr::null_mut(),
                    length,
                    protection,
                    MAP_SHARED,
                    file.as_raw_fd(),
                    0,
                )
            };
            if pointer as isize == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self {
                _file: file,
                pointer: pointer.cast(),
                length,
                writable,
            })
        }

        pub(super) fn as_slice(&self) -> &[u8] {
            if self.length == 0 {
                &[]
            } else {
                // SAFETY: The mapping is readable and remains alive for the slice.
                unsafe { slice::from_raw_parts(self.pointer, self.length) }
            }
        }

        pub(super) fn as_mut_slice(&mut self) -> &mut [u8] {
            assert!(self.writable);
            if self.length == 0 {
                &mut []
            } else {
                // SAFETY: The mapping is writable, uniquely borrowed, and alive.
                unsafe { slice::from_raw_parts_mut(self.pointer, self.length) }
            }
        }
    }

    impl Drop for Mapping {
        fn drop(&mut self) {
            if self.length != 0 {
                // SAFETY: This is the exact address and length returned by mmap.
                unsafe {
                    munmap(self.pointer.cast(), self.length);
                }
            }
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;
    use std::fs::{File, OpenOptions};
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;
    use std::{io, ptr, slice};

    type Handle = *mut c_void;
    const PAGE_READONLY: u32 = 0x02;
    const PAGE_READWRITE: u32 = 0x04;
    const FILE_MAP_WRITE: u32 = 0x0002;
    const FILE_MAP_READ: u32 = 0x0004;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileMappingW(
            file: Handle,
            attributes: *mut c_void,
            protection: u32,
            maximum_size_high: u32,
            maximum_size_low: u32,
            name: *const u16,
        ) -> Handle;
        fn MapViewOfFile(
            mapping: Handle,
            access: u32,
            offset_high: u32,
            offset_low: u32,
            bytes: usize,
        ) -> *mut c_void;
        fn UnmapViewOfFile(address: *const c_void) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }

    pub(super) struct Mapping {
        _file: File,
        mapping: Handle,
        pointer: *mut u8,
        length: usize,
        writable: bool,
    }

    impl Mapping {
        pub(super) fn open(path: &Path) -> io::Result<Self> {
            let file = File::open(path)?;
            let length = usize::try_from(file.metadata()?.len())
                .map_err(|_| io::Error::other("file is too large to map"))?;
            Self::map(file, length, false)
        }

        pub(super) fn create(path: &Path, length: usize) -> io::Result<Self> {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)?;
            file.set_len(length as u64)?;
            Self::map(file, length, true)
        }

        fn map(file: File, length: usize, writable: bool) -> io::Result<Self> {
            if length == 0 {
                return Ok(Self {
                    _file: file,
                    mapping: ptr::null_mut(),
                    pointer: ptr::null_mut(),
                    length,
                    writable,
                });
            }
            let protection = if writable {
                PAGE_READWRITE
            } else {
                PAGE_READONLY
            };
            // SAFETY: The OS handle remains valid for the lifetime of the mapping.
            let mapping = unsafe {
                CreateFileMappingW(
                    file.as_raw_handle().cast(),
                    ptr::null_mut(),
                    protection,
                    0,
                    0,
                    ptr::null(),
                )
            };
            if mapping.is_null() {
                return Err(io::Error::last_os_error());
            }
            let access = if writable {
                FILE_MAP_WRITE
            } else {
                FILE_MAP_READ
            };
            // SAFETY: The mapping handle is valid and zero length maps the file.
            let pointer = unsafe { MapViewOfFile(mapping, access, 0, 0, 0) };
            if pointer.is_null() {
                // SAFETY: mapping was returned by CreateFileMappingW.
                unsafe {
                    CloseHandle(mapping);
                }
                return Err(io::Error::last_os_error());
            }
            Ok(Self {
                _file: file,
                mapping,
                pointer: pointer.cast(),
                length,
                writable,
            })
        }

        pub(super) fn as_slice(&self) -> &[u8] {
            if self.length == 0 {
                &[]
            } else {
                // SAFETY: The mapped view is readable and remains alive.
                unsafe { slice::from_raw_parts(self.pointer, self.length) }
            }
        }

        pub(super) fn as_mut_slice(&mut self) -> &mut [u8] {
            assert!(self.writable);
            if self.length == 0 {
                &mut []
            } else {
                // SAFETY: The view is writable, uniquely borrowed, and alive.
                unsafe { slice::from_raw_parts_mut(self.pointer, self.length) }
            }
        }
    }

    impl Drop for Mapping {
        fn drop(&mut self) {
            if !self.pointer.is_null() {
                // SAFETY: pointer was returned by MapViewOfFile.
                unsafe {
                    UnmapViewOfFile(self.pointer.cast());
                }
            }
            if !self.mapping.is_null() {
                // SAFETY: mapping was returned by CreateFileMappingW.
                unsafe {
                    CloseHandle(self.mapping);
                }
            }
        }
    }
}
