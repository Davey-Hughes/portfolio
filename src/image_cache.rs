//! Image caching and processing utilities
//!
//! This module handles:
//! - Processing images and converting to WebP
//! - Caching processed images
//! - Cleaning up orphaned cache files
//! - Pre-warming the cache on startup

use leptos::logging::log;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// How many images may be decoded and transcoded at once: the startup pre-warm's
/// workers, and separately the compressed endpoint's requests, so twice this at worst.
/// A transcode takes about 8 MB of memory per megapixel at its peak, about 2.2 GB for a
/// 283-megapixel photo, and about 10–35 ms of CPU per megapixel, a few seconds for the
/// largest. That is for an 8-bit JPEG: a 16-bit RGBA PNG of that size peaked at 4.7 GB,
/// and a JPEG XL takes several times its picture while it decodes (see
/// `MAX_DECODED_BYTES` and `MAX_JXL_WORKING_BYTES`).
pub fn transcode_limit() -> usize {
    std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .min(4)
}

/// One size of one image: its source file and the cache file made from it.
struct CacheEntry {
    source: PathBuf,
    /// Read before the source is decoded: a source replaced meanwhile then leaves a
    /// cache file that doesn't match it, which is made again next time.
    source_modified: Option<SystemTime>,
    cache_path: PathBuf,
}

impl CacheEntry {
    /// `image_path` is as for [`process_and_cache_image`].
    fn find(
        images_dir: &str,
        cache_dir: &str,
        image_path: &str,
        width: u32,
        quality: u8,
    ) -> Option<Self> {
        // Cut at the last dot. The extension only matters when no file is spelled in
        // lower or upper case (see `find_image_file`).
        let (path_without_ext, requested_ext) = match image_path.rsplit_once('.') {
            Some((path_without_ext, ext)) => (path_without_ext, Some(ext)),
            None => (image_path, None),
        };

        // Find the actual file with any supported extension
        let source = find_image_file(
            images_dir,
            path_without_ext,
            requested_ext,
            &SOURCE_EXTENSIONS,
        )?;

        // Cache filename without timestamp - makes cache persistent. The `_l1`
        // suffix is an encoder-version tag: it changed when we switched from
        // the `image` crate's lossless WebP encoder to libwebp lossy. Bumping
        // this string invalidates older cache entries automatically (they
        // don't match the new name; the orphan sweep removes them in time).
        let cache_filename = format!(
            "{}_w{}_q{}_l1.webp",
            path_without_ext.replace(['/', '\\'], "_"),
            width,
            quality
        );

        Some(Self {
            source_modified: source.metadata().ok().and_then(|m| m.modified().ok()),
            source,
            cache_path: PathBuf::from(cache_dir).join(&cache_filename),
        })
    }
}

/// How long a file has to be left alone before the server trusts it to be whole.
///
/// A photo exported in place passes through part-written states, and the `image` crate
/// decodes a JPEG that stops halfway without complaint. A size made from one would
/// carry the source's time, and if the export ends within the same whole second, that
/// is the finished file's time too: the size would count as fresh and go out under the
/// photo's address for a year. So a source modified less than this long ago is made
/// again for each request, not cached, and sent with `no-store`; so is an original.
///
/// One second covers that when the file server's clock agrees with this one; the rest
/// allows for its clock running up to nine seconds behind. For a time ahead of this
/// clock, see [`has_settled`]. The cost is a transcode per request in the first ten
/// seconds after a photo changes, and on the NFS share nobody asks for it that soon:
/// pages only show the new address once the photo lists see the change.
pub const SETTLE_TIME: std::time::Duration = std::time::Duration::from_secs(10);

/// How far ahead of this server's clock a file's time can be and still be taken for the
/// file server's own clock running fast, that is, for a write that may be going on now.
const FAST_CLOCK: std::time::Duration = std::time::Duration::from_mins(1);

/// Whether a file last modified at `modified` has been left alone for [`SETTLE_TIME`]
/// by `now`.
///
/// A time up to [`FAST_CLOCK`] ahead counts once `now` has passed it by the settle time.
/// A time further ahead wasn't set by a write happening now: it was set on purpose
/// after the file was written, copied from a camera card or a machine whose clock is
/// ahead (`cp -p`, `rsync -a`), or by `touch -d`. Such a file counts as settled at once,
/// like one whose time can't be read. Waiting for this clock to catch up could take
/// days, with every view of the photo transcoded again and sent with `no-store`.
pub fn has_settled(modified: SystemTime, now: SystemTime) -> bool {
    match now.duration_since(modified) {
        Ok(age) => age >= SETTLE_TIME,
        Err(ahead) => ahead.duration() > FAST_CLOCK,
    }
}

/// Whether a source whose time was `before` when it was found and `after` once it was
/// decoded can be trusted to have been read whole: the same both times, and settled.
///
/// The second reading matters on NFS, where the client keeps a file's attributes for up
/// to a minute, so `before` can be older than the file. Opening the file to decode it
/// fetches them again, so a write in progress shows in `after`.
fn source_settled(before: Option<SystemTime>, after: Option<SystemTime>, now: SystemTime) -> bool {
    match (before, after) {
        (Some(before), Some(after)) => before == after && has_settled(after, now),
        // A time that can't be read can't be checked; the cache check trusts such a
        // source too.
        _ => true,
    }
}

/// What the cache holds for one size of one image.
pub enum Lookup {
    /// There is no source image at that path.
    NoSource,
    /// The source, as it is now, failed to decode before, or the decode limits refused
    /// it, so no size can be made from it (see [`process_image`]).
    Undecodable,
    /// A WebP made from the source as it is now.
    Cached(Vec<u8>),
    /// Nothing made from the source as it is now.
    Missing,
}

/// One size of one image, as WebP.
pub struct Webp {
    pub data: Vec<u8>,
    /// False when the source may still have been being written as it was read, so
    /// `data` may be part of a picture. Such a WebP isn't cached, and nobody should
    /// keep it.
    pub settled: bool,
}

/// What the cache holds for one size of an image. Only reads; never transcodes.
///
/// `image_path` is as for [`process_and_cache_image`], which finds the same source.
pub fn cached_image(
    images_dir: &str,
    cache_dir: &str,
    image_path: &str,
    width: u32,
    quality: u8,
) -> Lookup {
    let Some(entry) = CacheEntry::find(images_dir, cache_dir, image_path, width, quality) else {
        return Lookup::NoSource;
    };
    match try_use_cached_image(entry.source_modified, &entry.cache_path) {
        Some(data) => Lookup::Cached(data),
        None if failed_before(&entry.source) => Lookup::Undecodable,
        None => Lookup::Missing,
    }
}

/// The formats the compressed endpoint makes its WebP files from, in order of
/// preference: it serves a photo from the first of these it has a file in.
///
/// Only formats this build can decode. The gallery also lists AVIF files, but the
/// `image` crate is built without an AVIF decoder (see its features in `Cargo.toml`),
/// so an AVIF source could only fail. Left out, a photo's other formats serve instead,
/// and one that exists only as AVIF has no compressed sizes. Enabling a decoder means
/// adding its format here too.
///
/// A GIF gives the first frame of its animation: the `image` crate's decoder reads that
/// frame and stops, so the work doesn't grow with the number of frames.
const SOURCE_EXTENSIONS: [&str; 6] = ["jpg", "jpeg", "jxl", "webp", "png", "gif"];

/// Process and cache a single image with the given parameters
/// Returns the WebP data or None if processing failed
///
/// `image_path` is the image's path under `images_dir`, extension included, as the
/// request names it. The source is found by the path without the extension (see
/// [`find_image_file`]), and the cache file is named after that too.
pub fn process_and_cache_image(
    images_dir: &str,
    cache_dir: &str,
    image_path: &str,
    width: u32,
    quality: u8,
) -> Option<Webp> {
    let entry = CacheEntry::find(images_dir, cache_dir, image_path, width, quality)?;

    // Check if a cached version exists and was made from the source as it is now
    if let Some(data) = try_use_cached_image(entry.source_modified, &entry.cache_path) {
        return Some(Webp {
            data,
            settled: true,
        });
    }

    // Process the image
    let data = process_image(&entry.source, width, quality)?;

    // Save to cache, unless the source may have been mid-write as it was read
    let modified_after = entry.source.metadata().ok().and_then(|m| m.modified().ok());
    let settled = source_settled(entry.source_modified, modified_after, SystemTime::now());
    if settled {
        save_to_cache(cache_dir, &entry.cache_path, &data, entry.source_modified);
    }

    Some(Webp { data, settled })
}

/// Find an image file with any of the supported extensions, refusing any
/// path that resolves outside `images_dir` (defends against `..`-style
/// traversal in the URL path).
///
/// `extensions` are in lower case, in order of preference. A file's extension may be
/// in any case, as cameras write `DSC_0001.JPG`. For each format this tries the
/// lower-case spelling and then the upper-case one, a `stat` each. A mixed spelling
/// such as `Jpg` has up to 16 forms, so rather than try them all it lists the folder,
/// and only when nothing above exists and the request names an image file that is
/// there (see [`lists_folder`]). A made-up name lists nothing. The site's addresses
/// carry each photo's real main file, so they all resolve.
///
/// The folder is resolved and checked first ([`image_folder`]), so nothing is looked
/// up in a folder outside `images_dir`. A file found is taken as it is when it is a
/// regular file there; a link is followed and must lead inside `images_dir` too.
///
/// Which file this picks never depends on the request, only whether it finds one:
/// lower case, then upper case, and a mixed spelling only if no format is spelled
/// either way; between mixed spellings, by format and then by name in byte order
/// (`x.JPg` before `x.Jpg`). `gallery::variant_order` puts a photo's files in the same
/// order, so the gallery's main file is the one the sizes are made from.
fn find_image_file(
    images_dir: &str,
    path_without_ext: &str,
    requested_ext: Option<&str>,
    extensions: &[&str],
) -> Option<PathBuf> {
    let images_root = std::path::PathBuf::from(images_dir).canonicalize().ok()?;
    let (folder, stem) = image_folder(&images_root, path_without_ext)?;
    let resolve = |name: &str| {
        // One `lstat`. canonicalize() takes a system call for every component of the
        // path, so it is only needed for a link.
        let candidate = folder.join(name);
        let metadata = std::fs::symlink_metadata(&candidate).ok()?;
        if metadata.is_file() {
            return Some(candidate);
        }
        if !metadata.is_symlink() {
            return None;
        }
        let resolved = candidate.canonicalize().ok()?;
        (resolved.starts_with(&images_root) && resolved.is_file()).then_some(resolved)
    };
    let found = extensions.iter().find_map(|ext| {
        [ext.to_ascii_lowercase(), ext.to_ascii_uppercase()]
            .into_iter()
            .find_map(|spelling| resolve(&format!("{stem}.{spelling}")))
    });
    if found.is_some() {
        return found;
    }

    if !lists_folder(&folder, stem, requested_ext, extensions) {
        return None;
    }
    let mut names: Vec<(usize, String)> = std::fs::read_dir(&folder)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let ext = name.strip_prefix(stem)?.strip_prefix('.')?;
            let format = extensions
                .iter()
                .position(|supported| supported.eq_ignore_ascii_case(ext))?;
            Some((format, name))
        })
        .collect();
    names.sort_unstable();
    names.into_iter().find_map(|(_, name)| resolve(&name))
}

/// The folder `path_without_ext` is in, resolved, and the name within it; `None` when
/// the folder isn't inside `images_root`, before anything in it is looked at. A
/// request can't learn from the lookup whether a file exists outside the images root,
/// though how long a folder takes to resolve may show whether that folder exists.
fn image_folder<'a>(images_root: &Path, path_without_ext: &'a str) -> Option<(PathBuf, &'a str)> {
    let (folder, stem) = path_without_ext
        .rsplit_once('/')
        .unwrap_or(("", path_without_ext));
    let folder = images_root.join(folder).canonicalize().ok()?;
    folder.starts_with(images_root).then_some((folder, stem))
}

