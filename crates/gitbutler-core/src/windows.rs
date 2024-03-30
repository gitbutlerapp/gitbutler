use std::os::windows::fs::MetadataExt;

pub trait MetadataShim {
    fn ino(&self) -> u64;
    fn dev(&self) -> u64;
    fn uid(&self) -> u32;
    fn gid(&self) -> u32;
}

impl MetadataShim for std::fs::Metadata {
    fn ino(&self) -> u64 {
        self.file_index().expect("file metadata constructed based on directory listing instead of a file (see https://doc.rust-lang.org/std/os/windows/fs/trait.MetadataExt.html#tymethod.file_index)")
    }
    #[allow(clippy::cast_lossless)]
    fn dev(&self) -> u64 {
        self.volume_serial_number().expect("file metadata constructed based on directory listing instead of a file (see https://doc.rust-lang.org/std/os/windows/fs/trait.MetadataExt.html#tymethod.volume_serial_number)") as u64
    }
    fn uid(&self) -> u32 {
        0
    }
    fn gid(&self) -> u32 {
        0
    }
}
