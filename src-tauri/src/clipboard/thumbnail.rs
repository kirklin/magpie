//! Thumbnail generation for image entries.
//!
//! The history list renders each image entry as a 24pt icon, but pointed an
//! `<img>` straight at the full-size capture. A WebView has to decode the WHOLE
//! image into an uncompressed bitmap before it can scale it down, so a 3360x2158
//! screenshot cost ~27 MB of resident memory to show as a 24pt square — and
//! WebKit's decoded-image cache does not release that promptly when the row
//! scrolls out of view. With ~1000 captures on disk that is >12 GB of potential
//! bitmap, which is exactly how the app ended up sitting on gigabytes of RSS.
//!
//! So we pre-shrink instead: every capture also writes a `thumbs/{hash}.png`
//! whose longest edge is `THUMB_MAX_EDGE`, and the list loads that. A thumbnail
//! decodes to at most ~256 KB, roughly a 100x reduction per row.
//!
//! Downscaling itself is streamed row-by-row (see `downscale_png`) so *making*
//! the thumbnail never materializes the full bitmap either — otherwise the fix
//! would reintroduce the very spike it exists to avoid.

use std::io::BufReader;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

/// Longest edge of a generated thumbnail, in pixels.
///
/// The list icon is 24pt, so 256px covers even a 3x Retina scale with room to
/// spare while decoding to ~256 KB (256*256*4) in the worst case.
pub const THUMB_MAX_EDGE: u32 = 256;

/// Directory holding generated thumbnails, as a sibling of `clipboard_images`.
/// Kept separate so the orphan/retention logic that walks `clipboard_images`
/// never mistakes a thumbnail for an original capture.
pub fn thumbs_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("thumbs");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Map an original image path to its thumbnail path.
///
/// Keyed by the original's file stem, which for our own captures IS the content
/// hash — so identical content shares one thumbnail, and a pruned entry's
/// thumbnail is as unreferenced as its original.
pub fn thumb_path_for(app_handle: &AppHandle, image_path: &Path) -> Result<PathBuf, String> {
    thumb_path_in(&thumbs_dir(app_handle)?, image_path)
}

/// Resolve a thumbnail path inside `dir`. Split out from `thumb_path_for` so the
/// invariant that actually keeps captures safe — the result always lands in the
/// thumbnails directory and can never alias the original — is testable without
/// an `AppHandle`.
fn thumb_path_in(dir: &Path, image_path: &Path) -> Result<PathBuf, String> {
    let stem = image_path
        .file_stem()
        .ok_or_else(|| "image path has no file name".to_string())?
        .to_string_lossy()
        .to_string();
    Ok(dir.join(format!("{stem}.png")))
}

/// Ensure a thumbnail exists for `image_path`, returning the path the UI should
/// load.
///
/// Returns the ORIGINAL path when a thumbnail would be pointless or impossible:
/// the source is already small enough, or it is not a PNG we can decode. The
/// caller can therefore use the result unconditionally.
pub fn ensure_thumbnail(app_handle: &AppHandle, image_path: &Path) -> Result<PathBuf, String> {
    if !image_path.exists() {
        return Err(format!("source image not found: {}", image_path.display()));
    }

    let dst = thumb_path_for(app_handle, image_path)?;
    // Already generated. Filenames are content hashes for our own captures, so
    // an existing thumbnail always matches the current bytes; for foreign files
    // we still refresh when the source is newer than the thumbnail.
    if dst.exists() && !is_stale(image_path, &dst) {
        return Ok(dst);
    }

    match downscale_png(image_path, &dst, THUMB_MAX_EDGE) {
        Ok(true) => Ok(dst),
        // Source is already at or below the target size: no thumbnail written,
        // and loading the original costs no more than loading a copy of it.
        Ok(false) => Ok(image_path.to_path_buf()),
        Err(e) => Err(e),
    }
}