/// Whether [`find_image_file`] lists `folder` for `stem`, when no file of that name is
/// spelled in lower or upper case.
///
/// Only for a request that names an image file which is there: a mixed spelling such
/// as `x.Jpg`, or an AVIF that the gallery made a photo's main file, whose other files
/// may be spelled any way. "An image" means one of `extensions` or `avif`, in any
/// case, so a folder (`home/.`), a `gallery.toml` or a made-up name lists nothing.
fn lists_folder(
    folder: &Path,
    stem: &str,
    requested_ext: Option<&str>,
    extensions: &[&str],
) -> bool {
    let Some(requested) = requested_ext else {
        return false;
    };
    let names_an_image = extensions
        .iter()
        .chain(&["avif"])
        .any(|ext| ext.eq_ignore_ascii_case(requested));
    names_an_image && folder.join(format!("{stem}.{requested}")).is_file()
}

/// How an extension is spelled: 0 for all lower case (`jpg`), 1 for all upper case
/// (`JPG`), 2 for a mix (`Jpg`). Between files whose names differ only in the case
/// of the extension, the lower rank wins, here and in the gallery.
pub(crate) fn extension_spelling_rank(ext: &str) -> u8 {
    if !ext.bytes().any(|b| b.is_ascii_uppercase()) {
        0
    } else if !ext.bytes().any(|b| b.is_ascii_lowercase()) {
        1
    } else {
        2
    }
}

/// The cached WebP at `cache_path`, if it was made from the source as it is now.
///
/// [`save_to_cache`] gives each cache file its source's modification time, so the
/// file is fresh only while the two match. A source replaced by a newer file or by an
/// older one (`cp -p`, `rsync -a`, a restore from backup) no longer matches, and
/// neither does a cache file written before the server did this. A newer-than-source
/// test would keep serving the old picture for an older replacement, now at the new
/// address the site builds from the older time. The times match in whole seconds, as
/// the version in that address does, so a cache directory that keeps coarser times
/// than the photos still matches, as long as it keeps them to the second. An empty
/// file never counts.
fn try_use_cached_image(source_modified: Option<SystemTime>, cache_path: &Path) -> Option<Vec<u8>> {
    use std::io::Read;

    // The check and the read go through one open file, so a file renamed into place
    // between the two can't be read under the other's check.
    let mut file = std::fs::File::open(cache_path).ok()?;
    let metadata = file.metadata().ok()?;
    if metadata.len() == 0 {
        return None;
    }

    // If we can't get timestamps, just use the cache if it exists
    if let (Some(source_time), Ok(cache_time)) = (source_modified, metadata.modified()) {
        let source_secs = whole_seconds(source_time);
        if source_secs.is_none() || source_secs != whole_seconds(cache_time) {
            return None;
        }
    }

    let mut data = Vec::new();
    file.read_to_end(&mut data).ok()?;
    Some(data)
}

/// `time` in whole seconds since 1970, or `None` before then.
fn whole_seconds(time: SystemTime) -> Option<u64> {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|since| since.as_secs())
}

/// Process an image: load, resize, and convert to WebP
fn process_image(full_path: &Path, width: u32, quality: u8) -> Option<Vec<u8>> {
    if failed_before(full_path) {
        return None;
    }

    // Load the image (handle JXL specially, `.JXL` too)
    let is_jxl = full_path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("jxl"));
    let img = if is_jxl {
        load_jxl_image(full_path)?
    } else {
        load_standard_image(full_path)?
    };

    // Resize if needed (SIMD Lanczos3 via fast_image_resize)
    let img = if img.width() > width {
        resize_for_width(&img, width)
    } else {
        img
    };

    // Convert to WebP
    convert_to_webp(&img, quality)
}

/// Downscale `img` to `width` px wide (preserving aspect ratio) with
/// fast_image_resize's SIMD Lanczos3 — markedly faster than the `image` crate's
/// scalar convolution on the full-res photos this server transcodes. Falls back
/// to the `image` crate on the rare error path so serving never breaks.
///
/// `#[doc(hidden)] pub` for `benches/image_pipeline.rs`; not part of the public API.
#[doc(hidden)]
pub fn resize_for_width(img: &image::DynamicImage, width: u32) -> image::DynamicImage {
    use fast_image_resize::images::Image;
    use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};

    let (sw, sh) = (img.width(), img.height());
    let dw = width;
    // Widened to u64 so the multiply cannot overflow; the quotient is bounded by the
    // source aspect ratio times `dw`, so the saturating narrowing is unreachable in
    // practice and only there to keep the cast total.
    let dh = u32::try_from(((u64::from(sh) * u64::from(dw)) / u64::from(sw).max(1)).max(1))
        .unwrap_or(u32::MAX);

    // libwebp (and fir) work on packed RGB/RGBA8; convert once up front so both
    // the resize and the later encode operate on the same layout.
    let has_alpha = img.color().has_alpha();
    let (pixel_type, src_buf) = if has_alpha {
        (PixelType::U8x4, img.to_rgba8().into_raw())
    } else {
        (PixelType::U8x3, img.to_rgb8().into_raw())
    };

    let fallback = || img.resize(width, u32::MAX, image::imageops::FilterType::Lanczos3);

    let Ok(src) = Image::from_vec_u8(sw, sh, src_buf, pixel_type) else {
        return fallback();
    };
    let mut dst = Image::new(dw, dh, pixel_type);
    let opts = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));
    if Resizer::new().resize(&src, &mut dst, &opts).is_err() {
        return fallback();
    }

    let raw = dst.into_vec();
    let rebuilt = if has_alpha {
        image::RgbaImage::from_raw(dw, dh, raw).map(image::DynamicImage::ImageRgba8)
    } else {
        image::RgbImage::from_raw(dw, dh, raw).map(image::DynamicImage::ImageRgb8)
    };
    rebuilt.unwrap_or_else(fallback)
}

// How large a picture the server decodes. The photos are big: the largest on the
// production share is 13021 × 21718, about 283 MP, and the next few are about 240 MP.
// A header can claim far more than its file holds, though: a GIF of 35 bytes can
// declare a 65535 × 65535 canvas, 16 GiB decoded, and the decoder allocates the whole
// canvas before it reads a pixel. One such file would crash the server at every
// start, since the prewarm decodes everything. So every decode checks the header
// against these first, and allocates nothing for a picture over them.

/// Pixels a side: half what a GIF or PNG header can say, and half again the longest
/// side of any photo (21,718).
const MAX_SIDE: u32 = 32_768;

/// Pixels in all: 400 MP, about 40% over the largest photo.
const MAX_PIXELS: u64 = 400_000_000;

/// Bytes for a decode, the picture and the decoder's own buffers together:
/// [`MAX_PIXELS`] at eight bytes a pixel, 3.2 GB. That is 16-bit RGBA, the widest
/// layout among the photos (there is a 16-bit RGBA PNG), so every format gets the same
/// 400 MP. The largest photo takes 0.85 GB as an 8-bit RGB JPEG, and a GIF's canvas
/// at most 1.6 GB, which leaves 1.6 GB for a frame that doesn't fill it. The resize
/// that follows copies the picture once more, as 8-bit RGB or RGBA, outside this.
const MAX_DECODED_BYTES: u64 = MAX_PIXELS * 8;

/// Pixels in all for a JPEG XL: 100 MP. jxl-oxide works in 32-bit floats and needs
/// 25–30 bytes a pixel beside the picture (a 48 MP JPEG XL needed between 1.15 and
/// 1.2 GB; a flat 36 MP one of 6.9 KB peaked at 1.1 GB), so at 400 MP a file of a few
/// KB could take 10–12 GB. There are no JPEG XL files on the share. 100 MP is about a
/// 100-megapixel camera's frame, and keeps jxl-oxide's own memory within
/// [`MAX_JXL_WORKING_BYTES`]. A larger photo needs a JPEG copy to get its sizes from:
/// [`SOURCE_EXTENSIONS`] puts JPEG XL before WebP, PNG and GIF, and the pick never
/// falls through to the next file, so that it stays a matter of file names.
const MAX_JXL_PIXELS: u64 = 100_000_000;

/// Bytes jxl-oxide may allocate while it decodes a JPEG XL, beside the picture:
/// [`MAX_JXL_PIXELS`] at 32 bytes a pixel, 3.2 GB.
const MAX_JXL_WORKING_BYTES: u64 = MAX_JXL_PIXELS * 32;

/// The limits a decoder starts with: [`MAX_SIDE`], and all of [`MAX_DECODED_BYTES`].
fn decode_limits() -> image::Limits {
    let mut limits = image::Limits::no_limits();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_DECODED_BYTES);
    limits
}

/// Whether a picture of `width` × `height`, decoding to `bytes`, is within the limits,
/// at most `max_pixels` in all.
fn check_size(width: u32, height: u32, bytes: u64, max_pixels: u64) -> image::ImageResult<()> {
    use image::error::{ImageError, LimitError, LimitErrorKind};

    decode_limits().check_dimensions(width, height)?;
    if u64::from(width) * u64::from(height) > max_pixels {
        return Err(ImageError::Limits(LimitError::from_kind(
            LimitErrorKind::DimensionError,
        )));
    }
    decode_limits().reserve(bytes)
}

/// The limits for a decoder whose picture takes `picture` bytes: what the picture
/// leaves of [`MAX_DECODED_BYTES`], so that the two together stay within it, as in
/// `image`'s own `ImageReader::decode`.
fn limits_beside(picture: u64) -> image::ImageResult<image::Limits> {
    let mut limits = decode_limits();
    limits.reserve(picture)?;
    Ok(limits)
}

/// Load a JXL image using jxl-oxide
fn load_jxl_image(path: &Path) -> Option<image::DynamicImage> {
    use image::ImageDecoder;
    use jxl_oxide::integration::JxlDecoder;

    let stamp = file_stamp(path);
    let decoded = (|| {
        let mut decoder = JxlDecoder::new(std::fs::File::open(path)?)?;
        let (width, height) = decoder.dimensions();
        check_size(width, height, decoder.total_bytes(), MAX_JXL_PIXELS)?;
        // `set_limits` checks the sides again, and gives jxl-oxide its own allowance.
        let mut limits = decode_limits();
        limits.max_alloc = Some(MAX_JXL_WORKING_BYTES);
        decoder.set_limits(limits)?;
        image::DynamicImage::from_decoder(decoder)
    })();
    settle(path, stamp, decoded)
}

/// Load a standard image format
fn load_standard_image(path: &Path) -> Option<image::DynamicImage> {
    use image::ImageDecoder;

    let stamp = file_stamp(path);
    let decoded = (|| {
        let mut reader = image::ImageReader::open(path)?;
        reader.limits(decode_limits());
        // `into_decoder` gives the decoder all of `MAX_DECODED_BYTES`, and neither it
        // nor `from_decoder` counts the picture, so that is done here: the picture is
        // checked against the limits, and the decoder keeps only what it leaves. The
        // PNG decoder takes its allowance when it is made, as in `ImageReader::decode`;
        // its own buffers are a few rows.
        let mut decoder = reader.into_decoder()?;
        let (width, height) = decoder.dimensions();
        let picture = decoder.total_bytes();
        check_size(width, height, picture, MAX_PIXELS)?;
        decoder.set_limits(limits_beside(picture)?)?;
        image::DynamicImage::from_decoder(decoder)
    })();
    settle(path, stamp, decoded)
}

/// A file's length and modification time, which change when it is replaced.
type FileStamp = (u64, Option<std::time::SystemTime>);

/// Files that couldn't be decoded, as they were then: refused by the limits, or failing
/// to decode, once they had settled. Each is logged once, and refused again from a
/// `stat` until it changes. Otherwise every request for its sizes, and every prewarm,
/// would read it again to fail the same way: for a JPEG that means reading the whole
/// file, for a PNG cut short decoding it up to the cut, and a JPEG XL can use up its
/// allowance before it fails.
static FAILED: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<PathBuf, FileStamp>>,
> = std::sync::LazyLock::new(Default::default);

