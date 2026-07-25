use rustix::fs::{FileType, Mode, OFlags, fstat, open};
use rustix::io::Errno;
use std::fs::File;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub struct DirectoryEntry {
    pub name: Vec<u8>,
    pub kind: i32,
    pub size: u64,
}

pub const ENTRY_FILE: i32 = 0;
pub const ENTRY_DIRECTORY: i32 = 1;
pub const ENTRY_OTHER: i32 = 2;

pub fn list_directory(path: &Path) -> Result<Vec<DirectoryEntry>, String> {
    let entries = std::fs::read_dir(path)
        .map_err(|error| format!("cannot list directory {}: {error}", path.display()))?;
    let mut result = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let Ok(metadata) = std::fs::symlink_metadata(entry.path()) else {
            continue;
        };
        result.push(DirectoryEntry {
            name: entry.file_name().as_bytes().to_vec(),
            kind: if metadata.is_file() {
                ENTRY_FILE
            } else if metadata.is_dir() {
                ENTRY_DIRECTORY
            } else {
                ENTRY_OTHER
            },
            size: if metadata.is_file() { metadata.len() } else { 0 },
        });
    }
    Ok(result)
}

pub fn read_regular(
    path: &Path,
    minimum_bytes: u64,
    maximum_bytes: u64,
    label: &str,
) -> Result<Option<Vec<u8>>, String> {
    let descriptor = match open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    ) {
        Ok(descriptor) => descriptor,
        Err(Errno::NOENT) => return Ok(None),
        Err(error) => return Err(format!("cannot open {label} {}: {error}", path.display())),
    };
    let metadata = fstat(&descriptor)
        .map_err(|error| format!("cannot inspect {label} {}: {error}", path.display()))?;
    let size = u64::try_from(metadata.st_size).unwrap_or(u64::MAX);
    if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile
        || size < minimum_bytes
        || size > maximum_bytes
    {
        return Err(format!(
            "{label} must be a regular file between {minimum_bytes} and {maximum_bytes} bytes: {}",
            path.display()
        ));
    }
    let length = usize::try_from(size)
        .map_err(|_| format!("{label} is too large to read: {}", path.display()))?;
    let mut data = Vec::new();
    data.try_reserve_exact(length)
        .map_err(|_| format!("cannot allocate {label} {}", path.display()))?;
    data.resize(length, 0);
    File::from(descriptor)
        .read_exact(&mut data)
        .map_err(|error| format!("cannot read {label} {}: {error}", path.display()))?;
    Ok(Some(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fixture_directory;
    use std::os::unix::fs::symlink;

    #[test]
    fn reads_only_bounded_regular_files_without_following_links() {
        let directory = fixture_directory("regular-file");
        let path = directory.join("fixture.tsv");
        let link = directory.join("fixture-link.tsv");
        std::fs::write(&path, b"alpha\tbeta\n").unwrap();
        symlink(&path, &link).unwrap();

        assert_eq!(
            read_regular(&path, 1, 32, "fixture").unwrap().unwrap(),
            b"alpha\tbeta\n"
        );
        assert!(read_regular(&path, 32, 64, "fixture").is_err());
        assert!(read_regular(&path, 1, 4, "fixture").is_err());
        assert!(read_regular(&link, 1, 32, "fixture").is_err());
        assert!(
            read_regular(&directory.join("missing"), 1, 32, "fixture")
                .unwrap()
                .is_none()
        );

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn lists_entries_with_lstat_kinds_and_sizes() {
        let directory = fixture_directory("list-directory");
        std::fs::write(directory.join("track.ogg"), b"ogg").unwrap();
        std::fs::create_dir(directory.join("nested")).unwrap();
        symlink(directory.join("track.ogg"), directory.join("linked.ogg")).unwrap();

        let mut entries = list_directory(&directory).unwrap();
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        let summary = entries
            .iter()
            .map(|entry| (entry.name.as_slice(), entry.kind, entry.size))
            .collect::<Vec<_>>();
        assert_eq!(
            summary,
            vec![
                (b"linked.ogg".as_slice(), ENTRY_OTHER, 0),
                (b"nested".as_slice(), ENTRY_DIRECTORY, 0),
                (b"track.ogg".as_slice(), ENTRY_FILE, 3),
            ]
        );
        assert!(list_directory(&directory.join("missing")).is_err());

        std::fs::remove_dir_all(directory).unwrap();
    }
}
