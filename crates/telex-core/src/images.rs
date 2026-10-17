//! Pictures put into a message being written (pasted, dropped, or Insert picture…), for the
//! HTML editor as `data:` URIs. Settings › Writing › Pictures says how large they are made:
//! as they are, or with their longest side at most 1600 or 800 pixels. A photo made smaller is
//! also turned upright and written again without its hidden details (the camera, where it was
//! taken). GIFs stay as they are, so they keep moving. The send step turns the `data:` URIs
//! into parts of the message (`html::inline_images`). Pictures in a signature are smaller still.

use std::io::Cursor;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use serde::Serialize;

use crate::html::{ImageLimits, SIGNATURE_IMAGES};
use crate::settings::ImageSize;
use crate::tr;

/// A picture ready for the editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Picture {
    /// `data:image/...;base64,...`
    pub src: String,
    pub width: u32,
    pub height: u32,
    /// Its size in the message.
    pub bytes: usize,
    /// It was made smaller than the file.
    pub smaller: bool,
}

/// What the longest side may be.
fn longest(size: ImageSize) -> Option<u32> {
    match size {
        ImageSize::Original => None,
        ImageSize::Large => Some(1600),
        ImageSize::Medium => Some(800),
    }
}

/// The longest side of a picture in a signature (a logo).
const SIGNATURE_SIDE: u32 = 600;

/// Larger pictures aren't decoded: 16384 pixels a side, 256 MiB decoded.
const MAX_SIDE: u32 = 16_384;
const MAX_DECODED: u64 = 256 * 1024 * 1024;

/// JPEG quality for photos written again.
const QUALITY: u8 = 85;

/// Why a picture can't go in.
enum Refused {
    NotAPicture,
    /// Larger than this many bytes, even made smaller.
    TooLarge(usize),
}

pub fn not_a_picture() -> String {
    tr!(
        "That isn't a picture Telex can put in a message: use PNG, JPEG, GIF or WebP.",
        "Tego obrazu Telex nie wstawi do wiadomości: użyj PNG, JPEG, GIF lub WebP."
    )
}

/// Read a picture's bytes and make it the size the setting says.
pub fn prepare(data: &[u8], size: ImageSize) -> Result<Picture, String> {
    within(data, longest(size), ImageLimits::default().each).map_err(|why| match why {
        Refused::NotAPicture => not_a_picture(),
        Refused::TooLarge(_) => too_large(),
    })
}

/// A picture too large for a message, even made smaller.
pub fn too_large() -> String {
    let mb = ImageLimits::default().each / (1024 * 1024);
    tr!(
        "That picture is too large for a message (over {mb} MB). Choose Large or Medium in Settings › Writing, or attach it as a file.",
        "Ten obraz jest za duży do wiadomości (ponad {mb} MB). Wybierz Duże lub Średnie w Ustawieniach › Pisanie albo dołącz go jako plik."
    )
}

/// A picture for a signature: at most 600 pixels, within `html::SIGNATURE_IMAGES`.
pub fn prepare_for_signature(data: &[u8]) -> Result<Picture, String> {
    within(data, Some(SIGNATURE_SIDE), SIGNATURE_IMAGES.each).map_err(|why| match why {
        Refused::NotAPicture => not_a_picture(),
        Refused::TooLarge(limit) => {
            let kb = limit / 1024;
            tr!(
                "A picture in a signature can take {kb} KB. Use a smaller one.",
                "Obraz w podpisie może zająć {kb} KB. Użyj mniejszego."
            )
        }
    })
}

/// The picture with its longest side at most `bound` pixels, in at most `limit` bytes.
fn within(data: &[u8], bound: Option<u32>, limit: usize) -> Result<Picture, Refused> {
    let format = image::guess_format(data).map_err(|_| Refused::NotAPicture)?;
    let mime = match format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Gif => "image/gif",
        ImageFormat::WebP => "image/webp",
        _ => return Err(Refused::NotAPicture),
    };
    let mut reader = ImageReader::with_format(Cursor::new(data), format);
    // A crafted file can claim any size: what decoding it may take is bounded.
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_DECODED);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|_| Refused::NotAPicture)?;
    let (width, height) = decoder.dimensions();
    let fits = bound.is_none_or(|b| width.max(height) <= b);
    // As it is: the original asked for, a GIF (it may move), or a PNG small enough already.
    let keep = bound.is_none()
        || format == ImageFormat::Gif
        || (fits && matches!(format, ImageFormat::Png | ImageFormat::WebP));
    let (mime, bytes, width, height, smaller) = if keep {
        (mime, data.to_vec(), width, height, false)
    } else {
        let orientation = decoder.orientation().ok();
        let mut picture = DynamicImage::from_decoder(decoder).map_err(|_| Refused::NotAPicture)?;
        if let Some(o) = orientation {
            picture.apply_orientation(o);
        }
        let (picture, smaller) = match bound {
            Some(b) if picture.width().max(picture.height()) > b => {
                (picture.resize(b, b, FilterType::Lanczos3), true)
            }
            _ => (picture, false),
        };
        let photo = format == ImageFormat::Jpeg || !has_transparency(&picture);
        let mut out = Vec::new();
        let written = if photo {
            let rgb = DynamicImage::ImageRgb8(picture.to_rgb8());
            JpegEncoder::new_with_quality(&mut out, QUALITY)
                .encode_image(&rgb)
                .is_ok()
        } else {
            picture
                .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
                .is_ok()
        };
        if !written {
            return Err(Refused::NotAPicture);
        }
        let mime = if photo { "image/jpeg" } else { "image/png" };
        (mime, out, picture.width(), picture.height(), smaller)
    };
    if bytes.len() > limit {
        return Err(Refused::TooLarge(limit));
    }
    Ok(Picture {
        src: format!("data:{mime};base64,{}", STANDARD.encode(&bytes)),
        width,
        height,
        bytes: bytes.len(),
        smaller,
    })
}