fn file_stamp(path: &Path) -> Option<FileStamp> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.len(), metadata.modified().ok()))
}

/// Whether `path`, as it is now, failed to decode before.
fn failed_before(path: &Path) -> bool {
    let Some(stamp) = file_stamp(path) else {
        return false;
    };
    FAILED
        .lock()
        .is_ok_and(|failed| failed.get(path) == Some(&stamp))
}

/// Whether `err` is a failure to read the file that may pass, such as `EIO` from the NFS
/// share, rather than something about what the file holds. That is any I/O error but an
/// unexpected end of the file: a PNG or a WebP cut short fails that way, and so does a
/// JPEG XL, which reports every error as a decoding error with the I/O error under it.
/// `image`'s own I/O error doesn't give the I/O error as its source, so it is looked
/// at first.
fn read_failed(err: &image::ImageError) -> bool {
    let passes = |io: &std::io::Error| io.kind() != std::io::ErrorKind::UnexpectedEof;
    if let image::ImageError::IoError(io) = err {
        return passes(io);
    }
    let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(err) = cause {
        if let Some(io) = err.downcast_ref::<std::io::Error>() {
            return passes(io);
        }
        cause = err.source();
    }
    false
}

/// The decoded picture, or `None`. A failure is remembered with `before`, the file's
/// stamp before it was read (see [`FAILED`]), and logged once for each version of the
/// file, but only once the file has settled: the same stamp after the attempt, and a
/// time [`SETTLE_TIME`] old, the rule a WebP made from it is cached by. A file still
/// being written can fail now and decode later at the same length and time, written by
/// something that sets the length first, or with a stamp the NFS client kept. Nor is a
/// failure to read the file remembered, as it may pass (see [`read_failed`]).
fn settle(
    path: &Path,
    before: Option<FileStamp>,
    decoded: image::ImageResult<image::DynamicImage>,
) -> Option<image::DynamicImage> {
    let err = match decoded {
        Ok(picture) => return Some(picture),
        Err(err) => err,
    };
    if read_failed(&err) {
        return None;
    }
    let stamp = before.filter(|before| Some(*before) == file_stamp(path))?;
    if !stamp
        .1
        .is_none_or(|modified| has_settled(modified, SystemTime::now()))
    {
        return None;
    }
    let first = FAILED
        .lock()
        .is_ok_and(|mut failed| failed.insert(path.to_owned(), stamp) != Some(stamp));
    if first {
        log!("Not making sizes from {}: {err}", path.display());
    }
    None
}

/// Convert an image to lossy WebP at the given quality (0-100). Uses
/// libwebp via the `webp` crate because the `image` crate's built-in
/// WebP encoder is lossless-only.
///
/// `#[doc(hidden)] pub` so `benches/image_pipeline.rs` and
/// `examples/alloc_profile.rs` can measure the encode step directly; not part
/// of the public API.
#[doc(hidden)]
pub fn convert_to_webp(img: &image::DynamicImage, quality: u8) -> Option<Vec<u8>> {
    // libwebp expects RGB or RGBA; convert to RGB8 for opaque images and
    // RGBA8 only when there's an alpha channel.
    let encoder = if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height())
            .encode(f32::from(quality))
    } else {
        let rgb = img.to_rgb8();
        webp::Encoder::from_rgb(rgb.as_raw(), rgb.width(), rgb.height()).encode(f32::from(quality))
    };
    Some(encoder.to_vec())
}

/// Save processed image data to cache, with `source_modified`, the source's
/// modification time read before it was decoded, as the file's own (see
/// [`try_use_cached_image`]).
///
/// Several requests, and the startup pre-warm, can make the same file at once, while
/// others read it. So each writer fills and dates a file of its own beside it, then
/// renames that over the cache file in one step: a reader gets the whole of one file
/// or the whole of another, never part of one.
///
/// Failures (full disk, read-only cache dir, …) are logged but otherwise
/// non-fatal: the image was still produced for this request, only the cache
/// speedup is lost.
fn save_to_cache(
    cache_dir: &str,
    cache_path: &Path,
    data: &[u8],
    source_modified: Option<SystemTime>,
) {
    use std::fs;
    use std::io::Write;

    if let Err(err) = fs::create_dir_all(cache_dir) {
        log!("cache: create_dir_all({}) failed: {err}", cache_dir);
        return;
    }
    let temporary = temporary_path(cache_path);
    // A name that is already taken belongs to someone else, such as a server that
    // crashed: in Docker every server is process 1. Leave that file alone.
    let mut file = match fs::File::create_new(&temporary) {
        Ok(file) => file,
        Err(err) => {
            log!("cache: creating {} failed: {err}", temporary.display());
            return;
        }
    };
    // No fsync before the rename: a power cut can still leave the cache file empty,
    // which counts as a miss, or cut short, and a cache isn't worth waiting on the disk.
    let saved = file
        .write_all(data)
        .and_then(|()| match source_modified {
            Some(modified) => file.set_modified(modified),
            None => Ok(()),
        })
        .and_then(|()| fs::rename(&temporary, cache_path));
    if let Err(err) = saved {
        log!("cache: saving {} failed: {err}", cache_path.display());
        fs::remove_file(&temporary).ok();
    }
}

/// A file of one writer's own beside `cache_path`, for [`save_to_cache`]:
/// `<cache file name>.<process id>-<count>.tmp`. [`cleanup_cache`] removes any that a
/// crash leaves behind.
fn temporary_path(cache_path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};

    static WRITES: AtomicU64 = AtomicU64::new(0);
    let count = WRITES.fetch_add(1, Ordering::Relaxed);
    let mut name = cache_path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}-{count}.tmp", std::process::id()));
    cache_path.with_file_name(name)
}

/// Whether `filename` is one of [`temporary_path`]'s names.
fn is_temporary_file(filename: &str) -> bool {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    filename
        .strip_suffix(".tmp")
        .and_then(|rest| rest.rsplit_once('.'))
        .and_then(|(cache_name, writer)| Some((cache_name, writer.split_once('-')?)))
        .is_some_and(|(cache_name, (process, count))| {
            digits(process) && digits(count) && cache_file_prefix(cache_name).is_some()
        })
}

/// Clean up orphaned cache files that no longer have corresponding source images
/// Also removes old cache files that haven't been accessed in a while
pub fn cleanup_cache(images_dir: &str, cache_dir: &str) {
    use std::fs;
    use std::time::{Duration, SystemTime};

    log!("Starting cache cleanup...");

    let cache_path = std::path::Path::new(cache_dir);
    if !cache_path.exists() {
        log!("Cache directory does not exist, skipping cleanup");
        return;
    }

    // Enumerate every source image and compute the path-encoded prefix
    // `process_and_cache_image` would produce. Doing it this way avoids the
    // ambiguous reverse decoding that previously deleted cache files for any
    // source whose name happened to contain `_` or `_w`.
    //
    // No photos at all is what a NAS share that is down looks like: an empty
    // mountpoint, or none. A read error is what one that is partly there looks like.
    // Either way some cache files would look orphaned only because their photos are
    // out of sight, and once the share is back those photos would have to be made
    // again, so `None` removes no orphans. The rest of the cleanup doesn't depend on
    // the photos and goes ahead.
    let valid_prefixes = match collect_valid_prefixes(images_dir) {
        Ok(prefixes) if !prefixes.is_empty() => Some(prefixes),
        Ok(_) => {
            log!("No photos found in {images_dir}; removing no orphaned cache files");
            None
        }
        Err(e) => {
            log!("Couldn't read all of {images_dir} ({e}); removing no orphaned cache files");
            None
        }
    };

    let mut orphaned_count = 0;
    let mut old_count = 0;
    let mut unfinished_count = 0;
    let mut error_count = 0;

    // Cache files not accessed in 30 days will be removed
    let max_age = Duration::from_hours(720); // 30 days
    let now = SystemTime::now();

    let cache_entries = match fs::read_dir(cache_path) {
        Ok(entries) => entries,
        Err(e) => {
            log!("Failed to read cache directory: {}", e);
            return;
        }
    };

    for entry in cache_entries.flatten() {
        let cache_file = entry.path();

        if !cache_file.is_file() {
            continue;
        }

        let Some(filename) = cache_file.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        // A writer's unfinished file, left by a crash. One a request is writing right
        // now goes too, which only costs that request its cache file.
        if is_temporary_file(filename) {
            match fs::remove_file(&cache_file) {
                Ok(()) => unfinished_count += 1,
                Err(e) => {
                    log!("Failed to remove unfinished cache file {}: {}", filename, e);
                    error_count += 1;
                }
            }
            continue;
        }

        let Some(prefix) = cache_file_prefix(filename) else {
            // Unknown filename pattern — leave it alone rather than risk
            // deleting an unrelated file someone dropped in the cache dir.
            continue;
        };

        if valid_prefixes
            .as_ref()
            .is_some_and(|valid| !valid.contains(prefix))
        {
            match fs::remove_file(&cache_file) {
                Ok(()) => {
                    log!("Removed orphaned cache file: {}", filename);
                    orphaned_count += 1;
                }
                Err(e) => {
                    log!("Failed to remove cache file {}: {}", filename, e);
                    error_count += 1;
                }
            }
            continue;
        }

        match remove_if_unread_for(&cache_file, max_age, now) {
            Ok(Some(age)) => {
                let age_days = age.as_secs() / 86_400;
                log!("Removed old cache file ({}d old): {}", age_days, filename);
                old_count += 1;
            }
            Ok(None) => {}
            Err(e) => {
                log!("Failed to remove old cache file {}: {}", filename, e);
                error_count += 1;
            }
        }
    }

    log!(
        "Cache cleanup complete: removed {} orphaned, {} old (30+ days), {} unfinished, {} errors",
        orphaned_count,
        old_count,
        unfinished_count,
        error_count
    );
}

/// Parse the source-path prefix out of a cache filename.
///
/// Cache files are named `{prefix}_w{W}_q{Q}_l{V}.webp`. The prefix encodes
/// the source path with `/` (and `\`) replaced by `_`, so it can itself
/// contain underscores and even `_w` substrings (e.g. `home_walking`). We
/// strip the deterministic right-hand `_w<digits>_q<digits>_l<digits>.webp`
/// tail instead of trying to reverse the path encoding, which is lossy.
fn cache_file_prefix(filename: &str) -> Option<&str> {
    let stripped = filename.strip_suffix(".webp")?;
    let head = strip_underscore_digit_segment(stripped, "_l")?;
    let head = strip_underscore_digit_segment(head, "_q")?;
    let prefix = strip_underscore_digit_segment(head, "_w")?;
    (!prefix.is_empty()).then_some(prefix)
}

/// Strip a trailing `<sep><digits>` segment (e.g. `_l1`, `_q80`, `_w2400`)
/// from `s` and return the remainder. None if the tail doesn't match.
fn strip_underscore_digit_segment<'a>(s: &'a str, sep: &str) -> Option<&'a str> {
    let pos = s.rfind(sep)?;
    let (head, tail) = s.split_at(pos);
    let digits = &tail[sep.len()..];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(head)
}

/// Walk `images_dir` recursively and collect the set of valid cache-file
/// prefixes — the same encoding `process_and_cache_image` produces for each
/// source file. Used by `cleanup_cache` to decide whether a cache file's
/// source still exists.
///
/// `Err`, naming the path, when part of `images_dir` can't be read: a source may
/// be there but out of sight. What isn't there at all (`images_dir` itself, a file
/// deleted during the walk, a link to nothing) is no error.
fn collect_valid_prefixes(images_dir: &str) -> std::io::Result<std::collections::HashSet<String>> {
    let mut prefixes = std::collections::HashSet::new();
    let base = std::path::Path::new(images_dir);
    collect_valid_prefixes_recursive(base, base, &mut prefixes)?;
    Ok(prefixes)
}