/// Delete the thumbnail belonging to `image_path`, if any.
///
/// Must be called wherever an original capture is removed (entry delete, clear,
/// retention prune). Thumbnails live outside `clipboard_images`, so nothing else
/// would ever reclaim them and they would accumulate exactly like the orphaned
/// originals already on disk. Failure is ignored: a leftover thumbnail is
/// harmless next to a failed user-visible delete.
pub fn remove_for_image(app_handle: &AppHandle, image_path: &Path) {
    if let Ok(thumb) = thumb_path_for(app_handle, image_path) {
        // `thumb_path_for` always resolves under `thumbs/`, never to the
        // original in `clipboard_images/`, so this can only ever delete a
        // generated thumbnail. Removing the capture itself is the caller's job.
        let _ = std::fs::remove_file(thumb);
    }
}

/// True when `src` has been modified more recently than `dst`.
/// Unreadable timestamps are treated as "not stale" so a metadata hiccup can't
/// cause an endless regeneration loop.
fn is_stale(src: &Path, dst: &Path) -> bool {
    let Ok(src_m) = std::fs::metadata(src).and_then(|m| m.modified()) else {
        return false;
    };
    let Ok(dst_m) = std::fs::metadata(dst).and_then(|m| m.modified()) else {
        return false;
    };
    src_m > dst_m
}

/// Box-filter downscale of a PNG so its longest edge is at most `max_edge`.
///
/// Returns `Ok(false)` (writing nothing) when the source already fits.
///
/// Rows are consumed one at a time and accumulated straight into the small
/// destination buffer, so peak memory is one source row (a few KB) plus the
/// accumulator (~1 MB at 256x256) rather than the full decoded bitmap. That
/// matters: the images this exists for decode to tens of MB each.
fn downscale_png(src: &Path, dst: &Path, max_edge: u32) -> Result<bool, String> {
    let file = std::fs::File::open(src).map_err(|e| e.to_string())?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    // Normalize palette/grayscale/16-bit sources down to plain 8-bit samples so
    // the accumulation loop only ever deals with 1-4 bytes per pixel.
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);

    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let (src_w, src_h) = (reader.info().width, reader.info().height);
    let interlaced = reader.info().interlaced;
    let channels = reader.output_color_type().0.samples();
    if src_w == 0 || src_h == 0 {
        return Err("image has zero dimension".to_string());
    }

    // Nothing to gain if it already fits.
    if src_w <= max_edge && src_h <= max_edge {
        return Ok(false);
    }

    let scale = f64::from(src_w.max(src_h)) / f64::from(max_edge);
    let dst_w = ((f64::from(src_w) / scale).round() as u32).max(1);
    let dst_h = ((f64::from(src_h) / scale).round() as u32).max(1);

    let px = (dst_w as usize) * (dst_h as usize);
    let mut acc = vec![0f32; px * 4];
    let mut counts = vec![0u32; px];

    // Fold one source row into the accumulator. Each source pixel lands in the
    // destination cell it maps to; averaging by the per-cell count at the end
    // makes this an area-average (box) filter, which avoids the aliasing that
    // nearest-neighbour sampling would give on text-heavy screenshots.
    let mut accumulate = |y: u32, data: &[u8]| {
        let dy = ((u64::from(y) * u64::from(dst_h)) / u64::from(src_h)) as u32;
        for x in 0..src_w {
            let si = (x as usize) * channels;
            if si + channels > data.len() {
                break;
            }
            let (r, g, b, a) = match channels {
                1 => (data[si], data[si], data[si], 255),
                2 => (data[si], data[si], data[si], data[si + 1]),
                3 => (data[si], data[si + 1], data[si + 2], 255),
                _ => (data[si], data[si + 1], data[si + 2], data[si + 3]),
            };
            let dx = ((u64::from(x) * u64::from(dst_w)) / u64::from(src_w)) as u32;
            let cell = (dy as usize) * (dst_w as usize) + (dx as usize);
            let di = cell * 4;
            acc[di] += f32::from(r);
            acc[di + 1] += f32::from(g);
            acc[di + 2] += f32::from(b);
            acc[di + 3] += f32::from(a);
            counts[cell] += 1;
        }
    };

    if interlaced {
        // Interlaced PNGs arrive as Adam7 passes, so rows don't stream in
        // top-to-bottom order. Rare enough (we never write them; only a pasted
        // foreign file could be one) that decoding the whole frame is fine.
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
        let stride = info.line_size;
        for y in 0..src_h {
            let start = (y as usize) * stride;
            let Some(row) = buf.get(start..start + stride) else {
                break;
            };
            accumulate(y, row);
        }
    } else {
        let mut y = 0u32;
        while let Some(row) = reader.next_row().map_err(|e| e.to_string())? {
            accumulate(y, row.data());
            y += 1;
            if y >= src_h {
                break;
            }
        }
    }

    // Average each cell. A cell with no samples (possible only from rounding at
    // the very edge) falls back to fully transparent rather than garbage.
    let mut out = vec![0u8; px * 4];
    for cell in 0..px {
        let n = counts[cell];
        if n == 0 {
            continue;
        }
        let di = cell * 4;
        let n = n as f32;
        for c in 0..4 {
            out[di + c] = (acc[di + c] / n).round().clamp(0.0, 255.0) as u8;
        }
    }

    write_png_atomically(dst, &out, dst_w, dst_h)
}

