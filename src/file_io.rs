use crate::Error;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMPORARY_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn ioe(error: std::io::Error) -> Error {
    Error::Io(error.to_string())
}

pub(crate) fn write_all_atomic(path: &Path, data: &[u8]) -> Result<(), Error> {
    let (temporary_path, mut file) = create_temporary_file(path)?;
    let result = (|| {
        file.write_all(data).map_err(ioe)?;
        file.sync_all().map_err(ioe)?;
        drop(file);
        platform::replace_file(&temporary_path, path).map_err(ioe)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
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
    fn from_file(file: File, len: usize) -> Result<Self, Error> {
        Ok(Self {
            inner: platform::Mapping::from_file(file, len, true).map_err(ioe)?,
        })
    }

    pub(crate) fn as_mut_slice(&mut self) -> &mut [u8] {
        self.inner.as_mut_slice()
    }

    fn flush(&self) -> Result<(), Error> {
        self.inner.flush().map_err(ioe)
    }
}

pub(crate) struct AtomicWriteMap {
    destination: PathBuf,
    temporary_path: PathBuf,
    mapping: Option<WriteMap>,
}

impl AtomicWriteMap {
    pub(crate) fn create(destination: &Path, len: usize) -> Result<Self, Error> {
        let (temporary_path, file) = create_temporary_file(destination)?;
        let result = (|| {
            file.set_len(len as u64).map_err(ioe)?;
            WriteMap::from_file(file, len)
        })();
        match result {
            Ok(mapping) => Ok(Self {
                destination: destination.to_owned(),
                temporary_path,
                mapping: Some(mapping),
            }),
            Err(error) => {
                let _ = fs::remove_file(&temporary_path);
                Err(error)
            }
        }
    }

    pub(crate) fn as_mut_slice(&mut self) -> &mut [u8] {
        self.mapping
            .as_mut()
            .expect("atomic output has not been committed")
            .as_mut_slice()
    }

    pub(crate) fn commit(mut self) -> Result<(), Error> {
        self.mapping
            .as_ref()
            .expect("atomic output has not been committed")
            .flush()?;
        self.mapping.take();
        platform::replace_file(&self.temporary_path, &self.destination).map_err(ioe)?;
        self.temporary_path.clear();
        Ok(())
    }
}

impl Drop for AtomicWriteMap {
    fn drop(&mut self) {
        self.mapping.take();
        if !self.temporary_path.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.temporary_path);
        }
    }
}

fn create_temporary_file(destination: &Path) -> Result<(PathBuf, File), Error> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let file_name = destination
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("xca-output"));
    for _ in 0..128 {
        let counter = TEMPORARY_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut temporary_name = OsString::from(".");
        temporary_name.push(file_name);
        temporary_name.push(format!(".xca-tmp-{}-{counter}", std::process::id()));
        let temporary_path = parent.join(temporary_name);
        match fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => return Ok((temporary_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(ioe(error)),
        }
    }
    Err(Error::Io(
        "could not create a unique temporary output file".to_owned(),
    ))
}

pub(crate) fn paths_refer_to_same_file(first: &Path, second: &Path) -> Result<bool, Error> {
    let first_path = fs::canonicalize(first).map_err(ioe)?;
    match fs::canonicalize(second) {
        Ok(second_path) => {
            if first_path == second_path {
                return Ok(true);
            }
            platform::same_file(first, second).map_err(ioe)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ioe(error)),
    }
}

#[cfg(unix)]
mod platform {
    use std::ffi::c_void;
    use std::fs::{self, File};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;
    use std::{io, ptr, slice};

    const PROT_READ: i32 = 1;
    const PROT_WRITE: i32 = 2;
    const MAP_SHARED: i32 = 1;
    const MS_SYNC: i32 = 4;

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
        fn msync(address: *mut c_void, length: usize, flags: i32) -> i32;
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
            Self::from_file(file, length, false)
        }

        pub(super) fn from_file(file: File, length: usize, writable: bool) -> io::Result<Self> {
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

        pub(super) fn flush(&self) -> io::Result<()> {
            if self.length != 0 {
                // SAFETY: This is a valid shared writable mapping.
                if unsafe { msync(self.pointer.cast(), self.length, MS_SYNC) } != 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            self._file.sync_all()
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

    pub(super) fn same_file(first: &Path, second: &Path) -> io::Result<bool> {
        let first = fs::metadata(first)?;
        let second = fs::metadata(second)?;
        Ok(first.dev() == second.dev() && first.ino() == second.ino())
    }

    pub(super) fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
        fs::rename(source, destination)
    }
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;
    use std::fs::File;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;
    use std::{io, ptr, slice};

    type Handle = *mut c_void;
    const PAGE_READONLY: u32 = 0x02;
    const PAGE_READWRITE: u32 = 0x04;
    const FILE_MAP_WRITE: u32 = 0x0002;
    const FILE_MAP_READ: u32 = 0x0004;
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x0001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0008;

    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[repr(C)]
    struct ByHandleFileInformation {
        attributes: u32,
        creation_time: FileTime,
        last_access_time: FileTime,
        last_write_time: FileTime,
        volume_serial_number: u32,
        file_size_high: u32,
        file_size_low: u32,
        number_of_links: u32,
        file_index_high: u32,
        file_index_low: u32,
    }

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
        fn FlushViewOfFile(address: *const c_void, bytes: usize) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
        fn GetFileInformationByHandle(
            file: Handle,
            information: *mut ByHandleFileInformation,
        ) -> i32;
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
            Self::from_file(file, length, false)
        }

        pub(super) fn from_file(file: File, length: usize, writable: bool) -> io::Result<Self> {
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

        pub(super) fn flush(&self) -> io::Result<()> {
            if self.length != 0 {
                // SAFETY: This is a valid mapped view.
                if unsafe { FlushViewOfFile(self.pointer.cast(), self.length) } == 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            self._file.sync_all()
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

    pub(super) fn same_file(first: &Path, second: &Path) -> io::Result<bool> {
        fn identity(path: &Path) -> io::Result<(u32, u64)> {
            let file = File::open(path)?;
            let mut information = std::mem::MaybeUninit::<ByHandleFileInformation>::uninit();
            // SAFETY: `information` points to writable storage and the file handle is valid.
            if unsafe {
                GetFileInformationByHandle(file.as_raw_handle().cast(), information.as_mut_ptr())
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: GetFileInformationByHandle initialized the structure on success.
            let information = unsafe { information.assume_init() };
            let file_index = (u64::from(information.file_index_high) << 32)
                | u64::from(information.file_index_low);
            Ok((information.volume_serial_number, file_index))
        }

        Ok(identity(first)? == identity(second)?)
    }

    pub(super) fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
        let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
        let destination: Vec<u16> = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        // SAFETY: Both paths are valid, null-terminated UTF-16 strings.
        if unsafe {
            MoveFileExW(
                source.as_ptr(),
                destination.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}