fn collect_valid_prefixes_recursive(
    dir: &std::path::Path,
    base: &std::path::Path,
    out: &mut std::collections::HashSet<String>,
) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    let at = |path: &Path, err: Error| Error::new(err.kind(), format!("{}: {err}", path.display()));

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(at(dir, err)),
    };
    for entry in entries {
        let path = entry.map_err(|err| at(dir, err))?.path();
        // Follows links, as `Path::is_dir` does; and as for `is_dir`, a link to
        // nothing (its target missing, running through a file, or with a name too
        // long to exist) or a file deleted since the listing is no folder.
        let is_dir = match std::fs::metadata(&path) {
            Ok(metadata) => metadata.is_dir(),
            Err(err)
                if matches!(
                    err.kind(),
                    ErrorKind::NotFound | ErrorKind::NotADirectory | ErrorKind::InvalidFilename
                ) =>
            {
                false
            }
            Err(err) => return Err(at(&path, err)),
        };
        if is_dir {
            collect_valid_prefixes_recursive(&path, base, out)?;
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let ext = ext.to_ascii_lowercase();
        if !matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "webp" | "gif" | "jxl" | "avif"
        ) {
            continue;
        }
        let Ok(relative) = path.strip_prefix(base) else {
            continue;
        };
        let no_ext = relative.with_extension("");
        let encoded = no_ext.to_string_lossy().replace(['/', '\\'], "_");
        if !encoded.is_empty() {
            out.insert(encoded);
        }
    }
    Ok(())
}

/// Removes `cache_file` when it was last read more than `max_age` before `now`, and
/// returns how long before. The age is read before the file goes, since afterwards
/// there's nothing to ask. `Ok(None)` when the file stays: it was read within
/// `max_age`, or its age can't be told (it is gone, or was read after `now`). `Err`
/// only when the delete fails.
fn remove_if_unread_for(
    cache_file: &Path,
    max_age: std::time::Duration,
    now: std::time::SystemTime,
) -> std::io::Result<Option<std::time::Duration>> {
    let age = cache_file
        .metadata()
        .and_then(|metadata| metadata.accessed())
        .ok()
        .and_then(|accessed| now.duration_since(accessed).ok());
    match age {
        Some(age) if age > max_age => {
            std::fs::remove_file(cache_file)?;
            Ok(Some(age))
        }
        _ => Ok(None),
    }
}

/// Pre-generate cache images for all existing photos
pub fn prewarm_cache(images_dir: &str, cache_dir: &str) {
    use crate::image_params::ImageParams;
    use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

    log!("Starting cache prewarming...");

    let valid_presets = ImageParams::get_valid_presets();
    // Only use the first (default) preset for prewarming
    let (width, quality) = if let Some(preset) = valid_presets.first() {
        *preset
    } else {
        log!("No valid presets configured, skipping cache prewarming");
        return;
    };

    log!("Prewarming cache with default preset: {width}px @ quality {quality}");

    // Recursively find all image files
    let image_paths = collect_image_paths(images_dir);
    log!("Found {} images to process", image_paths.len());

    // Prewarm across a bounded worker pool. Each transcode is CPU- and
    // memory-heavy (see `transcode_limit`: about 2.2 GB at its peak for the largest
    // photos as 8-bit JPEGs, about 4.7 GB as a 16-bit RGBA PNG, and a JPEG XL takes
    // several times its picture while it decodes; see `MAX_DECODED_BYTES` and
    // `MAX_JXL_WORKING_BYTES`), so cap parallelism at min(4, cores): fast startup on
    // multi-core boxes without a memory spike on large galleries. Workers pull from a
    // shared index, so uneven per-image cost stays balanced. Two images can share a
    // cache file (`sunset.jpg` and `sunset.webp`, or `sunset.JPG`), and requests can
    // be making the same file meanwhile; `save_to_cache` renames each finished file
    // into place, so that is safe.
    let workers = transcode_limit();

    let next = AtomicUsize::new(0);
    let processed = AtomicU32::new(0);
    let errors = AtomicU32::new(0);

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let idx = next.fetch_add(1, Ordering::Relaxed);
                    let Some(image_path) = image_paths.get(idx) else {
                        break;
                    };
                    if process_and_cache_image(images_dir, cache_dir, image_path, width, quality)
                        .is_some()
                    {
                        processed.fetch_add(1, Ordering::Relaxed);
                    } else {
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
    });

    log!(
        "Cache prewarming complete: processed {} images, {} errors",
        processed.load(Ordering::Relaxed),
        errors.load(Ordering::Relaxed)
    );
}

/// Recursively collect all image file paths
fn collect_image_paths(images_dir: &str) -> Vec<String> {
    let mut paths = Vec::new();
    collect_image_paths_recursive(std::path::Path::new(images_dir), images_dir, &mut paths);
    paths
}

