//! Dark HTML mail: the colors a message sets, turned around for the dark theme.
//!
//! Mail is designed for white paper. In the dark theme the frame is dark, so a message's
//! own colors are recolored instead of inverting the whole page: light backgrounds become
//! dark and dark text becomes light, keeping each color's hue, in OKLab (a color space
//! where lightness looks even across hues). Backgrounds that are already dark and text that
//! is already light stay as they are, so a black band with white text keeps its look.
//! Images are never touched.
//!
//! Each dark theme has its own paper and ink: white turns into the theme's background and
//! black into its text color, hue and all, so a message looks like part of the theme.

/// What a color paints, which decides where it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    Text,
    Background,
    Border,
}

/// Backgrounds lighter than this (OKLab L, 0–1) turn dark; darker ones stay.
const BG_TURN: f64 = 0.5;
/// Text darker than this turns light; lighter text stays.
const TEXT_TURN: f64 = 0.62;
/// Below this chroma a color counts as gray, and takes on the paper's or the ink's tint.
const GRAY: f64 = 0.04;

/// Where colors go: the dark theme's paper (what white becomes) and ink (what black becomes),
/// in OKLab.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Target {
    paper: (f64, f64, f64),
    ink: (f64, f64, f64),
}

impl Target {
    /// From the theme's background and text colors (`#14171b`, `#e2e6eb`), or `None` when
    /// either isn't a color or the pair isn't dark paper with light ink.
    pub(crate) fn new(paper: &str, ink: &str) -> Option<Self> {
        let lab = |c: &str| {
            let [r, g, b, _] = parse_color(c)?;
            Some(to_oklab([r, g, b]))
        };
        let (paper, ink) = (lab(paper)?, lab(ink)?);
        (paper.0 < BG_TURN - 0.05 && ink.0 > TEXT_TURN + 0.05).then_some(Self { paper, ink })
    }
}

