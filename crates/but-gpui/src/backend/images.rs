//! The images a pull request's description shows. Only GitHub's own hosts are asked, as Lite
//! asks: GitHub fetches other hosts' images itself for the pages it shows, so they can't learn
//! who reads a description, but the Markdown names them as they are, and asking them would
//! tell them. A redirect is followed only to GitHub and the bucket it keeps uploads in, as
//! Lite allows. A private repository's images need a sign-in, so those are asked again with
//! the token of each GitHub account added, which goes to GitHub only: a redirect to another
//! host, as to that bucket, leaves it behind.

use std::io::Cursor;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use anyhow::{Context as _, Result, ensure};
use image::ImageDecoder as _;

use super::{Picture, runtime};

/// The most an image is read to; GitHub takes uploads up to 10 MB.
const MAX_BYTES: usize = 16 << 20;

/// The most memory decoding one may take, enough for 8,000 pixels square.
const MAX_DECODED: u64 = 256 << 20;

/// The most pixels an image may have across or down, as many as a GPU texture holds.
const MAX_SIDE: u32 = 16_384;

/// The most pixels an image keeps across and down once decoded: a description is at most
/// about 1,000 points wide, twice that in pixels on a Retina screen.
const KEPT: (u32, u32) = (2048, 4096);

/// Where GitHub keeps uploads, which an attachment's address redirects to.
const UPLOADS: &str = "github-production-user-asset-6210df.s3.amazonaws.com";

/// Whether `url` is an image the app fetches: one on GitHub, over HTTPS.
pub fn on_github(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url.trim()) else {
        return false;
    };
    let github = |host: &str| host == "github.com" || host.ends_with(".githubusercontent.com");
    url.scheme() == "https" && url.host_str().is_some_and(github)
}

/// Whether a redirect to `url` is followed: to GitHub or its uploads, over HTTPS.
fn followed(url: &reqwest::Url) -> bool {
    on_github(url.as_str()) || (url.scheme() == "https" && url.host_str() == Some(UPLOADS))
}

/// The image at `url`, decoded, its first frame only when it moves, and shrunk to the most a
/// description shows of it.
pub fn picture(url: &str) -> Result<Picture> {
    ensure!(on_github(url), "Only images on GitHub are shown");
    let bytes = match fetch(url, None) {
        Ok(bytes) => bytes,
        // GitHub answers as if a private repository's image weren't there.
        Err(err) if hidden(&err) => signed_in(url).ok_or(err)?,
        Err(err) => return Err(err),
    };
    decode(&bytes)
}

fn hidden(err: &anyhow::Error) -> bool {
    let status = err
        .downcast_ref::<reqwest::Error>()
        .and_then(|err| err.status());
    status.is_some_and(|status| matches!(status.as_u16(), 401 | 403 | 404))
}

/// What `url` serves to the first GitHub account added that may see it. The keychain is read
/// only here, so a public image never asks for it.
fn signed_in(url: &str) -> Option<Vec<u8>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir().ok()?);
    let accounts = but_github::list_known_github_accounts(&storage).ok()?;
    // An Enterprise account's token is for its own server.
    (accounts
        .iter()
        .filter(|account| account.custom_host().is_none()))
    .find_map(|account| {
        let token = but_github::get_gh_access_token(account, &storage).ok()??;
        fetch(url, Some(&token.0)).ok()
    })
}

/// What `url` serves, given 30 seconds and up to [`MAX_BYTES`].
fn fetch(url: &str, token: Option<&str>) -> Result<Vec<u8>> {
    static CLIENT: LazyLock<Option<reqwest::Client>> = LazyLock::new(|| {
        let redirects = reqwest::redirect::Policy::custom(|attempt| {
            match attempt.previous().len() < 10 && followed(attempt.url()) {
                true => attempt.follow(),
                false => attempt.stop(),
            }
        });
        reqwest::Client::builder().redirect(redirects).build().ok()
    });
    let client = CLIENT.as_ref().context("Couldn't set up fetching images")?;
    runtime().handle().block_on(async {
        let mut request = client.get(url).timeout(Duration::from_secs(30));
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let mut response = request.send().await?.error_for_status()?;
        let too_big = "The image is too big to show";
        let size = response.content_length().unwrap_or(0);
        ensure!(size <= MAX_BYTES as u64, too_big);
        let mut bytes = Vec::with_capacity(size as usize);
        while let Some(chunk) = response.chunk().await? {
            bytes.extend_from_slice(&chunk);
            ensure!(bytes.len() <= MAX_BYTES, too_big);
        }
        Ok(bytes)
    })
}