/// Recursively collect all image file paths (helper function)
fn collect_image_paths_recursive(dir: &std::path::Path, base: &str, paths: &mut Vec<String>) {
    use std::fs;

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_image_paths_recursive(&path, base, paths);
            } else if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                // Support all formats
                if matches!(
                    ext.as_ref(),
                    "jxl" | "avif" | "jpg" | "jpeg" | "webp" | "png" | "gif"
                ) && let Ok(relative) = path.strip_prefix(base)
                {
                    paths.push(relative.to_string_lossy().to_string());
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;

    fn write_jpeg(path: &std::path::Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, bytes).unwrap();
    }

    /// Give the file at `path` a modification time an hour ago.
    fn backdate(path: &std::path::Path) {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_hours(1))
            .unwrap();
    }

    /// 2-byte file is enough for `path.exists()` checks; we never decode it
    /// in these tests.
    const STUB: &[u8] = b"\xff\xd8";

    #[test]
    fn resize_for_width_downscales_preserving_aspect_and_type() {
        // RGB source -> RGB output at the requested width, aspect preserved.
        let src = image::DynamicImage::ImageRgb8(image::RgbImage::new(200, 100));
        let out = resize_for_width(&src, 80);
        assert_eq!((out.width(), out.height()), (80, 40));
        assert!(matches!(out, image::DynamicImage::ImageRgb8(_)));

        // RGBA source -> RGBA output (alpha channel preserved through resize).
        let src_a = image::DynamicImage::ImageRgba8(image::RgbaImage::new(200, 100));
        let out_a = resize_for_width(&src_a, 50);
        assert_eq!((out_a.width(), out_a.height()), (50, 25));
        assert!(matches!(out_a, image::DynamicImage::ImageRgba8(_)));
    }

    #[test]
    #[serial] // prewarm reads IMAGE_PRESETS, shared with other env-driven tests
    fn prewarm_cache_transcodes_every_image_in_parallel() {
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");

        // Small but real JPEGs across a subdir, so `process_image` actually
        // decodes + encodes them (exercising the parallel worker pool).
        let mut jpeg = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::new(16, 12))
            .write_to(
                &mut std::io::Cursor::new(&mut jpeg),
                image::ImageFormat::Jpeg,
            )
            .unwrap();
        for i in 0..5 {
            let path = images.join(format!("g/pic{i}.jpg"));
            write_jpeg(&path, &jpeg);
            // Old enough to be trusted whole, so the pre-warm keeps what it makes.
            backdate(&path);
        }

        prewarm_cache(images.to_str().unwrap(), cache.to_str().unwrap());

        // One cached WebP derivative per source image.
        let produced = fs::read_dir(&cache).unwrap().count();
        assert_eq!(produced, 5);
    }

    #[test]
    fn find_image_file_finds_existing_extension() {
        let dir = TempDir::new().unwrap();
        let images = dir.path();
        write_jpeg(&images.join("foo/bar.jpg"), STUB);

        let found = find_image_file(images.to_str().unwrap(), "foo/bar", None, &["jpg", "png"]);
        assert!(found.is_some());
        let found = found.unwrap();
        assert!(found.ends_with("foo/bar.jpg"));
    }

    #[test]
    fn find_image_file_returns_none_for_nonexistent() {
        let dir = TempDir::new().unwrap();
        let found = find_image_file(dir.path().to_str().unwrap(), "missing", None, &["jpg"]);
        assert!(found.is_none());
    }

    #[test]
    fn find_image_file_rejects_path_traversal() {
        // Set up:  <root>/inside/inside.jpg   (legitimate)
        //         <root>/outside.jpg          (must NOT be reachable)
        let dir = TempDir::new().unwrap();
        let inside = dir.path().join("inside");
        fs::create_dir_all(&inside).unwrap();
        write_jpeg(&inside.join("inside.jpg"), STUB);
        write_jpeg(&dir.path().join("outside.jpg"), STUB);

        // Sanity: legitimate path resolves.
        assert!(find_image_file(inside.to_str().unwrap(), "inside", None, &["jpg"]).is_some());

        // Traversal: from `<root>/inside` try to reach `../outside` — would
        // resolve to `<root>/outside.jpg` which is OUTSIDE the images_dir.
        let escape = find_image_file(inside.to_str().unwrap(), "../outside", None, &["jpg"]);
        assert!(escape.is_none(), "traversal escape was not blocked");
    }

    /// 2024-11-15 06:33:20 UTC, a source's modification time.
    fn source_time() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_731_652_400)
    }

    /// A cache file holding `cached`, modified at `modified`.
    fn cache_file_at(dir: &TempDir, modified: SystemTime) -> PathBuf {
        let cache = dir.path().join("source.webp");
        fs::write(&cache, b"cached").unwrap();
        fs::File::options()
            .write(true)
            .open(&cache)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        cache
    }

    #[test]
    fn try_use_cached_image_returns_data_with_the_source_time() {
        let dir = TempDir::new().unwrap();
        let cache = cache_file_at(&dir, source_time());

        let data = try_use_cached_image(Some(source_time()), &cache);
        assert_eq!(data.as_deref(), Some(b"cached".as_slice()));
    }

    #[test]
    fn try_use_cached_image_misses_when_source_newer() {
        let dir = TempDir::new().unwrap();
        let cache = cache_file_at(&dir, source_time());

        let newer = source_time() + Duration::from_mins(1);
        assert!(try_use_cached_image(Some(newer), &cache).is_none());
    }

    #[test]
    fn try_use_cached_image_misses_when_source_older() {
        let dir = TempDir::new().unwrap();
        let cache = cache_file_at(&dir, source_time());

        let older = source_time() - Duration::from_hours(24);
        assert!(try_use_cached_image(Some(older), &cache).is_none());
    }

    #[test]
    fn try_use_cached_image_misses_when_cache_absent() {
        let dir = TempDir::new().unwrap();
        let cache = dir.path().join("source.webp");
        // No cache file written.
        assert!(try_use_cached_image(Some(source_time()), &cache).is_none());
    }

    /// An images dir holding `home/pic.png`, a `width`×`height` picture modified at
    /// `secs` seconds after 1970, and an empty cache dir beside it.
    fn one_picture(width: u32, height: u32, secs: u64) -> (TempDir, String, String) {
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        replace_picture(&images, width, height, secs);
        let images = images.to_str().unwrap().to_owned();
        let cache = cache.to_str().unwrap().to_owned();
        (dir, images, cache)
    }

    /// Write `home/pic.png` under `images` as a `width`×`height` picture, modified at
    /// `secs` seconds after 1970.
    fn replace_picture(images: &Path, width: u32, height: u32, secs: u64) {
        let path = images.join("home/pic.png");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::RgbImage::new(width, height).save(&path).unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
            .unwrap();
    }

    /// The compressed endpoint's answer for `home/pic`, decoded to its size.
    fn compressed_size(images: &str, cache: &str) -> (u32, u32) {
        let webp = process_and_cache_image(images, cache, "home/pic", 2400, 90)
            .unwrap()
            .data;
        let picture = image::load_from_memory(&webp).unwrap();
        (picture.width(), picture.height())
    }

    /// The one cache file `process_and_cache_image` made for `home/pic` at 2400/90.
    fn cache_file(cache: &str) -> PathBuf {
        PathBuf::from(cache).join("home_pic_w2400_q90_l1.webp")
    }

    // Replaced by an earlier export (`cp -p`, `rsync -a`, a restore from backup) with
    // no watcher event to prune the cache, as on an NFS share written from elsewhere.
    #[test]
    fn a_source_replaced_by_an_older_file_is_made_again() {
        let (_dir, images, cache) = one_picture(8, 4, 1_731_652_400);
        assert_eq!(compressed_size(&images, &cache), (8, 4));

        replace_picture(Path::new(&images), 6, 6, 1_731_652_400 - 86_400);

        assert_eq!(compressed_size(&images, &cache), (6, 6));
    }

    #[test]
    fn an_unchanged_source_is_served_from_the_cache() {
        let (_dir, images, cache) = one_picture(8, 4, 1_731_652_400);
        compressed_size(&images, &cache);

        // Different bytes under the times the server left on the file: served as they
        // are only if the server takes the file from the cache without making it again.
        let path = cache_file(&cache);
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        fs::write(&path, b"from the cache").unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();

        let served = process_and_cache_image(&images, &cache, "home/pic", 2400, 90).map(|w| w.data);
        assert_eq!(served.as_deref(), Some(b"from the cache".as_slice()));
    }

    // What every cache file made before the server stamped them looks like: newer
    // than its source, with no other link to it.
    #[test]
    fn a_cache_file_not_stamped_with_its_source_is_made_again() {
        let (_dir, images, cache) = one_picture(8, 4, 1_731_652_400);
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache_file(&cache), b"made before the stamp").unwrap();

        assert_eq!(compressed_size(&images, &cache), (8, 4));
    }

    /// `time` as whole seconds since 1970.
    fn secs(time: SystemTime) -> u64 {
        time.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    // An export written in place: part of the new picture is on disk when the request
    // comes, and the rest follows within the same second, which leaves the address and
    // the time the cache goes by as they were.
    #[test]
    fn a_source_changed_moments_ago_is_made_but_not_cached() {
        let (_dir, images, cache) = one_picture(8, 4, secs(SystemTime::now()));

        let webp = process_and_cache_image(&images, &cache, "home/pic", 2400, 90).unwrap();

        assert!(
            !webp.settled,
            "a source this new may still be being written"
        );
        assert!(
            !cache_file(&cache).exists(),
            "and nothing made from it is kept"
        );
    }

    #[test]
    fn a_source_left_alone_for_the_settle_time_is_cached() {
        let then = SystemTime::now() - SETTLE_TIME - Duration::from_secs(1);
        let (_dir, images, cache) = one_picture(8, 4, secs(then));

        let webp = process_and_cache_image(&images, &cache, "home/pic", 2400, 90).unwrap();

        assert!(webp.settled);
        assert!(cache_file(&cache).exists());
    }

    #[test]
    fn a_file_has_settled_only_once_the_settle_time_has_passed() {
        let now = SystemTime::now();
        assert!(!has_settled(now, now));
        assert!(!has_settled(
            now - SETTLE_TIME + Duration::from_millis(500),
            now
        ));
        assert!(has_settled(now - SETTLE_TIME, now));
        // A clock ahead of this one, as a file server's can be: not yet.
        assert!(!has_settled(now + Duration::from_secs(5), now));
        assert!(!has_settled(now + Duration::from_secs(30), now));
    }

    // A time set on purpose rather than by a write: copied from a camera card or a
    // machine whose clock is ahead (`cp -p`, `rsync -a`), or `touch -d`. Waiting for
    // this clock to pass it could take days, with every view made again meanwhile.
    #[test]
    fn a_time_far_ahead_of_this_clock_counts_as_settled() {
        let now = SystemTime::now();
        assert!(has_settled(now + Duration::from_secs(61), now));
        assert!(has_settled(now + Duration::from_hours(24), now));
    }

    #[test]
    fn a_source_dated_a_day_ahead_is_cached() {
        let tomorrow = SystemTime::now() + Duration::from_hours(24);
        let (_dir, images, cache) = one_picture(8, 4, secs(tomorrow));

        let webp = process_and_cache_image(&images, &cache, "home/pic", 2400, 90).unwrap();

        assert!(webp.settled);
        assert!(cache_file(&cache).exists());
    }

    // Read before and after it was decoded, the source's time differs: it was written
    // meanwhile, so what was read may be part of a picture.
    #[test]
    fn a_source_changed_while_it_was_decoded_has_not_settled() {
        let old = SystemTime::now() - Duration::from_hours(1);
        let later = old + Duration::from_secs(30);
        let now = SystemTime::now();
        assert!(source_settled(Some(old), Some(old), now));
        assert!(!source_settled(Some(old), Some(later), now));
        assert!(!source_settled(Some(old), Some(now), now));
    }

    #[test]
    fn cached_image_tells_a_missing_source_from_a_missing_size() {
        let (_dir, images, cache) = one_picture(8, 4, 1_731_652_400);

        assert!(matches!(
            cached_image(&images, &cache, "home/gone", 2400, 90),
            Lookup::NoSource
        ));
        assert!(matches!(
            cached_image(&images, &cache, "home/pic", 2400, 90),
            Lookup::Missing
        ));
        process_and_cache_image(&images, &cache, "home/pic", 2400, 90).unwrap();
        assert!(matches!(
            cached_image(&images, &cache, "home/pic", 2400, 90),
            Lookup::Cached(_)
        ));
    }

    // What a crash between creating and filling a cache file can leave.
    #[test]
    fn an_empty_cache_file_is_made_again() {
        let (_dir, images, cache) = one_picture(8, 4, 1_731_652_400);
        fs::create_dir_all(&cache).unwrap();
        let path = cache_file(&cache);
        fs::write(&path, b"").unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_731_652_400))
            .unwrap();

        assert_eq!(compressed_size(&images, &cache), (8, 4));
    }

    /// Several requests make the same size at once while others read it, as after a
    /// deploy or when a photo comes in two formats. Every answer must be the whole
    /// picture: never part of one, and never an empty one.
    #[test]
    fn concurrent_makers_and_readers_only_ever_get_whole_pictures() {
        const MAKERS: usize = 4;
        const READERS: usize = 4;
        const ROUNDS: u64 = 4;

        let (_dir, images, cache) = one_picture(8, 4, 1_731_652_400);
        // Noise compresses badly, so each cache file takes a while to write, and a
        // read in the middle of a write is likely to be caught.
        let source = Path::new(&images).join("home/pic.png");
        let mut state = 0x2545_f491_u32;
        let noise = image::RgbImage::from_fn(640, 480, |_, _| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let [r, g, b, _] = state.to_le_bytes();
            image::Rgb([r, g, b])
        });
        noise.save(&source).unwrap();
        let whole = process_and_cache_image(&images, &cache, "home/pic", 2400, 90)
            .unwrap()
            .data;

        let mut bad = Vec::new();
        for round in 1..=ROUNDS {
            // A new time makes every cached size stale, so each maker writes it again.
            fs::File::options()
                .write(true)
                .open(&source)
                .unwrap()
                .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_731_652_400 + round))
                .unwrap();
            let start = std::sync::Barrier::new(MAKERS + READERS);
            let making = std::sync::atomic::AtomicUsize::new(MAKERS);
            let answers = std::sync::Mutex::new(Vec::new());
            std::thread::scope(|scope| {
                for _ in 0..MAKERS {
                    scope.spawn(|| {
                        start.wait();
                        let answer = process_and_cache_image(&images, &cache, "home/pic", 2400, 90)
                            .map(|w| w.data);
                        answers.lock().unwrap().push(answer);
                        making.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                    });
                }
                for _ in 0..READERS {
                    scope.spawn(|| {
                        start.wait();
                        while making.load(std::sync::atomic::Ordering::SeqCst) > 0 {
                            let answer =
                                process_and_cache_image(&images, &cache, "home/pic", 2400, 90)
                                    .map(|w| w.data);
                            answers.lock().unwrap().push(answer);
                        }
                    });
                }
            });
            for answer in answers.into_inner().unwrap() {
                if answer.as_ref() != Some(&whole) {
                    bad.push(answer.map_or(0, |bytes| bytes.len()));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{} answers were not the whole {}-byte picture; their lengths: {bad:?}",
            bad.len(),
            whole.len()
        );
    }

    #[test]
    fn save_to_cache_creates_dir_and_writes_bytes_with_the_source_time() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join("nested/cache");
        let cache_path = cache_dir.join("foo.webp");
        save_to_cache(
            cache_dir.to_str().unwrap(),
            &cache_path,
            b"webp-data",
            Some(source_time()),
        );
        assert_eq!(fs::read(&cache_path).unwrap(), b"webp-data");
        assert_eq!(
            fs::metadata(&cache_path).unwrap().modified().unwrap(),
            source_time()
        );
    }

    #[test]
    fn cache_file_prefix_strips_w_q_l_suffix() {
        assert_eq!(
            cache_file_prefix("home_sunset_w2400_q80_l1.webp"),
            Some("home_sunset")
        );
        assert_eq!(
            cache_file_prefix("home_sunset_w4000_q90_l1.webp"),
            Some("home_sunset")
        );
    }

    #[test]
    fn cache_file_prefix_handles_underscore_in_source_name() {
        // Source path `home/space_needle.jpg` encodes to `home_space_needle`.
        // The lossy old code split on the first `_w` and replaced every `_`
        // with `/`, mangling this. The new parser strips from the right.
        assert_eq!(
            cache_file_prefix("home_space_needle_w2400_q80_l1.webp"),
            Some("home_space_needle")
        );
    }

    #[test]
    fn cache_file_prefix_handles_w_substring_in_source_name() {
        // `home/walking.jpg` — prefix `home_walking` contains `_w`.
        assert_eq!(
            cache_file_prefix("home_walking_w800_q80_l1.webp"),
            Some("home_walking")
        );
        // And starts-with: `home/wave.jpg` — prefix begins with `wave`.
        assert_eq!(
            cache_file_prefix("home_wave_w800_q80_l1.webp"),
            Some("home_wave")
        );
    }

    #[test]
    fn cache_file_prefix_rejects_unknown_pattern() {
        assert_eq!(cache_file_prefix("notes.txt"), None);
        assert_eq!(cache_file_prefix("foo.webp"), None);
        assert_eq!(cache_file_prefix("foo_w_q80_l1.webp"), None); // empty digit group
        assert_eq!(cache_file_prefix("_w2400_q80_l1.webp"), None); // empty prefix
    }

    #[test]
    fn collect_valid_prefixes_walks_recursively() {
        use std::collections::HashSet;
        let dir = TempDir::new().unwrap();
        let images = dir.path();
        write_jpeg(&images.join("home/sunset.jpg"), STUB);
        write_jpeg(&images.join("home/space_needle.jpg"), STUB);
        write_jpeg(&images.join("travel/iceland/glacier.png"), STUB);
        // Non-image file is skipped.
        fs::write(images.join("home/notes.txt"), b"x").unwrap();

        let prefixes = collect_valid_prefixes(images.to_str().unwrap()).unwrap();
        let expected: HashSet<String> =
            ["home_sunset", "home_space_needle", "travel_iceland_glacier"]
                .into_iter()
                .map(String::from)
                .collect();
        assert_eq!(prefixes, expected);
    }

    #[test]
    fn cleanup_cache_keeps_caches_for_underscored_source_names() {
        // Regression: the previous `_` → `/` reverse decoded
        // `home_space_needle` as `home/space/needle`, found no source, and
        // wrongly deleted the cache. The new code looks up the literal prefix
        // in the set built from a directory walk.
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        fs::create_dir_all(&images).unwrap();
        fs::create_dir_all(&cache).unwrap();
        write_jpeg(&images.join("home/space_needle.jpg"), STUB);

        let kept = cache.join("home_space_needle_w2400_q80_l1.webp");
        fs::write(&kept, b"webp").unwrap();

        cleanup_cache(images.to_str().unwrap(), cache.to_str().unwrap());

        assert!(
            kept.exists(),
            "cache for an existing source with `_` in its name must survive cleanup"
        );
    }

    #[test]
    fn cleanup_cache_removes_orphan_cache_files() {
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        write_jpeg(&images.join("home/kept.jpg"), STUB);
        let kept = cache.join("home_kept_w2400_q80_l1.webp");
        fs::write(&kept, b"webp").unwrap();
        let orphan = cache.join("home_deleted_w2400_q80_l1.webp");
        fs::write(&orphan, b"webp").unwrap();

        cleanup_cache(images.to_str().unwrap(), cache.to_str().unwrap());

        assert!(kept.exists());
        assert!(!orphan.exists());
    }

    // What a NAS outage looks like at startup: the mountpoint is there but holds
    // only the empty home gallery directory the server makes for itself. Every cache
    // file would look orphaned, and remaking them all costs every photo's transcode.
    #[test]
    fn cleanup_cache_removes_no_orphans_while_the_images_folder_has_no_photos() {
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        fs::create_dir_all(images.join("home")).unwrap();
        fs::create_dir_all(&cache).unwrap();
        let cached = cache.join("home_sunset_w2400_q80_l1.webp");
        fs::write(&cached, b"webp").unwrap();
        let (leftover, old) = leftover_and_old_file(&cache);

        cleanup_cache(images.to_str().unwrap(), cache.to_str().unwrap());

        assert!(cached.exists(), "a photo out of sight keeps its cache file");
        assert!(!leftover.exists(), "a crash's leftover is removed");
        assert!(!old.exists(), "a file unused for 30 days is removed");
    }

    // A share that is only partly there: one gallery lists, another can't be. The
    // photos in the one that can't are out of sight, not gone.
    #[test]
    fn cleanup_cache_removes_no_orphans_when_a_gallery_folder_cant_be_listed() {
        use std::os::unix::fs::PermissionsExt;
        /// A folder unlocked again when this goes, however the test ends, so the
        /// temporary directory around it can be removed.
        struct Locked(PathBuf);
        impl Drop for Locked {
            fn drop(&mut self) {
                fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755)).ok();
            }
        }

        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        write_jpeg(&images.join("home/pic.jpg"), STUB);
        write_jpeg(&images.join("nature/bird.jpg"), STUB);
        let hidden = cache.join("nature_bird_w2400_q80_l1.webp");
        fs::write(&hidden, b"webp").unwrap();
        // Declared after `dir`, so it unlocks the folder before `dir` is removed.
        let locked = Locked(images.join("nature"));
        fs::set_permissions(&locked.0, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read_dir(&locked.0).is_ok() {
            // Root reads it anyway; the next test fails a read for root too.
            eprintln!("skipped: {} is still readable", locked.0.display());
            return;
        }

        cleanup_cache(images.to_str().unwrap(), cache.to_str().unwrap());

        assert!(hidden.exists());
    }

    // A gallery whose folder can't even be looked at: a link to itself fails with
    // "too many levels of symbolic links", for root as well.
    #[test]
    fn cleanup_cache_removes_no_orphans_when_a_gallery_cant_be_read() {
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        write_jpeg(&images.join("home/pic.jpg"), STUB);
        std::os::unix::fs::symlink("nature", images.join("nature")).unwrap();
        let hidden = cache.join("nature_bird_w2400_q80_l1.webp");
        fs::write(&hidden, b"webp").unwrap();

        cleanup_cache(images.to_str().unwrap(), cache.to_str().unwrap());

        assert!(hidden.exists());
    }

    // When the walk can't see everything, only the orphans are in doubt. A crash's
    // leftover, and a file nobody has asked for in 30 days, go as usual.
    #[test]
    fn cleanup_cache_still_removes_leftovers_and_old_files_when_the_walk_fails() {
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        write_jpeg(&images.join("home/pic.jpg"), STUB);
        std::os::unix::fs::symlink("nature", images.join("nature")).unwrap();
        let hidden = cache.join("nature_bird_w2400_q80_l1.webp");
        fs::write(&hidden, b"webp").unwrap();
        let (leftover, old) = leftover_and_old_file(&cache);

        cleanup_cache(images.to_str().unwrap(), cache.to_str().unwrap());

        assert!(hidden.exists(), "a photo out of sight keeps its cache file");
        assert!(!leftover.exists(), "a crash's leftover is removed");
        assert!(!old.exists(), "a file unused for 30 days is removed");
    }

    /// Two files in `cache` that the cleanup removes whatever it makes of the photos:
    /// a crash's leftover temporary file, and a cache file last read 31 days ago.
    fn leftover_and_old_file(cache: &Path) -> (PathBuf, PathBuf) {
        let leftover = cache.join("nature_bird_w2400_q80_l1.webp.4242-7.tmp");
        let old = cache.join("nature_fox_w2400_q80_l1.webp");
        for file in [&leftover, &old] {
            fs::write(file, b"webp").unwrap();
        }
        let month_ago = SystemTime::now() - Duration::from_hours(31 * 24);
        fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_times(fs::FileTimes::new().set_accessed(month_ago))
            .unwrap();
        (leftover, old)
    }

    // A link to nothing hides nothing, so it doesn't stop the cleanup, whether its
    // target is missing or runs through a file.
    #[test]
    fn cleanup_cache_removes_orphans_past_links_to_nothing() {
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        write_jpeg(&images.join("home/pic.jpg"), STUB);
        std::os::unix::fs::symlink("gone", images.join("missing")).unwrap();
        std::os::unix::fs::symlink("home/pic.jpg/x", images.join("through_a_file")).unwrap();
        std::os::unix::fs::symlink("x".repeat(300), images.join("name_too_long")).unwrap();
        let orphan = cache.join("home_deleted_w2400_q80_l1.webp");
        fs::write(&orphan, b"webp").unwrap();

        cleanup_cache(images.to_str().unwrap(), cache.to_str().unwrap());

        assert!(!orphan.exists());
    }

    #[test]
    fn cleanup_cache_leaves_unknown_filenames_alone() {
        // A stray file someone dropped in the cache dir mustn't be deleted
        // just because its source can't be inferred.
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        write_jpeg(&images.join("home/pic.jpg"), STUB);
        let stray = cache.join("README.txt");
        fs::write(&stray, b"hello").unwrap();

        cleanup_cache(images.to_str().unwrap(), cache.to_str().unwrap());

        assert!(stray.exists());
    }

    // What a crash in the middle of `save_to_cache` leaves.
    #[test]
    fn cleanup_cache_removes_leftover_temporary_files() {
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        write_jpeg(&images.join("home/pic.jpg"), STUB);
        let leftover = cache.join("home_pic_w2400_q90_l1.webp.4242-7.tmp");
        fs::write(&leftover, b"half a picture").unwrap();
        let stray = cache.join("notes.tmp");
        fs::write(&stray, b"not ours").unwrap();

        cleanup_cache(images.to_str().unwrap(), cache.to_str().unwrap());

        assert!(!leftover.exists(), "a leftover temporary file is removed");
        assert!(
            stray.exists(),
            "a name the server doesn't make is left alone"
        );
    }

    #[test]
    fn remove_if_unread_for_returns_the_age_of_the_file_it_removed() {
        let dir = TempDir::new().unwrap();
        let (_, old) = leftover_and_old_file(dir.path());
        let fresh = dir.path().join("fresh.webp");
        fs::write(&fresh, b"webp").unwrap();
        let now = SystemTime::now();
        let month = Duration::from_hours(720);

        let removed = remove_if_unread_for(&old, month, now).unwrap();
        assert_eq!(
            removed.map(|age| age.as_secs() / 86_400),
            Some(31),
            "last read 31 days ago"
        );
        assert!(!old.exists(), "a file unused for 30 days is removed");

        assert_eq!(remove_if_unread_for(&fresh, month, now).unwrap(), None);
        assert!(fresh.exists(), "a file read just now stays");

        let hour_ago = now - Duration::from_hours(1);
        assert_eq!(
            remove_if_unread_for(&fresh, Duration::ZERO, hour_ago).unwrap(),
            None
        );
        assert!(fresh.exists(), "a file read after `now` stays");

        assert_eq!(
            remove_if_unread_for(&old, Duration::ZERO, now).unwrap(),
            None,
            "a file that is gone is no error"
        );
    }

    /// Photos named the way cameras and other tools name them, in a gallery `g`.
    const ANY_CASE: [&str; 5] = ["Upper.JPG", "Mixed.Jpg", "Shot.PNG", "Pic.WEBP", "Clip.GIF"];

    /// A 4×3 JPEG XL, which the `image` crate can't write: `cjxl -d 0` of a flat PPM.
    pub(crate) const TINY_JXL: &[u8] = &[
        0xff, 0x0a, 0x10, 0x30, 0x10, 0x50, 0x5c, 0x08, 0x08, 0x00, 0x01, 0x00, 0x58, 0x00, 0x4b,
        0x18, 0x8b, 0x15, 0xc2, 0x01, 0x63, 0x9b, 0x50, 0xf1, 0x07, 0xd8, 0x3b, 0x00, 0x00, 0xfb,
        0x48, 0x05, 0x78, 0x20, 0x00, 0x00,
    ];

    /// A small image at `path`, in the format its extension names in whatever case.
    /// Backdated past the settle time, so the sizes made from it are cached.
    fn write_image(path: &std::path::Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::RgbImage::new(16, 12).save(path).unwrap();
        backdate(path);
    }

    /// Which file `find_image_file` picks for `g/<stem>` under `images`, asked for with
    /// the extension `requested`, as a name.
    fn picked(images: &std::path::Path, stem: &str, requested: Option<&str>) -> Option<String> {
        let found = find_image_file(
            images.to_str().unwrap(),
            &format!("g/{stem}"),
            requested,
            &SOURCE_EXTENSIONS,
        )?;
        Some(found.file_name()?.to_str()?.to_owned())
    }

    #[test]
    fn find_image_file_finds_the_extension_in_any_case() {
        let dir = TempDir::new().unwrap();
        for name in ANY_CASE {
            write_jpeg(&dir.path().join("g").join(name), STUB);
        }

        for (stem, requested, expected) in [
            ("Upper", Some("JPG"), "Upper.JPG"),
            ("Mixed", Some("Jpg"), "Mixed.Jpg"),
            ("Shot", Some("PNG"), "Shot.PNG"),
            ("Pic", Some("WEBP"), "Pic.WEBP"),
            // Lower and upper case are found whatever the request names, as any format
            // already was.
            ("Upper", Some("jpg"), "Upper.JPG"),
            ("Upper", None, "Upper.JPG"),
            ("Pic", Some("webp"), "Pic.WEBP"),
        ] {
            assert_eq!(
                picked(dir.path(), stem, requested).as_deref(),
                Some(expected),
                "{stem} asked for as {requested:?}"
            );
        }
    }

    #[test]
    fn find_image_file_lists_a_folder_only_for_a_file_that_is_there() {
        let dir = TempDir::new().unwrap();
        write_jpeg(&dir.path().join("g/Mixed.Jpg"), STUB);
        write_jpeg(&dir.path().join("g/Other.jPg"), STUB);
        write_jpeg(&dir.path().join("g/Other.Webp"), STUB);

        // Found through the file the request names.
        assert_eq!(
            picked(dir.path(), "Mixed", Some("Jpg")).as_deref(),
            Some("Mixed.Jpg")
        );
        // Spellings that aren't there, made up or not, list nothing and find nothing.
        for requested in [Some("jpg"), Some("JPG"), Some("jPg"), Some("Webp"), None] {
            assert_eq!(
                picked(dir.path(), "Mixed", requested),
                None,
                "{requested:?}"
            );
        }
        assert_eq!(picked(dir.path(), "Gone", Some("Jpg")), None);
        // A file that is there picks among the name's files by the usual rule, whichever
        // one the request names: by format, so the jpg.
        for requested in ["jPg", "Webp"] {
            assert_eq!(
                picked(dir.path(), "Other", Some(requested)).as_deref(),
                Some("Other.jPg"),
                "{requested}"
            );
        }
    }

    #[test]
    fn find_image_file_picks_between_case_variants_the_same_way_every_time() {
        let dir = TempDir::new().unwrap();
        for (folder, names, expected) in [
            // Lower case first, then upper case, whatever the request names.
            ("a", ["x.JPG", "x.jpg"], "x.jpg"),
            ("b", ["x.Jpg", "x.JPG"], "x.JPG"),
            // Mixed spellings in byte order: upper-case letters sort first.
            ("c", ["x.jPg", "x.Jpg"], "x.Jpg"),
            // Format first, as before, for spellings found with a `stat`...
            ("d", ["x.webp", "x.JPG"], "x.JPG"),
            // ...and a mixed spelling only when no format is spelled either way.
            ("e", ["x.webp", "x.Jpg"], "x.webp"),
        ] {
            for name in names {
                write_jpeg(&dir.path().join(folder).join(name), STUB);
            }
            let x = format!("{folder}/x");
            for requested in [None, Some("jpg"), Some("JPG"), Some("Jpg"), Some("jPg")] {
                let found = find_image_file(
                    dir.path().to_str().unwrap(),
                    &x,
                    requested,
                    &SOURCE_EXTENSIONS,
                );
                let found = found.as_deref().and_then(std::path::Path::file_name);
                // A request that names a file that is there finds a mixed spelling; any
                // other finds a file only when one is spelled in lower or upper case.
                if folder == "c" && !matches!(requested, Some("Jpg" | "jPg")) {
                    assert_eq!(found, None, "{folder}: {requested:?}");
                } else {
                    assert_eq!(
                        found,
                        Some(std::ffi::OsStr::new(expected)),
                        "{folder}: {requested:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn find_image_file_keeps_a_mixed_spelling_inside_images_dir() {
        // <root>/inside is the images dir; <root>/outside.Jpg must not be reachable,
        // by a path or through a link.
        let dir = TempDir::new().unwrap();
        let inside = dir.path().join("inside");
        write_jpeg(&inside.join("g/inside.Jpg"), STUB);
        write_jpeg(&dir.path().join("outside.Jpg"), STUB);
        std::os::unix::fs::symlink(dir.path().join("outside.Jpg"), inside.join("g/link.Jpg"))
            .unwrap();
        let supported = ["jpg", "png"];
        let find =
            |path: &str| find_image_file(inside.to_str().unwrap(), path, Some("Jpg"), &supported);

        assert!(find("g/inside").is_some());
        assert!(
            find("../outside").is_none(),
            "traversal escape was not blocked"
        );
        assert!(
            find("g/../../outside").is_none(),
            "traversal escape was not blocked"
        );
        assert!(find("g/link").is_none(), "a link out was followed");
    }

    #[test]
    fn find_image_file_passes_over_avif_which_this_build_cannot_decode() {
        let dir = TempDir::new().unwrap();
        for name in [
            "a.AVIF", "a.webp", "b.avif", "b.Webp", "c.avif", "c.GIF", "d.avif", "e.GIF", "e.png",
        ] {
            write_jpeg(&dir.path().join("g").join(name), STUB);
        }

        for (stem, requested, expected) in [
            // An AVIF would only fail to decode, so the other format serves.
            ("a", "AVIF", Some("a.webp")),
            ("c", "avif", Some("c.GIF")),
            // The AVIF the gallery names leads to a mixed spelling too.
            ("b", "avif", Some("b.Webp")),
            // Nothing to make the sizes from.
            ("d", "avif", None),
            // A GIF is a source now, last in the order.
            ("e", "GIF", Some("e.png")),
        ] {
            assert_eq!(
                picked(dir.path(), stem, Some(requested)).as_deref(),
                expected,
                "{stem}.{requested}"
            );
        }
    }

    #[test]
    fn the_gallery_and_the_endpoint_pick_the_same_file() {
        // Spellings of each format the endpoint can decode. Every pair and every triple
        // of them is one photo, in a folder of its own.
        const NAMES: [&str; 18] = [
            "x.jpg", "x.JPG", "x.Jpg", "x.jPg", "x.jpeg", "x.JPEG", "x.Jpeg", "x.jxl", "x.JXL",
            "x.webp", "x.WEBP", "x.Webp", "x.png", "x.PNG", "x.Png", "x.gif", "x.GIF", "x.Gif",
        ];
        let mut sets = Vec::new();
        for (i, a) in NAMES.iter().enumerate() {
            for (j, b) in NAMES.iter().enumerate().skip(i + 1) {
                sets.push(vec![*a, *b]);
                sets.extend(NAMES[j + 1..].iter().map(|c| vec![*a, *b, *c]));
            }
        }
        assert_eq!(sets.len(), 153 + 816);

        let dir = TempDir::new().unwrap();
        let root = dir.path().to_str().unwrap();
        for (n, files) in sets.iter().enumerate() {
            let folder = dir.path().join(format!("f{n}"));
            for file in files {
                write_jpeg(&folder.join(file), STUB);
            }
            let mut photos = Vec::new();
            crate::gallery::find_images_for_gallery(&folder, dir.path(), &mut photos);
            assert_eq!(photos.len(), 1, "{files:?}");
            let main = photos[0].filename.as_str();

            // The site asks for the main file. Asking for any of the others gives the
            // same answer.
            for file in files {
                let requested = file.rsplit_once('.').map(|(_, ext)| ext);
                let found =
                    find_image_file(root, &format!("f{n}/x"), requested, &SOURCE_EXTENSIONS);
                let found = found.as_deref().and_then(Path::file_name);
                assert_eq!(
                    found.and_then(std::ffi::OsStr::to_str),
                    Some(main),
                    "{files:?}, asked for {file}"
                );
            }
        }
    }

    #[test]
    fn process_and_cache_image_transcodes_any_case_of_extension() {
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        for name in ANY_CASE {
            write_image(&images.join("g").join(name));
        }
        fs::write(images.join("g/Art.JXL"), TINY_JXL).unwrap();
        backdate(&images.join("g/Art.JXL"));

        for name in ANY_CASE.into_iter().chain(["Art.JXL"]) {
            let webp = process_and_cache_image(
                images.to_str().unwrap(),
                cache.to_str().unwrap(),
                &format!("g/{name}"),
                8,
                80,
            );
            let webp = webp.unwrap_or_else(|| panic!("no WebP for {name}")).data;
            assert_eq!(&webp[8..12], b"WEBP", "{name}");

            // Cached under the name without the extension, in the case it has.
            let stem = name.split_once('.').unwrap().0;
            let cached = cache.join(format!("g_{stem}_w8_q80_l1.webp"));
            assert_eq!(fs::read(&cached).ok(), Some(webp), "{name}");
        }
    }

    #[test]
    fn process_and_cache_image_serves_another_format_beside_avif() {
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        // Not real AVIF: this build can't decode it, so its bytes never matter.
        write_jpeg(&images.join("g/x.AVIF"), STUB);
        write_image(&images.join("g/x.webp"));
        write_jpeg(&images.join("g/y.avif"), STUB);
        let process = |path: &str| {
            process_and_cache_image(
                images.to_str().unwrap(),
                cache.to_str().unwrap(),
                path,
                8,
                80,
            )
            .map(|webp| webp.data)
        };

        let webp = process("g/x.AVIF").expect("the WebP beside the AVIF");
        assert_eq!(&webp[8..12], b"WEBP");
        assert_eq!(process("g/y.avif"), None);
    }

    /// A 40×30 animated GIF of `frames` frames, the first red and the rest blue.
    fn animated_gif(frames: usize) -> Vec<u8> {
        use image::codecs::gif::GifEncoder;
        use image::{Delay, Frame, Rgba, RgbaImage};

        let mut gif = Vec::new();
        // The encoder writes the GIF's trailer when it is dropped, at the end of this block.
        {
            let mut encoder = GifEncoder::new_with_speed(&mut gif, 30);
            for n in 0..frames {
                let colour = if n == 0 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                };
                let picture = RgbaImage::from_pixel(40, 30, Rgba(colour));
                let delay = Delay::from_numer_denom_ms(100, 1);
                encoder
                    .encode_frame(Frame::from_parts(picture, 0, 0, delay))
                    .unwrap();
            }
        }
        gif
    }

    #[test]
    fn process_and_cache_image_makes_a_gif_from_its_first_frame_alone() {
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        fs::create_dir_all(images.join("g")).unwrap();
        let gif = animated_gif(300);
        fs::write(images.join("g/Loop.GIF"), &gif).unwrap();
        // The same animation cut off halfway, in the middle of a frame and with no
        // trailer. It still transcodes, because nothing after the first frame is read.
        fs::write(images.join("g/Cut.gif"), &gif[..gif.len() / 2]).unwrap();

        for name in ["Loop.GIF", "Cut.gif"] {
            let webp = process_and_cache_image(
                images.to_str().unwrap(),
                cache.to_str().unwrap(),
                &format!("g/{name}"),
                2400,
                90,
            )
            .unwrap_or_else(|| panic!("no WebP for {name}"))
            .data;
            let picture = image::load_from_memory(&webp).unwrap().to_rgb8();
            assert_eq!(picture.dimensions(), (40, 30), "{name}");
            let [red, green, blue] = picture.get_pixel(20, 15).0;
            assert!(
                red > 200 && green < 50 && blue < 50,
                "{name}: ({red}, {green}, {blue}) is not the first frame's red"
            );
        }
    }

    #[test]
    #[serial] // prewarm reads IMAGE_PRESETS, shared with other env-driven tests
    fn prewarm_cache_transcodes_any_case_of_extension() {
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        for name in ANY_CASE {
            write_image(&images.join("g").join(name));
        }

        prewarm_cache(images.to_str().unwrap(), cache.to_str().unwrap());

        let mut produced: Vec<String> = fs::read_dir(&cache)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        produced.sort();
        let (width, quality) = crate::image_params::ImageParams::get_valid_presets()[0];
        let mut expected: Vec<String> = ["Clip", "Mixed", "Pic", "Shot", "Upper"]
            .iter()
            .map(|stem| format!("g_{stem}_w{width}_q{quality}_l1.webp"))
            .collect();
        expected.sort();
        assert_eq!(produced, expected);
    }

    #[test]
    fn cleanup_cache_keeps_the_caches_of_any_case_of_extension() {
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        for name in ANY_CASE {
            write_jpeg(&images.join("g").join(name), STUB);
        }
        let kept = ["g_Upper", "g_Mixed", "g_Shot", "g_Pic", "g_Clip"];
        // The name keeps its case, so a cache file for `upper` is not one for `Upper`.
        let orphans = ["g_Gone", "g_upper"];
        for prefix in kept.iter().chain(&orphans) {
            fs::write(cache.join(format!("{prefix}_w2400_q90_l1.webp")), b"webp").unwrap();
        }

        cleanup_cache(images.to_str().unwrap(), cache.to_str().unwrap());

        for prefix in kept {
            let file = cache.join(format!("{prefix}_w2400_q90_l1.webp"));
            assert!(file.exists(), "{prefix} was removed");
        }
        for prefix in orphans {
            let file = cache.join(format!("{prefix}_w2400_q90_l1.webp"));
            assert!(!file.exists(), "{prefix} was kept");
        }
    }

    /// `image_path` cut as `process_and_cache_image` cuts it.
    fn cut(image_path: &str) -> (&str, Option<&str>) {
        match image_path.rsplit_once('.') {
            Some((path_without_ext, ext)) => (path_without_ext, Some(ext)),
            None => (image_path, None),
        }
    }

    #[test]
    fn a_folder_is_listed_only_for_an_image_named_inside_images_dir() {
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let home = images.join("home");
        for name in [
            "x.Jpg",
            "y.avif",
            "gallery.Jpg",
            ".Jpg",
            "album.Webp",
            "gallery.toml",
        ] {
            write_jpeg(&home.join(name), STUB);
        }
        // A folder named like an image.
        fs::create_dir_all(home.join("album.Jpg")).unwrap();
        write_jpeg(&dir.path().join("outside.Jpg"), STUB);
        let root = images.canonicalize().unwrap();
        // The folder `find_image_file` would list for `image_path`, if any.
        let listed = |image_path: &str| {
            let (path_without_ext, requested) = cut(image_path);
            let (folder, stem) = image_folder(&root, path_without_ext)?;
            lists_folder(&folder, stem, requested, &SOURCE_EXTENSIONS)
                .then(|| (folder.clone(), stem.to_owned()))
        };

        // An image the request names that is there: a mixed spelling, or an AVIF.
        assert_eq!(
            listed("home/x.Jpg"),
            Some((root.join("home"), "x".to_owned()))
        );
        assert_eq!(
            listed("home/y.avif"),
            Some((root.join("home"), "y".to_owned()))
        );
        // Nothing else lists: a folder, a TOML, no extension, a folder named like an
        // image, a made-up name, or an image outside the images dir, whose folder isn't
        // looked in at all.
        for image_path in [
            "home/.",
            "home/gallery.toml",
            "home",
            "home/album.Jpg",
            "home/zz.Jpg",
            "../outside.Jpg",
            "home/../../outside.Jpg",
        ] {
            assert_eq!(listed(image_path), None, "{image_path}");
        }
        for path_without_ext in ["../outside", "home/../../outside", "/etc/passwd"] {
            assert_eq!(
                image_folder(&root, path_without_ext),
                None,
                "{path_without_ext}"
            );
        }

        // A folder is never taken for a file, here or in the `stat`s before a listing.
        fs::create_dir_all(home.join("both.jpg")).unwrap();
        write_jpeg(&home.join("both.webp"), STUB);
        let found = find_image_file(
            images.to_str().unwrap(),
            "home/both",
            Some("jpg"),
            &SOURCE_EXTENSIONS,
        );
        assert_eq!(found, Some(root.join("home/both.webp")));

        // So those find nothing, though a listing would find `gallery.Jpg`, `.Jpg` and
        // `album.Webp`.
        for image_path in ["home/gallery.toml", "home/.", "home/album.Jpg"] {
            let (path_without_ext, requested) = cut(image_path);
            let found = find_image_file(
                images.to_str().unwrap(),
                path_without_ext,
                requested,
                &SOURCE_EXTENSIONS,
            );
            assert_eq!(found, None, "{image_path}");
        }
    }

    /// A GIF of one 1×1 frame whose header declares a `width` × `height` canvas, as a
    /// file of a few dozen bytes can.
    fn gif_declaring(width: u16, height: u16) -> Vec<u8> {
        let mut gif = Vec::new();
        image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]))
            .write_to(&mut std::io::Cursor::new(&mut gif), image::ImageFormat::Gif)
            .unwrap();
        // The logical screen descriptor follows the six-byte signature.
        gif[6..8].copy_from_slice(&width.to_le_bytes());
        gif[8..10].copy_from_slice(&height.to_le_bytes());
        gif
    }

    #[test]
    fn the_decode_limits_admit_the_largest_photos_and_refuse_huge_headers() {
        let check = |width, height, bytes| check_size(width, height, bytes, MAX_PIXELS);
        // The largest photo on the production share, even as 16-bit RGBA (8 bytes a
        // pixel), and 400 MP that way.
        assert!(check(13_021, 21_718, 13_021 * 21_718 * 8).is_ok());
        assert!(check(20_000, 20_000, 20_000 * 20_000 * 8).is_ok());
        // What a GIF or PNG header can claim: 65535 × 65535, 16 GiB as RGBA.
        assert!(check(65_535, 65_535, 65_535 * 65_535 * 4).is_err());
        // A side over 32,768.
        assert!(check(32_769, 1, 32_769 * 4).is_err());
        // Within the sides and the bytes, but over 400 MP.
        assert!(check(20_000, 20_001, 20_000 * 20_001 * 4).is_err());
        // Within the pixels, but over the bytes.
        assert!(check(20_000, 20_000, 20_000 * 20_000 * 8 + 1).is_err());
        // A JPEG XL: 100 MP, not 400.
        assert!(check_size(10_000, 10_000, 10_000 * 10_000 * 16, MAX_JXL_PIXELS).is_ok());
        assert!(check_size(10_000, 10_001, 10_000 * 10_001 * 3, MAX_JXL_PIXELS).is_err());
    }

    #[test]
    fn a_decoder_gets_what_the_picture_leaves_of_the_allowance() {
        // A canvas of 400 MP as RGBA, 1.6 GB, leaves 1.6 GB for a GIF's frame buffer.
        let picture = 20_000 * 20_000 * 4;
        let mut beside = limits_beside(picture).unwrap();
        assert_eq!(beside.max_alloc, Some(MAX_DECODED_BYTES - picture));
        assert!(beside.reserve(MAX_DECODED_BYTES - picture).is_ok());
        assert!(
            limits_beside(picture)
                .unwrap()
                .reserve(MAX_DECODED_BYTES - picture + 1)
                .is_err()
        );
        // A picture that takes it all leaves nothing; one over it is refused.
        assert_eq!(limits_beside(MAX_DECODED_BYTES).unwrap().max_alloc, Some(0));
        assert!(limits_beside(MAX_DECODED_BYTES + 1).is_err());
    }

    #[test]
    fn a_failed_file_is_refused_again_from_its_metadata_until_it_changes() {
        use std::io::Write as _;

        let dir = TempDir::new().unwrap();
        // Refused by the limits, and not an image at all, both settled.
        let canvas = dir.path().join("canvas.gif");
        fs::write(&canvas, gif_declaring(32_769, 1)).unwrap();
        backdate(&canvas);
        let broken = dir.path().join("broken.png");
        fs::write(&broken, b"\x89PNG\r\n\x1a\nnot really").unwrap();
        backdate(&broken);
        for path in [&canvas, &broken] {
            assert!(!failed_before(path));
            assert!(load_standard_image(path).is_none());
            assert!(failed_before(path), "{} wasn't remembered", path.display());
            assert_eq!(process_image(path, 2400, 90), None);
        }

        // Replaced by a picture within the limits, so its length changes.
        fs::write(&canvas, gif_declaring(32_768, 1)).unwrap();
        fs::File::options()
            .append(true)
            .open(&canvas)
            .unwrap()
            .write_all(b"\0")
            .unwrap();
        assert!(!failed_before(&canvas), "a changed file is still refused");
        assert!(load_standard_image(&canvas).is_some());
    }

    #[test]
    fn a_failure_to_read_a_file_is_not_remembered() {
        // A folder opens, but reading it fails: an I/O error, which says nothing about
        // what a file holds. Dated an hour back, so only that keeps it from the memo.
        let dir = TempDir::new().unwrap();
        let folder = dir.path().join("folder.png");
        fs::create_dir(&folder).unwrap();
        fs::File::open(&folder)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_hours(1))
            .unwrap();

        assert!(load_standard_image(&folder).is_none());
        assert!(!failed_before(&folder));
    }

    #[test]
    fn only_an_io_error_other_than_an_early_end_counts_as_a_failure_to_read() {
        use image::error::{DecodingError, ImageError, ImageFormatHint};
        use std::io::{Error, ErrorKind};

        let eio = || Error::from_raw_os_error(5);
        let eof = || Error::from(ErrorKind::UnexpectedEof);
        // As `image` reports them, and as jxl-oxide does, under a decoding error.
        let wrapped =
            |io: Error| ImageError::Decoding(DecodingError::new(ImageFormatHint::Unknown, io));
        assert!(read_failed(&ImageError::IoError(eio())));
        assert!(read_failed(&wrapped(eio())));
        // A file that ends too soon is cut off, which is a property of the file.
        assert!(!read_failed(&ImageError::IoError(eof())));
        assert!(!read_failed(&wrapped(eof())));
        let not_io = ImageError::Decoding(DecodingError::new(ImageFormatHint::Unknown, "bad data"));
        assert!(!read_failed(&not_io));
    }

    #[test]
    fn a_failure_is_remembered_only_once_the_file_has_settled() {
        let dir = TempDir::new().unwrap();
        let broken = dir.path().join("broken.png");
        fs::write(&broken, b"\x89PNG\r\n\x1a\nnot really").unwrap();

        // Just written: it may be part of a file still being written.
        assert!(load_standard_image(&broken).is_none());
        assert!(
            !failed_before(&broken),
            "an unsettled failure was remembered"
        );

        backdate(&broken);
        assert!(load_standard_image(&broken).is_none());
        assert!(
            failed_before(&broken),
            "a settled failure wasn't remembered"
        );
    }

    #[test]
    fn a_file_cut_short_is_remembered_once_settled() {
        let dir = TempDir::new().unwrap();
        // Enough detail that the files aren't tiny.
        let picture = image::RgbImage::from_fn(64, 48, |x, y| {
            let (x, y) = (u8::try_from(x).unwrap(), u8::try_from(y).unwrap());
            image::Rgb([
                x.wrapping_mul(4),
                y.wrapping_mul(5),
                (x ^ y).wrapping_mul(3),
            ])
        });
        let mut cuts = Vec::new();
        for (name, format) in [
            ("cut.png", image::ImageFormat::Png),
            ("cut.webp", image::ImageFormat::WebP),
        ] {
            let mut whole = Vec::new();
            picture
                .write_to(&mut std::io::Cursor::new(&mut whole), format)
                .unwrap();
            // Early, in the header, and late, in the picture.
            for (n, end) in [20, whole.len() * 9 / 10].into_iter().enumerate() {
                let path = dir.path().join(format!("{n}-{name}"));
                fs::write(&path, &whole[..end]).unwrap();
                backdate(&path);
                cuts.push(path);
            }
        }
        let cut_jxl = dir.path().join("cut.jxl");
        fs::write(&cut_jxl, &TINY_JXL[..6]).unwrap();
        backdate(&cut_jxl);

        for path in &cuts {
            assert!(
                load_standard_image(path).is_none(),
                "{} decoded",
                path.display()
            );
            assert!(failed_before(path), "{} wasn't remembered", path.display());
        }
        assert!(load_jxl_image(&cut_jxl).is_none());
        assert!(failed_before(&cut_jxl), "the cut JPEG XL wasn't remembered");
    }

    /// A writer that sets a file's length first and fills it in after: caught partway,
    /// with a tail of zeros, it fails to decode at the final length. Filled in, at the
    /// same length and with the same time, it has to decode, not stay failed.
    #[test]
    fn a_preallocated_file_caught_partway_is_not_stuck() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("loop.gif");
        let whole = animated_gif(3);
        let mut partway = whole[..whole.len() / 3].to_vec();
        partway.resize(whole.len(), 0);
        fs::write(&path, &partway).unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(
            process_image(&path, 2400, 90),
            None,
            "the partway file decoded"
        );

        fs::write(&path, &whole).unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        assert!(
            process_image(&path, 2400, 90).is_some(),
            "stuck on the partway file"
        );
    }

    #[test]
    fn a_gif_declaring_a_canvas_over_the_limits_is_refused_before_it_is_allocated() {
        let dir = TempDir::new().unwrap();
        // Sizes that would be harmless if the limits were lost, so this fails rather
        // than taking the machine's memory: a side over 32,768, and 400 MP and a bit,
        // 1.6 GB as RGBA. A 65535 × 65535 header is refused by the same checks (see
        // `the_decode_limits_admit_the_largest_photos_and_refuse_huge_headers`).
        for (width, height) in [(32_769, 1), (20_000, 20_001)] {
            let path = dir.path().join(format!("{width}x{height}.gif"));
            fs::write(&path, gif_declaring(width, height)).unwrap();
            assert!(
                load_standard_image(&path).is_none(),
                "{width} × {height} was decoded"
            );
        }
    }

    #[test]
    fn a_gif_declaring_a_large_canvas_within_the_limits_decodes() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("canvas.gif");
        fs::write(&path, gif_declaring(6_000, 6_000)).unwrap();

        let picture = load_standard_image(&path).expect("a 6000 × 6000 canvas, 144 MB");
        assert_eq!((picture.width(), picture.height()), (6_000, 6_000));
    }
}