/// Recolor every color in an inline style (`color:#333; background:#fff`) for the dark
/// frame. Properties that don't paint (fonts, sizes) are left alone, and so are `url(…)`s.
pub(crate) fn recolor_style(style: &str, to: &Target) -> String {
    split_top_level(style, ';')
        .into_iter()
        .map(|decl| match decl.split_once(':') {
            Some((prop, value)) => match role_of(prop) {
                Some(role) => format!("{prop}:{}", recolor_value(value, role, to, 0)),
                None => decl.to_owned(),
            },
            None => decl.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// An HTML color attribute (`bgcolor="#ffffff"`, `<font color="navy">`), recolored.
/// Values that aren't colors stay as they are.
pub(crate) fn recolor_attribute(value: &str, role: Role, to: &Target) -> String {
    let v = value.trim();
    // Old HTML accepts hex without the #: bgcolor="ffffff".
    let parsed = parse_color(v).or_else(|| {
        (v.len() == 6 && v.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| parse_color(&format!("#{v}")))
            .flatten()
    });
    parsed.map_or_else(|| value.to_owned(), |c| css(recolor_rgba(c, role, to)))
}

/// A color the frame's own style may use: `#rgb` or `#rrggbb` and nothing else, since it
/// goes into CSS as it is.
pub(crate) fn is_plain_hex(s: &str) -> bool {
    s.strip_prefix('#')
        .is_some_and(|h| matches!(h.len(), 3 | 6) && h.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn role_of(property: &str) -> Option<Role> {
    let p = property.trim().to_ascii_lowercase();
    if p == "color" || p.starts_with("text-decoration") || p == "caret-color" {
        Some(Role::Text)
    } else if p.starts_with("background") {
        Some(Role::Background)
    } else if p.starts_with("border") || p.starts_with("outline") || p.starts_with("column-rule") {
        Some(Role::Border)
    } else {
        None
    }
}

/// How deep functions inside functions are recolored; deeper ones are left as they are, so a
/// value made to be deep can't exhaust the stack.
const MAX_NESTING: u8 = 8;

/// Replace each color in a CSS value, e.g. `1px solid #ccc` or `linear-gradient(#fff, red)`.
fn recolor_value(value: &str, role: Role, to: &Target, depth: u8) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(c) = rest.chars().next() {
        if c == '#' {
            let len = 1 + rest[1..]
                .bytes()
                .take_while(u8::is_ascii_alphanumeric)
                .count();
            let token = &rest[..len];
            match parse_color(token) {
                Some(rgba) => out.push_str(&css(recolor_rgba(rgba, role, to))),
                None => out.push_str(token),
            }
            rest = &rest[len..];
        } else if c.is_ascii_alphabetic() || c == '-' {
            let len = rest
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'-')
                .count();
            let word = &rest[..len];
            let after = &rest[len..];
            let lower = word.to_ascii_lowercase();
            if after.starts_with('(') {
                let closed = matching_paren(after);
                let close = closed.unwrap_or(after.len());
                let call = &rest[..len + close];
                if matches!(lower.as_str(), "rgb" | "rgba" | "hsl" | "hsla") {
                    match parse_color(call) {
                        Some(rgba) => out.push_str(&css(recolor_rgba(rgba, role, to))),
                        None => out.push_str(call),
                    }
                } else if lower == "url" || depth >= MAX_NESTING {
                    out.push_str(call); // an image, never recolored; or too deep
                } else {
                    // Another function, e.g. a gradient: recolor what is inside. One never
                    // closed runs to the end of the value.
                    out.push_str(word);
                    out.push('(');
                    let inner = match closed {
                        Some(c) => &after[1..c - 1],
                        None => after.strip_prefix('(').unwrap_or(after),
                    };
                    out.push_str(&recolor_value(inner, role, to, depth + 1));
                    if closed.is_some() {
                        out.push(')');
                    }
                }
                rest = &rest[len + close..];
            } else {
                match named(&lower) {
                    Some(rgb) => out.push_str(&css(recolor_rgba(rgb, role, to))),
                    None => out.push_str(word),
                }
                rest = after;
            }
        } else {
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// Length of `(…)` at the start of `s`, the closing parenthesis included, or `None` when
/// it is never closed.
fn matching_paren(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split on `sep` outside quotes and parentheses (a data: URI in `url("…")` has `;`).
fn split_top_level(s: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut quote, mut start) = (0i32, None, 0);
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => depth -= 1,
            (None, c) if c == sep && depth <= 0 => {
                parts.push(&s[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

// ---- Reading colors ----

/// An opaque color as `#rrggbb` (lowercase), whatever way CSS wrote it; `None` otherwise.
pub(crate) fn opaque_hex(s: &str) -> Option<String> {
    let rgba = parse_color(s)?;
    (rgba[3] == 255).then(|| hex(rgba))
}

/// `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, `hsl()`, `hsla()` or a color
/// name, as red, green, blue and alpha.
fn parse_color(s: &str) -> Option<[u8; 4]> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex(hex);
    }
    let lower = s.to_ascii_lowercase();
    if lower.bytes().all(|b| b.is_ascii_lowercase()) {
        return named(&lower);
    }
    let (func, args) = lower.split_once('(')?;
    let args = args.strip_suffix(')')?;
    // rgb(1, 2, 3), rgb(1 2 3 / 50%) and the same with alpha
    let parts: Vec<&str> = args
        .split([',', '/', ' '])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if !(3..=4).contains(&parts.len()) {
        return None;
    }
    let alpha = match parts.get(3) {
        Some(a) => (number(a, 1.0)? * 255.0).round().clamp(0.0, 255.0) as u8,
        None => 255,
    };
    let rgb = match func.trim() {
        "rgb" | "rgba" => {
            let channel = |p: &str| number(p, 255.0).map(|v| v.round().clamp(0.0, 255.0) as u8);
            [channel(parts[0])?, channel(parts[1])?, channel(parts[2])?]
        }
        "hsl" | "hsla" => {
            let hue = parts[0].trim_end_matches("deg").parse::<f64>().ok()?;
            hsl_to_rgb(hue, number(parts[1], 1.0)?, number(parts[2], 1.0)?)
        }
        _ => return None,
    };
    Some([rgb[0], rgb[1], rgb[2], alpha])
}

/// A number, or a percentage of `full` ("50%" of 255 is 127.5).
fn number(s: &str, full: f64) -> Option<f64> {
    match s.strip_suffix('%') {
        Some(p) => p.parse::<f64>().ok().map(|v| v / 100.0 * full),
        None => s.parse().ok(),
    }
}

fn parse_hex(hex: &str) -> Option<[u8; 4]> {
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let digit = |i: usize| u8::from_str_radix(&hex[i..=i], 16).ok().map(|d| d * 17);
    let pair = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    match hex.len() {
        3 => Some([digit(0)?, digit(1)?, digit(2)?, 255]),
        4 => Some([digit(0)?, digit(1)?, digit(2)?, digit(3)?]),
        6 => Some([pair(0)?, pair(2)?, pair(4)?, 255]),
        8 => Some([pair(0)?, pair(2)?, pair(4)?, pair(6)?]),
        _ => None,
    }
}

fn hsl_to_rgb(hue: f64, sat: f64, light: f64) -> [u8; 3] {
    let h = hue.rem_euclid(360.0) / 360.0;
    let (s, l) = (sat.clamp(0.0, 1.0), light.clamp(0.0, 1.0));
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let channel = |t: f64| {
        let t = t.rem_euclid(1.0);
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round() as u8
    };
    [channel(h + 1.0 / 3.0), channel(h), channel(h - 1.0 / 3.0)]
}

/// The CSS color names, each with its hex value.
const NAMES: &str =
    "aliceblue f0f8ff antiquewhite faebd7 aqua 00ffff aquamarine 7fffd4 azure f0ffff \
beige f5f5dc bisque ffe4c4 black 000000 blanchedalmond ffebcd blue 0000ff blueviolet 8a2be2 \
brown a52a2a burlywood deb887 cadetblue 5f9ea0 chartreuse 7fff00 chocolate d2691e coral ff7f50 \
cornflowerblue 6495ed cornsilk fff8dc crimson dc143c cyan 00ffff darkblue 00008b darkcyan 008b8b \
darkgoldenrod b8860b darkgray a9a9a9 darkgreen 006400 darkgrey a9a9a9 darkkhaki bdb76b \
darkmagenta 8b008b darkolivegreen 556b2f darkorange ff8c00 darkorchid 9932cc darkred 8b0000 \
darksalmon e9967a darkseagreen 8fbc8f darkslateblue 483d8b darkslategray 2f4f4f \
darkslategrey 2f4f4f darkturquoise 00ced1 darkviolet 9400d3 deeppink ff1493 deepskyblue 00bfff \
dimgray 696969 dimgrey 696969 dodgerblue 1e90ff firebrick b22222 floralwhite fffaf0 \
forestgreen 228b22 fuchsia ff00ff gainsboro dcdcdc ghostwhite f8f8ff gold ffd700 \
goldenrod daa520 gray 808080 green 008000 greenyellow adff2f grey 808080 honeydew f0fff0 \
hotpink ff69b4 indianred cd5c5c indigo 4b0082 ivory fffff0 khaki f0e68c lavender e6e6fa \
lavenderblush fff0f5 lawngreen 7cfc00 lemonchiffon fffacd lightblue add8e6 lightcoral f08080 \
lightcyan e0ffff lightgoldenrodyellow fafad2 lightgray d3d3d3 lightgreen 90ee90 lightgrey d3d3d3 \
lightpink ffb6c1 lightsalmon ffa07a lightseagreen 20b2aa lightskyblue 87cefa \
lightslategray 778899 lightslategrey 778899 lightsteelblue b0c4de lightyellow ffffe0 lime 00ff00 \
limegreen 32cd32 linen faf0e6 magenta ff00ff maroon 800000 mediumaquamarine 66cdaa \
mediumblue 0000cd mediumorchid ba55d3 mediumpurple 9370db mediumseagreen 3cb371 \
mediumslateblue 7b68ee mediumspringgreen 00fa9a mediumturquoise 48d1cc mediumvioletred c71585 \
midnightblue 191970 mintcream f5fffa mistyrose ffe4e1 moccasin ffe4b5 navajowhite ffdead \
navy 000080 oldlace fdf5e6 olive 808000 olivedrab 6b8e23 orange ffa500 orangered ff4500 \
orchid da70d6 palegoldenrod eee8aa palegreen 98fb98 paleturquoise afeeee palevioletred db7093 \
papayawhip ffefd5 peachpuff ffdab9 peru cd853f pink ffc0cb plum dda0dd powderblue b0e0e6 \
purple 800080 rebeccapurple 663399 red ff0000 rosybrown bc8f8f royalblue 4169e1 \
saddlebrown 8b4513 salmon fa8072 sandybrown f4a460 seagreen 2e8b57 seashell fff5ee \
sienna a0522d silver c0c0c0 skyblue 87ceeb slateblue 6a5acd slategray 708090 slategrey 708090 \
snow fffafa springgreen 00ff7f steelblue 4682b4 tan d2b48c teal 008080 thistle d8bfd8 \
tomato ff6347 turquoise 40e0d0 violet ee82ee wheat f5deb3 white ffffff whitesmoke f5f5f5 \
yellow ffff00 yellowgreen 9acd32";

fn named(lower: &str) -> Option<[u8; 4]> {
    let mut words = NAMES.split_ascii_whitespace();
    while let (Some(name), Some(hex)) = (words.next(), words.next()) {
        if name == lower {
            return parse_hex(hex);
        }
    }
    None
}

// ---- Turning colors around ----

fn recolor_rgba([r, g, b, a]: [u8; 4], role: Role, to: &Target) -> [u8; 4] {
    let (l, ca, cb) = to_oklab([r, g, b]);
    // Backgrounds move toward the paper and text toward the ink. A light line on white
    // paper (a table rule) becomes a dark one; a dark line becomes light, so it still shows.
    let toward_paper = match role {
        Role::Background => true,
        Role::Text => false,
        Role::Border => l >= 0.5,
    };
    let (lightness, end) = if toward_paper {
        (dark_background(l, to.paper.0), to.paper)
    } else {
        (light_text(l, to.ink.0), to.ink)
    };
    // Grays that move take on the tint of the paper or the ink, so white becomes exactly
    // the theme's background. Colors keep their hue.
    let moved = (lightness - l).abs() > 1e-9;
    let gray = if moved {
        (1.0 - ca.hypot(cb) / GRAY).max(0.0)
    } else {
        0.0
    };
    let a_ = ca * (1.0 - gray) + end.1 * gray;
    let b_ = cb * (1.0 - gray) + end.2 * gray;
    let [r, g, b] = from_oklab_in_gamut(lightness, a_, b_);
    [r, g, b, a]
}

/// Light backgrounds go dark, white to the paper; dark ones stay.
fn dark_background(l: f64, paper: f64) -> f64 {
    if l <= BG_TURN {
        l
    } else {
        BG_TURN - (l.min(1.0) - BG_TURN) * (BG_TURN - paper) / (1.0 - BG_TURN)
    }
}

/// Dark text goes light, black to the ink; light text stays. The curve is flat near black,
/// so the usual near-black grays all end up close to the ink and stay easy to read.
fn light_text(l: f64, ink: f64) -> f64 {
    if l >= TEXT_TURN {
        l
    } else {
        ink - (ink - TEXT_TURN) * (l.max(0.0) / TEXT_TURN).powi(2)
    }
}

fn css([r, g, b, a]: [u8; 4]) -> String {
    if a == 255 {
        hex([r, g, b, a])
    } else {
        format!("rgba({r},{g},{b},{:.3})", f64::from(a) / 255.0)
    }
}

fn hex([r, g, b, _]: [u8; 4]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn to_linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(c: f64) -> f64 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB to OKLab (Björn Ottosson's matrices).
fn to_oklab([r, g, b]: [u8; 3]) -> (f64, f64, f64) {
    let (r, g, b) = (to_linear(r), to_linear(g), to_linear(b));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    (
        0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    )
}

/// OKLab to linear sRGB, which may be outside 0–1.
fn from_oklab(l: f64, a: f64, b: f64) -> [f64; 3] {
    let l_ = (l + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m_ = (l - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s_ = (l - 0.089_484_177_5 * a - 1.291_485_548 * b).powi(3);
    [
        4.076_741_662_1 * l_ - 3.307_711_591_3 * m_ + 0.230_969_929_2 * s_,
        -1.268_438_004_6 * l_ + 2.609_757_401_1 * m_ - 0.341_319_396_5 * s_,
        -0.004_196_086_3 * l_ - 0.703_418_614_7 * m_ + 1.707_614_701 * s_,
    ]
}

/// The color with lightness `l` and this hue, with as much of its chroma as sRGB can show.
fn from_oklab_in_gamut(l: f64, a: f64, b: f64) -> [u8; 3] {
    let fits = |k: f64| {
        from_oklab(l, a * k, b * k)
            .iter()
            .all(|c| (-1e-4..=1.0 + 1e-4).contains(c))
    };
    let mut k = 1.0;
    if !fits(k) {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..20 {
            let mid = (lo + hi) / 2.0;
            if fits(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        k = lo;
    }
    from_oklab(l, a * k, b * k).map(|c| (from_linear(c.clamp(0.0, 1.0)) * 255.0).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lightness(css: &str) -> f64 {
        let [r, g, b, _] = parse_color(css).unwrap();
        to_oklab([r, g, b]).0
    }

    #[test]
    fn colors_are_read_in_every_spelling() {
        assert_eq!(parse_color("#fff"), Some([255, 255, 255, 255]));
        assert_eq!(parse_color("#1A2b3C"), Some([0x1a, 0x2b, 0x3c, 255]));
        assert_eq!(parse_color("#00000080"), Some([0, 0, 0, 128]));
        assert_eq!(parse_color("rgb(10, 20, 30)"), Some([10, 20, 30, 255]));
        assert_eq!(parse_color("rgba(10,20,30,0.5)"), Some([10, 20, 30, 128]));
        assert_eq!(parse_color("rgb(10 20 30 / 50%)"), Some([10, 20, 30, 128]));
        assert_eq!(parse_color("RGB(100%, 0%, 0%)"), Some([255, 0, 0, 255]));
        assert_eq!(parse_color("hsl(120, 100%, 25%)"), Some([0, 128, 0, 255]));
        assert_eq!(named("navy"), Some([0, 0, 128, 255]));
        assert_eq!(parse_color("#ggg"), None);
        assert_eq!(parse_color("rgb(1,2)"), None);
    }

    /// Telex's Dark theme: its surface and its text.
    fn dark() -> Target {
        Target::new("#14171b", "#e2e6eb").unwrap()
    }

    #[test]
    fn hostile_styles_are_survived() {
        // A function never closed, ending in a letter that takes two bytes.
        assert!(recolor_style("color: x(ą", &dark()).starts_with("color: x(ą"));
        // Nesting made to exhaust the stack: deep levels are left as they are.
        let deep = format!("background: {}#fff", "x(".repeat(20_000));
        assert!(recolor_style(&deep, &dark()).ends_with("#fff"));
        let shallow = recolor_style("background: x(y(#ffffff))", &dark());
        assert!(!shallow.contains("#ffffff"), "{shallow}");
    }

    #[test]
    fn white_paper_and_black_ink_become_the_themes() {
        let to = dark();
        assert_eq!(
            recolor_attribute("#ffffff", Role::Background, &to),
            "#14171b"
        );
        assert_eq!(recolor_attribute("black", Role::Text, &to), "#e2e6eb");
        // Fjord is blue and Phosphor green: white takes on their tint too.
        let fjord = Target::new("#151e28", "#e3ebf3").unwrap();
        assert_eq!(
            recolor_attribute("white", Role::Background, &fjord),
            "#151e28"
        );
        let phosphor = Target::new("#08100b", "#c6f5d2").unwrap();
        assert_eq!(recolor_attribute("#000", Role::Text, &phosphor), "#c6f5d2");
        // Near-black text ends up close to the ink, easy to read.
        assert!(lightness(&recolor_attribute("#333333", Role::Text, &to)) > 0.8);
    }

    #[test]
    fn only_dark_paper_with_light_ink_is_a_target() {
        assert!(Target::new("#ffffff", "#000000").is_none(), "a light theme");
        assert!(Target::new("#14171b", "#2a2a2a").is_none(), "dark on dark");
        assert!(Target::new("red; x", "#fff").is_none());
        assert!(is_plain_hex("#14171b") && is_plain_hex("#fff"));
        assert!(!is_plain_hex("#fff;}body{") && !is_plain_hex("red") && !is_plain_hex("#ffff"));
    }

    #[test]
    fn dark_backgrounds_and_light_text_stay() {
        let to = dark();
        assert_eq!(
            recolor_attribute("#000000", Role::Background, &to),
            "#000000"
        );
        assert_eq!(
            recolor_attribute("#1a237e", Role::Background, &to),
            "#1a237e"
        );
        assert_eq!(recolor_attribute("#ffffff", Role::Text, &to), "#ffffff");
    }

    #[test]
    fn hue_survives() {
        let to = dark();
        // A pale yellow highlight becomes a dark yellow one, not a gray or a blue.
        let [r, g, b, _] =
            parse_color(&recolor_attribute("#fff3c4", Role::Background, &to)).unwrap();
        assert!(r > b && g > b, "still yellowish: {r} {g} {b}");
        assert!(lightness(&format!("#{r:02x}{g:02x}{b:02x}")) < 0.35);
        // A dark red text becomes a light red.
        let [r, g, b, _] = parse_color(&recolor_attribute("#8b0000", Role::Text, &to)).unwrap();
        assert!(r > g && r > b, "still red: {r} {g} {b}");
    }

    #[test]
    fn styles_are_recolored_by_property() {
        let s = recolor_style(
            "color:#333333; background-color: white; font-family: Silver, Arial; \
             border:1px solid #dddddd; width:600px",
            &dark(),
        );
        assert!(s.contains("background-color: #14171b"), "{s}");
        assert!(s.contains("font-family: Silver, Arial"), "fonts stay: {s}");
        assert!(s.contains("width:600px"), "{s}");
        assert!(!s.contains("#333333") && !s.contains("#dddddd"), "{s}");
        assert!(s.contains("1px solid #"), "{s}");
    }

    #[test]
    fn gradients_are_recolored_and_images_are_not() {
        let s = recolor_style(
            "background: url(\"data:image/png;base64,AAAA\") no-repeat, linear-gradient(to right, #fff, #eeeeee)",
            &dark(),
        );
        assert!(s.contains("url(\"data:image/png;base64,AAAA\")"), "{s}");
        assert!(s.contains("linear-gradient(to right, #"), "{s}");
        assert!(!s.contains("#fff,") && !s.contains("#eeeeee"), "{s}");
        assert!(s.ends_with(')'), "{s}");
    }

    #[test]
    fn transparency_is_kept() {
        let s = recolor_style("background:rgba(255,255,255,0.5)", &dark());
        assert!(s.starts_with("background:rgba("), "{s}");
        assert!(s.ends_with(",0.502)"), "{s}");
    }

    #[test]
    fn things_that_are_not_colors_stay() {
        let to = dark();
        assert_eq!(
            recolor_attribute("transparent", Role::Background, &to),
            "transparent"
        );
        assert_eq!(
            recolor_attribute("ffffff", Role::Background, &to),
            "#14171b"
        );
        assert_eq!(recolor_style("color:inherit", &to), "color:inherit");
        assert_eq!(recolor_style("color:#12", &to), "color:#12");
        assert_eq!(recolor_style("background:url(x", &to), "background:url(x");
    }
}
