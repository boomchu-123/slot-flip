use slot_ui::{sticker_lines, StickerFields};

fn fields() -> StickerFields<'static> {
    StickerFields {
        battery: Some(87),
        serial: "04738857",
        version: "0.1.0",
        upstream: "1.9.0",
        model: "RG SP",
    }
}

#[test]
fn the_headline_rows_read_as_a_device_plate() {
    let all = sticker_lines(&fields()).join("\n");
    assert!(
        all.contains("MY355"),
        "the plate names another device: {all}"
    );
    assert!(all.contains("5V"), "{all}");
    assert!(all.contains("1.5A"), "{all}");
    assert!(all.contains("87"), "the gauge reading is missing: {all}");
    assert!(all.contains("0473885"), "the serial went missing: {all}");
}

#[test]
fn the_input_row_carries_the_real_dc_symbol() {
    let all = sticker_lines(&fields()).join("\n");
    assert!(all.contains(slot_ui::DC), "the rating row lost its symbol");
    assert_eq!(slot_ui::DC, '\u{2393}');
}

#[test]
fn a_missing_gauge_is_not_drawn_as_empty() {
    let mut f = fields();
    f.battery = None;
    let all = sticker_lines(&f).join("\n");
    assert!(
        !all.contains("0%"),
        "no gauge was drawn as a flat battery: {all}"
    );
    assert!(
        all.contains("BATTERY"),
        "the row should still be there: {all}"
    );
}

#[test]
fn the_compliance_block_is_the_credits() {
    let all = sticker_lines(&fields()).join("\n").to_uppercase();
    for owed in [
        "MGBA",
        "GPSP",
        "BASEOS BY PVAIBHAV",
        "OPEN SANS",
        "NERD",
        "LCD3X",
        "MARKUS",
    ] {
        assert!(all.contains(owed), "the credits do not mention {owed}");
    }
}

#[test]
fn the_serial_row_matches_the_encoded_hash() {
    let all = sticker_lines(&fields()).join("\n");
    assert!(all.contains("0473885 7"), "{all}");
}

#[test]
fn every_line_is_already_upper_case() {
    for line in sticker_lines(&fields()) {
        assert_eq!(line, line.to_uppercase(), "{line}");
    }
}

#[test]
fn the_device_wordmark_is_drawn_in_its_band_and_antialiased() {
    use slot_ui::sticker_face;
    let face = sticker_face(&fields());
    let at = |x: u32, y: u32| -> u32 {
        let i = ((y * face.w + x) * 4) as usize;
        face.rgba[i] as u32 + face.rgba[i + 1] as u32 + face.rgba[i + 2] as u32
    };
    // The band between the panel and the version line, on the right where the
    // wordmark is right-aligned.
    let band: Vec<u32> = (400..face.w - 12)
        .flat_map(|x| (128..190).map(move |y| (x, y)))
        .map(|(x, y)| at(x, y))
        .collect();
    assert!(
        band.iter().any(|&v| v > 600),
        "nothing was drawn where the wordmark belongs"
    );
    assert!(
        band.iter().any(|&v| (150..600).contains(&v)),
        "the wordmark has no partly lit pixels: its edges are a hard step"
    );
}

#[test]
fn the_version_rides_above_the_where_to_look_line() {
    let lines = sticker_lines(&fields());
    let home = lines
        .iter()
        .position(|l| l == "GITHUB.COM/BOOMCHU-123/SLOT-FLIP")
        .unwrap();
    assert_eq!(
        lines[home - 1],
        format!("VERSION 0.1.0 (SLOT {})", fields().upstream),
        "the sticker must say which upstream slot this build follows"
    );
}

#[test]
fn the_wordmark_is_the_handhelds_own_whatever_the_os_calls_it() {
    let flip = slot_ui::sticker_face(&fields()).rgba;
    for model in ["", "RG SP", "RG35XX SP"] {
        let face = slot_ui::sticker_face(&StickerFields { model, ..fields() });
        assert!(face.rgba == flip, "{model:?} changed the sticker");
    }
}

#[test]
fn the_model_is_set_in_capitals_whatever_baseos_wrote() {
    let upper = slot_ui::sticker_face(&StickerFields {
        model: "RG35XX SP",
        ..fields()
    });
    let lower = slot_ui::sticker_face(&StickerFields {
        model: "rg35xx sp",
        ..fields()
    });
    assert!(upper.rgba == lower.rgba);
}

#[test]
fn a_model_baseos_gives_two_names_shows_the_first() {
    let both = slot_ui::sticker_face(&StickerFields {
        model: "RG35XX Plus / RG35XX 2024",
        ..fields()
    });
    let plus = slot_ui::sticker_face(&StickerFields {
        model: "RG35XX Plus",
        ..fields()
    });
    assert!(both.rgba == plus.rgba);
}
