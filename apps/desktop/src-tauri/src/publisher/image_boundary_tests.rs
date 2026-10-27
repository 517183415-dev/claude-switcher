use super::*;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

// Synthetic, uniformly gray JPEGs with a synthetic EXIF marker. They contain
// 2000x1000 and 2002x1000 pixels respectively and are both below 1 MiB.
const AT_LIMIT: &[u8] = include_bytes!("image_test_data/jpeg-at-limit.jpg");
const OVER_LIMIT: &[u8] = include_bytes!("image_test_data/jpeg-over-limit.jpg");
const METADATA: &[u8] = b"synthetic-private-metadata";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Root(PathBuf);
impl Root {
    fn create(path: PathBuf) -> std::io::Result<Self> {
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder.create(&path)?;
        Ok(Self(path))
    }
    fn new() -> Self {
        let temp = std::env::temp_dir().canonicalize().unwrap();
        loop {
            let path = temp.join(format!(
                "publisher-image-boundary-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match Self::create(path) {
                Ok(root) => return root,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create private image test directory: {error}"),
            }
        }
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn open(root: &Root) -> Publisher {
    Publisher::open(&root.0, "local-ad-key", None, 1000).unwrap()
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn visit(base: &Path, current: &Path, result: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let relative = path.strip_prefix(base).unwrap().to_owned();
            if entry.file_type().unwrap().is_dir() {
                result.insert(relative, None);
                visit(base, &path, result);
            } else {
                result.insert(relative, Some(fs::read(path).unwrap()));
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

#[test]
fn jpeg_at_pixel_limit_is_normalized_and_metadata_is_removed() {
    assert!(AT_LIMIT.len() < ads::MAX_IMAGE_BYTES);
    assert!(
        AT_LIMIT
            .windows(METADATA.len())
            .any(|window| window == METADATA)
    );
    let root = Root::new();
    let mut publisher = open(&root);
    let asset = publisher.import_image(AT_LIMIT, "image/jpeg").unwrap();
    assert_eq!((asset.width, asset.height), (2000, 1000));
    assert_eq!(
        u64::from(asset.width) * u64::from(asset.height),
        ads::MAX_IMAGE_PIXELS
    );
    let bytes = publisher.image_bytes(&asset).unwrap();
    ads::validate_image(&bytes, &asset).unwrap();
    assert!(
        !bytes
            .windows(METADATA.len())
            .any(|window| window == METADATA)
    );
}

#[test]
fn invalid_jpeg_inputs_leave_publisher_and_files_unchanged() {
    assert!(OVER_LIMIT.len() < ads::MAX_IMAGE_BYTES);
    let root = Root::new();
    let mut publisher = open(&root);
    let valid = publisher.import_image(AT_LIMIT, "image/jpeg").unwrap();
    let before = snapshot(&root.0);
    let assets = publisher.state.assets.clone();
    for (bytes, mime, expected) in [
        (OVER_LIMIT, "image/jpeg", "image_dimensions"),
        (&AT_LIMIT[..12], "image/jpeg", "image_header"),
        (AT_LIMIT, "image/png", "image_mime"),
        (&b"\xff\xd8\xffinvalid"[..], "image/jpeg", "image_header"),
    ] {
        assert!(
            matches!(publisher.import_image(bytes, mime), Err(Error::Invalid(reason)) if reason == expected)
        );
        assert_eq!(publisher.state.assets, assets);
        assert_eq!(snapshot(&root.0), before);
        assert_eq!(
            publisher.image_bytes(&valid).unwrap().len(),
            valid.bytes as usize
        );
    }
}

#[test]
fn preexisting_temp_directory_is_never_taken_or_removed() {
    let parent = Root::new();
    assert_eq!(
        fs::metadata(&parent.0).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let candidate = parent.0.join("existing");
    fs::create_dir(&candidate).unwrap();
    fs::write(candidate.join("sentinel"), b"preserve").unwrap();
    assert!(
        matches!(Root::create(candidate.clone()), Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists)
    );
    assert_eq!(fs::read(candidate.join("sentinel")).unwrap(), b"preserve");
}
