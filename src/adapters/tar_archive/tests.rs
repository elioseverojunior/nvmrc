use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::GzEncoder;
use lzma_rust2::{XzOptions, XzWriter};

use super::*;

/// A `.tar.gz` of `(path, contents, mode)` files and `(path, target)`
/// symlinks (which come first), written to `directory/archive.tar.gz`.
fn build(directory: &Path, files: &[(&str, &str, u32)], links: &[(&str, &str)]) -> PathBuf {
    let path = directory.join("archive.tar.gz");
    let mut builder = tar::Builder::new(GzEncoder::new(
        File::create(&path).unwrap(),
        Compression::default(),
    ));
    for (name, target) in links {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        builder.append_link(&mut header, name, target).unwrap();
    }
    for (name, contents, mode) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(*mode);
        header.set_cksum();
        builder
            .append_data(&mut header, name, contents.as_bytes())
            .unwrap();
    }
    builder
        .into_inner()
        .unwrap()
        .finish()
        .unwrap()
        .flush()
        .unwrap();
    path
}

#[test]
fn it_unpacks_files_into_the_destination() {
    let root = tempfile::tempdir().unwrap();
    let archive = build(
        root.path(),
        &[
            ("node-v1/bin/node", "binary", 0o755),
            ("node-v1/README", "hi", 0o644),
        ],
        &[],
    );
    let destination = root.path().join("out");
    fs::create_dir(&destination).unwrap();
    TarArchive.extract(&archive, &destination).unwrap();
    let node = destination.join("node-v1/bin/node");
    assert_eq!(fs::read_to_string(&node).unwrap(), "binary");
    assert_eq!(
        fs::read_to_string(destination.join("node-v1/README")).unwrap(),
        "hi"
    );
}

#[cfg(unix)]
#[test]
fn it_keeps_the_execute_bit_and_relative_symlinks() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let archive = build(
        root.path(),
        &[
            ("node-v1/bin/node", "x", 0o755),
            ("node-v1/lib/npm-cli.js", "y", 0o644),
        ],
        &[("node-v1/bin/npm", "../lib/npm-cli.js")],
    );
    let destination = root.path().join("out");
    fs::create_dir(&destination).unwrap();
    TarArchive.extract(&archive, &destination).unwrap();
    let mode = fs::metadata(destination.join("node-v1/bin/node"))
        .unwrap()
        .permissions()
        .mode();
    assert_ne!(mode & 0o111, 0);
    let npm = destination.join("node-v1/bin/npm");
    assert_eq!(fs::read_to_string(npm).unwrap(), "y");
}

#[test]
fn a_missing_archive_is_an_error() {
    let root = tempfile::tempdir().unwrap();
    let error = TarArchive
        .extract(&root.path().join("nope.tar.gz"), root.path())
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}

#[test]
fn something_that_is_not_gzip_is_an_error() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("bad.tar.gz");
    fs::write(&archive, "not an archive").unwrap();
    assert!(TarArchive.extract(&archive, root.path()).is_err());
}

#[test]
fn an_entry_that_climbs_out_of_the_destination_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("evil.tar.gz");
    // `tar::Builder` refuses `..`, so the entry is written by hand.
    let mut header = tar::Header::new_gnu();
    header.set_size(1);
    header.set_mode(0o644);
    let name = b"../escaped";
    header.as_old_mut().name[..name.len()].copy_from_slice(name);
    header.set_cksum();
    let mut encoder = GzEncoder::new(File::create(&archive).unwrap(), Compression::default());
    encoder.write_all(header.as_bytes()).unwrap();
    let mut block = [0_u8; 512];
    block[0] = b'x';
    encoder.write_all(&block).unwrap();
    encoder.write_all(&[0_u8; 1024]).unwrap();
    encoder.finish().unwrap();
    let destination = root.path().join("out");
    fs::create_dir(&destination).unwrap();
    let error = TarArchive.extract(&archive, &destination).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("unsafe path in archive"));
    assert!(!root.path().join("escaped").exists());
}

#[cfg(unix)]
#[test]
fn a_file_written_through_a_symlink_that_leaves_the_destination_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let outside = root.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let archive = build(
        root.path(),
        &[("top/link/stolen", "x", 0o644)],
        &[("top/link", outside.to_str().unwrap())],
    );
    let destination = root.path().join("out");
    fs::create_dir(&destination).unwrap();
    let result = TarArchive.extract(&archive, &destination);
    assert!(result.is_err());
    assert!(!outside.join("stolen").exists());
}

