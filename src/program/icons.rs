//! The icons a terminal draws beside the program's name, for a dark and a light background. A
//! terminal draws only the bytes it is given, so they travel as `data:` URIs.

use pencere::{Icon, IconTheme};

const ICONS: [(&[u8], IconTheme); 4] = [
    (
        include_bytes!("../assets/icon-32-dark.png"),
        IconTheme::Dark,
    ),
    (
        include_bytes!("../assets/icon-64-dark.png"),
        IconTheme::Dark,
    ),
    (
        include_bytes!("../assets/icon-32-light.png"),
        IconTheme::Light,
    ),
    (
        include_bytes!("../assets/icon-64-light.png"),
        IconTheme::Light,
    ),
];

/// `bytes` in base64 (RFC 4648), padded.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let group = chunk.iter().enumerate().fold(0_u32, |group, (at, byte)| {
            group | u32::from(*byte) << (16 - 8 * at)
        });
        for at in 0..4 {
            if at <= chunk.len() {
                out.push(char::from(ALPHABET[(group >> (18 - 6 * at) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A PNG as an icon: a `data:` URI, and the size its header says, by which a terminal picks the
/// icon it wants.
fn icon(png: &[u8], theme: IconTheme) -> Icon {
    let mut icon = Icon::new(format!("data:image/png;base64,{}", base64(png)));
    icon.mime_type = Some("image/png".to_owned());
    icon.theme = Some(theme);
    // After the signature and the length and name of the `IHDR` chunk, which comes first.
    let number = |at: usize| {
        png.get(at..at + 4)
            .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
            .map(u32::from_be_bytes)
    };
    if png.get(12..16) == Some(b"IHDR") {
        if let (Some(width), Some(height)) = (number(16), number(20)) {
            icon.sizes = vec![format!("{width}x{height}")];
        }
    }
    icon
}

/// The program's icons, for [`pencere::Options::icon`].
pub fn all() -> Vec<Icon> {
    ICONS.iter().map(|(png, theme)| icon(png, *theme)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_is_rfc_4648() {
        for (plain, coded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(plain.as_bytes()), coded, "{plain:?}");
        }
    }

    #[test]
    fn each_icon_says_its_size_and_its_ground() {
        let icons = all();
        assert_eq!(icons.len(), 4);
        let sizes: Vec<&str> = icons.iter().map(|icon| icon.sizes[0].as_str()).collect();
        assert_eq!(sizes, ["32x32", "64x64", "32x32", "64x64"]);
        assert!(
            icons
                .iter()
                .all(|icon| icon.src.starts_with("data:image/png;base64,iVBOR"))
        );
    }
}