/// Write RGBA bytes as a PNG via a temp file + rename.
///
/// The rename is what makes a half-written thumbnail unobservable: two rows
/// requesting the same thumbnail concurrently can both generate it, but a
/// reader only ever sees a complete file.
fn write_png_atomically(dst: &Path, rgba: &[u8], width: u32, height: u32) -> Result<bool, String> {
    let tmp = dst.with_extension(format!("tmp{}", std::process::id()));
    {
        let file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(rgba).map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
    }
    match std::fs::rename(&tmp, dst) {
        Ok(()) => Ok(true),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("magpie_thumb_{}_{}", tag, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Write an RGBA PNG of a solid colour, plus a distinct top-left quadrant so
    /// downscaling has something structural to preserve.
    fn write_test_png(path: &Path, w: u32, h: u32) {
        let mut rgba = vec![0u8; (w as usize) * (h as usize) * 4];
        for y in 0..h {
            for x in 0..w {
                let i = ((y as usize) * (w as usize) + (x as usize)) * 4;
                let left = x < w / 2;
                rgba[i] = if left { 255 } else { 0 };
                rgba[i + 1] = 0;
                rgba[i + 2] = if left { 0 } else { 255 };
                rgba[i + 3] = 255;
            }
        }
        write_png_atomically(path, &rgba, w, h).unwrap();
    }

    fn png_dims(path: &Path) -> (u32, u32) {
        let decoder = png::Decoder::new(std::fs::File::open(path).unwrap());
        let reader = decoder.read_info().unwrap();
        (reader.info().width, reader.info().height)
    }

    #[test]
    fn downscales_large_image_and_preserves_aspect_ratio() {
        let dir = tmp_dir("large");
        let src = dir.join("src.png");
        let dst = dir.join("dst.png");
        // 4:1 aspect, well over the cap on the long edge.
        write_test_png(&src, 1600, 400);

        assert!(downscale_png(&src, &dst, 256).unwrap(), "thumbnail written");
        let (w, h) = png_dims(&dst);
        assert_eq!(w, 256, "long edge clamped to max_edge");
        assert_eq!(h, 64, "aspect ratio preserved");

        // The whole point: the thumbnail must be dramatically cheaper to decode.
        // Bitmap cost scales with AREA, so the saving is ~scale^2 — here the long
        // edge shrinks 1600/256 = 6.25x, giving ~39x less bitmap. Real captures
        // are far lopsided-er in our favour: a 3360x2158 screenshot lands at
        // 256x164, i.e. ~172x less.
        let src_bitmap = 1600 * 400 * 4;
        let dst_bitmap = w * h * 4;
        assert!(
            dst_bitmap * 30 < src_bitmap,
            "decoded thumbnail should be >30x smaller: {dst_bitmap} vs {src_bitmap}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Deleting an entry removes its thumbnail; it must never be able to reach
    /// the original capture. The originals are the user's actual data — pasting
    /// and the preview pane both read them — so this separation is load-bearing.
    #[test]
    fn thumbnail_path_never_aliases_the_original_capture() {
        let images = Path::new("/data/clipboard_images");
        let thumbs = Path::new("/data/thumbs");
        for name in ["abc123.png", "deadbeef0badf00d.png", "x.png"] {
            let original = images.join(name);
            let thumb = thumb_path_in(thumbs, &original).unwrap();
            assert_ne!(thumb, original, "thumbnail must never resolve to the capture itself");
            assert!(thumb.starts_with(thumbs), "thumbnail must stay under thumbs/: {}", thumb.display());
        }
    }

    #[test]
    fn leaves_already_small_images_alone() {
        let dir = tmp_dir("small");
        let src = dir.join("src.png");
        let dst = dir.join("dst.png");
        write_test_png(&src, 64, 64);

        assert!(!downscale_png(&src, &dst, 256).unwrap(), "no thumbnail needed");
        assert!(!dst.exists(), "nothing written for an already-small image");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Measures the real saving against an actual `clipboard_images` directory.
    /// Ignored by default (depends on local data); run explicitly with:
    ///   MAGPIE_IMAGES=~/Library/Application\ Support/com.magpie.clipboard/clipboard_images \
    ///     cargo test --lib real_corpus -- --ignored --nocapture
    #[test]
    #[ignore = "requires a local clipboard_images corpus via MAGPIE_IMAGES"]
    fn real_corpus_thumbnails_shrink_decoded_bitmaps() {
        let Ok(src_dir) = std::env::var("MAGPIE_IMAGES") else {
            panic!("set MAGPIE_IMAGES to a clipboard_images directory");
        };
        let out = tmp_dir("corpus");
        let (mut n, mut src_bitmap, mut dst_bitmap, mut dst_disk, mut skipped) = (0u64, 0u64, 0u64, 0u64, 0u64);

        for entry in std::fs::read_dir(&src_dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("png") {
                continue;
            }
            let Ok((sw, sh)) = std::panic::catch_unwind(|| png_dims(&path)) else {
                continue;
            };
            let dst = out.join(path.file_name().unwrap());
            match downscale_png(&path, &dst, THUMB_MAX_EDGE) {
                Ok(true) => {
                    let (dw, dh) = png_dims(&dst);
                    n += 1;
                    src_bitmap += u64::from(sw) * u64::from(sh) * 4;
                    dst_bitmap += u64::from(dw) * u64::from(dh) * 4;
                    dst_disk += std::fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
                    let _ = std::fs::remove_file(&dst);
                }
                _ => skipped += 1,
            }
        }

        let gb = |b: u64| b as f64 / (1024.0 * 1024.0 * 1024.0);
        println!(
            "\n{n} images thumbnailed ({skipped} already small)\n\
             decoded bitmap: {:.2} GB -> {:.3} GB  ({:.0}x less)\n\
             thumbnails on disk: {:.3} GB",
            gb(src_bitmap),
            gb(dst_bitmap),
            src_bitmap as f64 / dst_bitmap.max(1) as f64,
            gb(dst_disk),
        );
        assert!(dst_bitmap * 10 < src_bitmap, "expected a large reduction");

        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn averages_colours_rather_than_point_sampling() {
        let dir = tmp_dir("avg");
        let src = dir.join("src.png");
        let dst = dir.join("dst.png");
        write_test_png(&src, 1024, 1024);
        downscale_png(&src, &dst, 8).unwrap();

        // Left half is red, right half is blue. Sample both halves of the
        // thumbnail and check the dominant channel survived the reduction.
        let decoder = png::Decoder::new(std::fs::File::open(&dst).unwrap());
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).unwrap();
        let (w, _h) = (info.width as usize, info.height as usize);

        let px = |x: usize, y: usize| {
            let i = (y * w + x) * 4;
            (buf[i], buf[i + 1], buf[i + 2], buf[i + 3])
        };
        let (lr, _, lb, la) = px(0, 0);
        let (rr, _, rb, _) = px(w - 1, 0);
        assert!(lr > lb, "left stays red-dominant ({lr} vs {lb})");
        assert!(rb > rr, "right stays blue-dominant ({rb} vs {rr})");
        assert_eq!(la, 255, "opaque source stays opaque");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