/// A picture the window read itself (pasted, or dropped from a web page), as base64 or a
/// whole `data:` URI: its bytes.
pub fn pasted(base64: &str) -> Result<Vec<u8>, String> {
    let payload = match base64.split_once(";base64,") {
        Some((head, rest)) if head.starts_with("data:") => rest,
        _ => base64,
    };
    STANDARD.decode(payload.trim()).map_err(|_| not_a_picture())
}

/// Any pixel that isn't fully opaque: such a picture stays a PNG.
fn has_transparency(picture: &DynamicImage) -> bool {
    picture.color().has_alpha() && picture.to_rgba8().pixels().any(|p| p.0[3] < 255)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage, Rgba, RgbaImage};

    fn png(width: u32, height: u32, alpha: u8) -> Vec<u8> {
        let img = RgbaImage::from_pixel(width, height, Rgba([200, 30, 30, alpha]));
        let mut out = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    fn jpeg(width: u32, height: u32) -> Vec<u8> {
        let img = RgbImage::from_fn(width, height, |x, y| {
            Rgb([(x % 255) as u8, (y % 255) as u8, 90])
        });
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, 90)
            .encode_image(&DynamicImage::ImageRgb8(img))
            .unwrap();
        out
    }

    #[test]
    fn large_photos_are_made_smaller_and_small_pictures_stay() {
        let photo = jpeg(3200, 1200);
        let p = prepare(&photo, ImageSize::Large).unwrap();
        assert_eq!((p.width, p.height), (1600, 600));
        assert!(p.smaller && p.src.starts_with("data:image/jpeg;base64,"));
        let p = prepare(&photo, ImageSize::Medium).unwrap();
        assert_eq!((p.width, p.height), (800, 300));
        let p = prepare(&photo, ImageSize::Original).unwrap();
        assert_eq!((p.width, p.height, p.smaller), (3200, 1200, false));
        assert_eq!(p.bytes, photo.len(), "the file as it is");

        // A screenshot that fits stays byte for byte; a large one stays a PNG when it is
        // see-through, and becomes a JPEG when it isn't.
        let small = png(300, 200, 255);
        let p = prepare(&small, ImageSize::Medium).unwrap();
        assert_eq!(
            p.src,
            format!("data:image/png;base64,{}", STANDARD.encode(&small))
        );
        let p = prepare(&png(2000, 1000, 128), ImageSize::Large).unwrap();
        assert!(p.src.starts_with("data:image/png;"));
        assert_eq!((p.width, p.height), (1600, 800));
        let p = prepare(&png(2000, 1000, 255), ImageSize::Large).unwrap();
        assert!(p.src.starts_with("data:image/jpeg;"));
    }

    #[test]
    fn a_signatures_picture_is_small() {
        let p = prepare_for_signature(&jpeg(1200, 400)).unwrap();
        assert_eq!((p.width, p.height), (600, 200));
        assert!(p.bytes <= SIGNATURE_IMAGES.each);
    }

    #[test]
    fn what_isnt_a_picture_is_refused_in_words() {
        let err = prepare(
            b"<svg xmlns='http://www.w3.org/2000/svg'/>",
            ImageSize::Large,
        )
        .unwrap_err();
        assert!(err.contains("PNG, JPEG, GIF or WebP"), "{err}");
        let err = crate::i18n::with(crate::i18n::Lang::Pl, || {
            prepare(b"%PDF-1.4", ImageSize::Large).unwrap_err()
        });
        assert!(err.starts_with("Tego obrazu"), "{err}");
        // A PNG that claims to be 100,000 pixels wide isn't decoded at all.
        let mut huge = png(1, 1, 255);
        huge[16..20].copy_from_slice(&100_000u32.to_be_bytes());
        assert!(prepare(&huge, ImageSize::Large).is_err());
    }
}
