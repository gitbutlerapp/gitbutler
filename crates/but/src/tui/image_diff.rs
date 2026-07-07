//! Loading and decoding of changed image files so TUIs can render them as pictures
//! instead of a "binary file" placeholder.

use std::{
    hash::{DefaultHasher, Hash as _, Hasher as _},
    sync::{Arc, Mutex},
};

use bstr::{BStr, ByteSlice};
use but_core::ui::{ChangeState, TreeChange, TreeStatus};
use gix::object::tree::EntryKind;
use image::DynamicImage;

/// Images larger than this are not decoded to keep memory use and render latency in check.
const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;

/// Decoded images larger than this on either axis are downscaled once at decode time,
/// off the UI thread, so per-frame resizing and terminal encoding work on fewer pixels.
const MAX_DECODED_EDGE: u32 = 1600;

/// How many decoded image diffs to keep, so scrolling through commits never decodes the
/// same image twice.
const DECODE_CACHE_SIZE: usize = 16;

/// Identity of one side of an image change for the decode cache.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SideId {
    Absent,
    Blob(gix::ObjectId),
    /// A worktree file, identified by its stat data so edits invalidate the entry.
    Worktree {
        path: bstr::BString,
        len: u64,
        modified: std::time::SystemTime,
    },
}

/// Cache key for one decoded image diff.
type DecodeCacheKey = (SideId, SideId);

/// Decoded image diffs, oldest first.
static DECODE_CACHE: Mutex<Vec<(DecodeCacheKey, Arc<ImageDiffData>)>> = Mutex::new(Vec::new());

/// Both sides of an image change, decoded and ready to render.
#[derive(Debug, Clone)]
pub(crate) struct ImageDiffData {
    /// The previous image, absent for additions.
    pub old: Option<DecodedImage>,
    /// The current image, absent for deletions.
    pub new: Option<DecodedImage>,
}

/// One decoded image plus the metadata shown in its caption.
#[derive(Debug, Clone)]
pub(crate) struct DecodedImage {
    /// Possibly downscaled from the file's native size for cheaper rendering.
    pub image: DynamicImage,
    /// Native width of the image file, for captions and layout.
    pub width: u32,
    /// Native height of the image file, for captions and layout.
    pub height: u32,
    pub byte_size: usize,
    /// Identifies the image content across renders, to cache terminal render state.
    pub fingerprint: u64,
}

/// Return `true` if `path` has an extension of an image format we can decode.
pub(crate) fn is_image_path(path: &[u8]) -> bool {
    let Some((_, ext)) = path.rsplit_once_str(b".") else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_slice(),
        b"png" | b"jpg" | b"jpeg" | b"gif" | b"webp" | b"bmp" | b"ico" | b"tif" | b"tiff"
    )
}

/// Decode the old and new images of `change`, or `None` if no side could be decoded.
pub(crate) fn from_change(
    repo: &gix::Repository,
    change: &TreeChange,
) -> Option<Arc<ImageDiffData>> {
    let cache_key = decode_cache_key(repo, change);
    if let Some(key) = &cache_key
        && let Ok(cache) = DECODE_CACHE.lock()
        && let Some((_, data)) = cache.iter().find(|(cached_key, _)| cached_key == key)
    {
        return Some(Arc::clone(data));
    }

    let path = change.path_bytes.as_bstr();
    let (old, new) = match &change.status {
        TreeStatus::Addition { state, .. } => (None, load_side(repo, state, path)),
        TreeStatus::Deletion { previous_state } => (load_side(repo, previous_state, path), None),
        TreeStatus::Modification {
            previous_state,
            state,
            ..
        } => (
            load_side(repo, previous_state, path),
            load_side(repo, state, path),
        ),
        TreeStatus::Rename {
            previous_path_bytes,
            previous_state,
            state,
            ..
        } => (
            load_side(repo, previous_state, previous_path_bytes.as_bstr()),
            load_side(repo, state, path),
        ),
    };
    let data = (old.is_some() || new.is_some()).then(|| Arc::new(ImageDiffData { old, new }))?;

    if let Some(key) = cache_key
        && let Ok(mut cache) = DECODE_CACHE.lock()
    {
        if cache.len() >= DECODE_CACHE_SIZE {
            cache.remove(0);
        }
        cache.push((key, Arc::clone(&data)));
    }
    Some(data)
}

/// The decode-cache key for `change`, or `None` when a side cannot be identified.
/// Blob sides are identified by their id; worktree sides (null id) by the file's stat
/// data, so editing the file invalidates the cached decode.
fn decode_cache_key(repo: &gix::Repository, change: &TreeChange) -> Option<DecodeCacheKey> {
    let path = change.path_bytes.as_bstr();
    match &change.status {
        TreeStatus::Addition { state, .. } => Some((SideId::Absent, side_id(repo, state, path)?)),
        TreeStatus::Deletion { previous_state } => {
            Some((side_id(repo, previous_state, path)?, SideId::Absent))
        }
        TreeStatus::Modification {
            previous_state,
            state,
            ..
        } => Some((
            side_id(repo, previous_state, path)?,
            side_id(repo, state, path)?,
        )),
        TreeStatus::Rename {
            previous_path_bytes,
            previous_state,
            state,
            ..
        } => Some((
            side_id(repo, previous_state, previous_path_bytes.as_bstr())?,
            side_id(repo, state, path)?,
        )),
    }
}

fn side_id(repo: &gix::Repository, state: &ChangeState, path: &BStr) -> Option<SideId> {
    if !state.id.is_null() {
        return Some(SideId::Blob(state.id));
    }
    let metadata = std::fs::metadata(repo.workdir()?.join(gix::path::from_bstr(path))).ok()?;
    Some(SideId::Worktree {
        path: path.to_owned(),
        len: metadata.len(),
        modified: metadata.modified().ok()?,
    })
}

fn load_side(repo: &gix::Repository, state: &ChangeState, path: &BStr) -> Option<DecodedImage> {
    if !matches!(state.kind, EntryKind::Blob | EntryKind::BlobExecutable) {
        return None;
    }
    let bytes = if state.id.is_null() {
        let workdir = repo.workdir()?;
        std::fs::read(workdir.join(gix::path::from_bstr(path))).ok()?
    } else {
        repo.find_blob(state.id).ok()?.detach().data
    };
    if bytes.len() > MAX_IMAGE_BYTES {
        return None;
    }
    let image = image::load_from_memory(&bytes).ok()?;
    let (width, height) = (image.width(), image.height());
    let image = if width > MAX_DECODED_EDGE || height > MAX_DECODED_EDGE {
        image.resize(
            MAX_DECODED_EDGE,
            MAX_DECODED_EDGE,
            image::imageops::FilterType::Triangle,
        )
    } else {
        image
    };
    let fingerprint = {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    };
    Some(DecodedImage {
        image,
        width,
        height,
        byte_size: bytes.len(),
        fingerprint,
    })
}

/// Format a byte count for image captions, e.g. `12.3 KiB`.
pub(crate) fn human_bytes(bytes: usize) -> String {
    let bytes = bytes as f64;
    if bytes < 1024.0 {
        format!("{bytes} B")
    } else if bytes < 1024.0 * 1024.0 {
        format!("{:.1} KiB", bytes / 1024.0)
    } else {
        format!("{:.1} MiB", bytes / (1024.0 * 1024.0))
    }
}