/// `bytes` as a picture drawn a point a pixel, as GitHub draws an image no HTML sizes.
fn decode(bytes: &[u8]) -> Result<Picture> {
    use image::ImageFormat as F;
    // One at a time, so a description's images can't take more than MAX_DECODED together.
    static DECODING: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
    let _one_at_a_time = DECODING.lock();
    let format = image::guess_format(bytes)?;
    ensure!(
        matches!(format, F::Png | F::Jpeg | F::Gif | F::WebP),
        "Not a picture GitButler draws"
    );
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_DECODED);
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits.clone());
    let mut decoder = reader.into_decoder()?;
    // Decoders check only the sides, so what decoding would take is checked here, before
    // it's taken: a few kilobytes can say they're a gigabyte of pixels.
    limits.reserve(decoder.total_bytes())?;
    let orientation = decoder.orientation()?;
    let mut image = image::DynamicImage::from_decoder(decoder)?;
    // Shrunk before it's turned, which copies it, and as it'll stand once turned.
    use image::metadata::Orientation as O;
    let (across, down) = (image.width(), image.height());
    let ((width, height), kept) = match orientation {
        O::Rotate90 | O::Rotate270 | O::Rotate90FlipH | O::Rotate270FlipH => {
            ((down, across), (KEPT.1, KEPT.0))
        }
        _ => ((across, down), KEPT),
    };
    if across > kept.0 || down > kept.1 {
        image = image.thumbnail(kept.0, kept.1);
    }
    image.apply_orientation(orientation);
    let mut buffer = image.into_rgba8();
    // gpui draws BGRA.
    for pixel in buffer.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Ok(Picture {
        image: Arc::new(gpui::RenderImage::new([image::Frame::new(buffer)])),
        width: width as f32,
        height: height as f32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_github_is_asked() {
        for url in [
            "https://github.com/user-attachments/assets/2326d61c",
            "https://raw.githubusercontent.com/o/r/sha/a.png",
            "https://private-user-images.githubusercontent.com/1/a.png?jwt=x",
        ] {
            assert!(on_github(url), "{url} is GitHub's");
        }
        for url in [
            "http://github.com/user-attachments/assets/2326d61c",
            "https://github.com.example.com/a.png",
            "https://evilgithubusercontent.com/a.png",
            "https://example.com/?u=https://github.com/a.png",
            "data:image/png;base64,AAAA",
            "../docs/a.png",
        ] {
            assert!(!on_github(url), "{url} would tell its host who read it");
        }
        let follows = |url: &str| reqwest::Url::parse(url).is_ok_and(|url| followed(&url));
        assert!(
            follows("https://github-production-user-asset-6210df.s3.amazonaws.com/1/a.png"),
            "an attachment redirects to GitHub's uploads"
        );
        assert!(
            !follows("https://evil.s3.amazonaws.com/a.png")
                && !follows("http://github-production-user-asset-6210df.s3.amazonaws.com/a.png"),
            "a redirect anywhere else stops"
        );
    }

    #[test]
    fn a_big_image_keeps_its_size_and_fewer_pixels() {
        let mut png = Vec::new();
        image::RgbaImage::new(3000, 100)
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("encodes");
        let picture = decode(&png).expect("decodes");
        assert_eq!(
            (picture.width, picture.height),
            (3000., 100.),
            "it's drawn as wide as it is"
        );
        assert_eq!(
            picture.image.size(0).width.0,
            2048,
            "but keeps only the pixels a description can show"
        );
    }

    #[test]
    fn a_small_file_that_decodes_huge_is_refused_before_it_takes_the_memory() {
        let mut webp = Vec::new();
        image::RgbaImage::new(1, 1)
            .write_to(&mut Cursor::new(&mut webp), image::ImageFormat::WebP)
            .expect("encodes");
        // The VP8L header after its signature byte: 14 bits each of width and height less one.
        let at = 21;
        let header = u32::from_le_bytes(webp[at..at + 4].try_into().expect("four bytes"));
        let huge = (header & 0xf000_0000) | (11_999 << 14) | 11_999;
        webp[at..at + 4].copy_from_slice(&huge.to_le_bytes());
        let err = decode(&webp).expect_err("12,000 pixels square is more than a decode may take");
        assert!(
            matches!(
                err.downcast_ref::<image::ImageError>(),
                Some(image::ImageError::Limits(_))
            ),
            "refused for its size, not for its data: {err:#}"
        );
    }
}
