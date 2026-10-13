#![no_main]
use libfuzzer_sys::fuzz_target;
use std::collections::HashMap;
use telex_core::{html, mime};

fuzz_target!(|data: &[u8]| {
    let _ = mime::parse_message(data, "local/fuzz", None);
    let _ = mime::attachments_of(data);
    let _ = mime::html_body(data);
    let _ = html::render(data, &HashMap::new(), &html::Paper::Light);
    let _ = html::render(data, &HashMap::new(), &html::Paper::Dark(html::Palette::default()));
    let text = String::from_utf8_lossy(data);
    let _ = html::compose_clean(&text);
    let _ = html::to_text(&text);
});