/// A `.tar.xz` of `(path, contents)` files, written to
/// `directory/archive.tar.xz`.
fn build_xz(directory: &Path, files: &[(&str, &str)]) -> PathBuf {
    let path = directory.join("archive.tar.xz");
    let writer = XzWriter::new(File::create(&path).unwrap(), XzOptions::with_preset(1)).unwrap();
    let mut builder = tar::Builder::new(writer);
    for (name, contents) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, name, contents.as_bytes())
            .unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap();
    path
}

#[test]
fn it_unpacks_an_xz_compressed_tar() {
    let root = tempfile::tempdir().unwrap();
    let archive = build_xz(
        root.path(),
        &[("node-v1/bin/node", "binary"), ("node-v1/README", "hi")],
    );
    let destination = root.path().join("out");
    fs::create_dir(&destination).unwrap();
    TarArchive.extract(&archive, &destination).unwrap();
    let node = destination.join("node-v1/bin/node");
    assert_eq!(fs::read_to_string(node).unwrap(), "binary");
    assert_eq!(
        fs::read_to_string(destination.join("node-v1/README")).unwrap(),
        "hi"
    );
}

#[test]
fn the_compression_is_told_by_the_bytes_not_by_the_name() {
    let root = tempfile::tempdir().unwrap();
    let archive = build(root.path(), &[("top/file", "x", 0o644)], &[]);
    let renamed = root.path().join("misnamed.tar.xz");
    fs::rename(&archive, &renamed).unwrap();
    let destination = root.path().join("out");
    fs::create_dir(&destination).unwrap();
    TarArchive.extract(&renamed, &destination).unwrap();
    assert!(destination.join("top/file").exists());
}

#[test]
fn something_that_is_not_xz_is_an_error() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("bad.tar.xz");
    fs::write(&archive, "not an archive").unwrap();
    let error = TarArchive.extract(&archive, root.path()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn a_truncated_xz_archive_is_an_error() {
    let root = tempfile::tempdir().unwrap();
    let archive = build_xz(root.path(), &[("top/file", &"x".repeat(5000))]);
    let bytes = fs::read(&archive).unwrap();
    fs::write(&archive, &bytes[..bytes.len() / 2]).unwrap();
    let destination = root.path().join("out");
    fs::create_dir(&destination).unwrap();
    assert!(TarArchive.extract(&archive, &destination).is_err());
}

/// The CRC-32 (IEEE) that guards an xz block header.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

/// Rewrites the LZMA2 dictionary size the first block header declares (the
/// property byte after filter id 0x21 and its size 1) and fixes its CRC.
fn declare_dictionary(archive: &Path, property: u8) {
    const BLOCK: usize = 12; // the xz stream header comes first
    let mut bytes = fs::read(archive).unwrap();
    let length = (usize::from(bytes[BLOCK]) + 1) * 4;
    let header = BLOCK..BLOCK + length - 4;
    let filter = bytes[header.clone()]
        .windows(2)
        .position(|pair| pair == [0x21, 0x01])
        .unwrap();
    bytes[BLOCK + filter + 2] = property;
    let crc = crc32(&bytes[header.clone()]).to_le_bytes();
    bytes[header.end..header.end + 4].copy_from_slice(&crc);
    fs::write(archive, bytes).unwrap();
}

#[test]
fn an_xz_archive_that_needs_too_much_memory_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let archive = build_xz(root.path(), &[("top/file", "x")]);
    declare_dictionary(&archive, 36); // a 1 GiB dictionary
    let destination = root.path().join("out");
    fs::create_dir(&destination).unwrap();
    let error = TarArchive.extract(&archive, &destination).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::OutOfMemory);
    assert!(!destination.join("top/file").exists());
}

#[test]
fn an_xz_archive_with_a_node_sized_dictionary_still_unpacks() {
    let root = tempfile::tempdir().unwrap();
    let archive = build_xz(root.path(), &[("top/file", "x")]);
    declare_dictionary(&archive, 28); // 64 MiB, as `xz -9` (and Node) uses
    let destination = root.path().join("out");
    fs::create_dir(&destination).unwrap();
    TarArchive.extract(&archive, &destination).unwrap();
    assert!(destination.join("top/file").exists());
}
