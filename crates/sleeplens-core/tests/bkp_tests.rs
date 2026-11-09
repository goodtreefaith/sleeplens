use sleeplens_core::bkp::{event_type_for_code, parse_bkp, parse_therapy_entry};

fn fixture() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/synthetic-as2x/TEST0000.bkp"
    ))
    .unwrap()
}

#[test]
fn fixture_summary_parity() {
    let bkp = parse_bkp(&fixture());
    assert_eq!(bkp.sessions.len(), 3);
    let a = &bkp.sessions[0];
    assert_eq!(a.session_number, 1);
    assert_eq!(a.start.as_ref().unwrap().iso, "2030-01-15T22:30:00");
    assert_eq!(
        [
            a.usage_minutes,
            a.start_pressure,
            a.pressure.min,
            a.pressure.mean,
            a.pressure.p90
        ],
        [11.0, 5.0, 5.0, 6.4, 8.0]
    );
    assert_eq!(
        [
            a.large_leak_minutes,
            a.snore_index,
            a.ahi,
            a.apnea_index,
            a.central_apnea_index,
            a.apnea2_index,
            a.obstructive_apnea_index,
            a.hypopnea_index,
            a.observed_max_pressure
        ],
        [1.5, 1.2, 16.4, 10.9, 5.5, 5.5, 5.4, 5.5, 8.0]
    );
    assert!(a.has_b5_marker);
    let b = &bkp.sessions[1];
    assert_eq!(
        [
            b.usage_minutes,
            b.large_leak_minutes,
            b.snore_index,
            b.ahi,
            b.apnea_index,
            b.central_apnea_index,
            b.obstructive_apnea_index,
            b.hypopnea_index,
            b.observed_max_pressure
        ],
        [12.0, 0.0, 0.0, 10.0, 5.0, 0.0, 5.0, 5.0, 7.5]
    );
    let c = &bkp.sessions[2];
    assert_eq!(
        [
            c.usage_minutes,
            c.ahi,
            c.central_apnea_index,
            c.obstructive_apnea_index,
            c.snore_index
        ],
        [11.0, 21.8, 10.9, 5.5, 3.5]
    );
}

#[test]
fn sentinel_indices_match_neoread() {
    let mut bytes = fixture();
    let o = bytes
        .as_chunks::<128>()
        .0
        .iter()
        .position(|b| b[..2] == [0xa5, 0xa5])
        .unwrap()
        * 128;
    for offset in [0x5a, 0x5c, 0x5e, 0x60] {
        bytes[o + offset..o + offset + 2].fill(0xff);
    }
    let result = parse_bkp(&bytes);
    let a = &result.sessions[0];
    assert_eq!(
        [
            a.apnea_index,
            a.central_apnea_index,
            a.obstructive_apnea_index,
            a.observed_max_pressure,
            a.hypopnea_index
        ],
        [0.0; 5]
    );
    assert_eq!(a.ahi, 16.4);
}

#[test]
fn fixture_events_settings_text_and_counters() {
    let bkp = parse_bkp(&fixture());
    assert_eq!(bkp.events.len(), 36);
    for (kind, expected) in [
        ("obstructiveApnea", 3),
        ("centralApnea", 3),
        ("hypopnea", 3),
        ("pressureChange", 12),
        ("level", 3),
        ("treatmentStart", 3),
        ("treatmentEnd", 3),
        ("snoreLevel", 2),
        ("snoreCount", 1),
        ("largeLeak", 1),
        ("flowLimitation", 2),
        ("unknown", 0),
    ] {
        assert_eq!(
            bkp.events.iter().filter(|e| e.event_type == kind).count(),
            expected,
            "{kind}"
        );
    }
    let mut levels: Vec<_> = bkp
        .events
        .iter()
        .filter(|e| e.event_type == "level")
        .map(|e| e.value)
        .collect();
    levels.sort();
    assert_eq!(levels, [2, 9, 14]);
    let central = bkp
        .events
        .iter()
        .find(|e| e.event_type == "centralApnea")
        .unwrap();
    assert_eq!(central.time.iso, "2030-01-15T22:34:10");
    assert_eq!(central.pressure, Some(6.5));
    assert_eq!(central.code, 0xe2);
    assert!(bkp
        .events
        .iter()
        .find(|e| e.event_type == "largeLeak")
        .unwrap()
        .pressure
        .is_none());
    assert!(bkp
        .events
        .windows(2)
        .all(|w| w[0].time.epoch_sec <= w[1].time.epoch_sec));
    assert_eq!(
        bkp.events
            .iter()
            .filter(|e| e.time.iso.starts_with("2030-01-17"))
            .count(),
        11
    );
    assert!(bkp
        .events
        .iter()
        .any(|e| e.time.iso == "2030-01-17T21:15:08" && e.event_type == "treatmentStart"));
    assert_eq!(bkp.settings.len(), 3);
    assert_eq!(
        bkp.settings[0].time.as_ref().unwrap().iso,
        "2030-01-15T22:30:00"
    );
    assert_eq!(
        bkp.text_events
            .iter()
            .map(|e| e.text.as_str())
            .collect::<Vec<_>>(),
        [
            "SD card Insert",
            "Parameter change Humidifire 15 to 2",
            "Stop work by Auto Off in WorkScreen",
            "Machine Reset = 6  Software Version TEST-0.00",
            "Screen Filter to Reset"
        ]
    );
    assert_eq!(bkp.stats.empty_blocks, 4);
    assert_eq!(bkp.stats.unknown_blocks, 0);
    assert_eq!(bkp.stats.counter_blocks, 2);
    assert_eq!(
        bkp.counters
            .iter()
            .map(|c| (c.date.as_str(), c.counter_a, c.counter_b, c.counter_c))
            .collect::<Vec<_>>(),
        [
            ("2030-01-15", 1000, 5000, 4999),
            ("2030-01-16", 1200, 5200, 5199),
            ("2030-01-17", 0, 5400, 5399)
        ]
    );
}

#[test]
fn both_firmware_code_families_and_entry_layouts() {
    for (nibble, kind) in [
        (0, "pressureChange"),
        (1, "obstructiveApnea"),
        (2, "centralApnea"),
        (3, "hypopnea"),
        (4, "snoreLevel"),
        (5, "largeLeak"),
        (6, "flowLimitation"),
        (7, "snoreCount"),
        (14, "treatmentStart"),
        (15, "treatmentEnd"),
    ] {
        assert_eq!(event_type_for_code(0xa0 | nibble), Some(kind));
        assert_eq!(event_type_for_code(0xe0 | nibble), Some(kind));
    }
    assert_eq!(event_type_for_code(0xe8), Some("level"));
    for code in [0xa8, 0xe9, 0xaa, 0xab, 0xac, 0xad] {
        assert_eq!(event_type_for_code(code), Some("unknown"));
    }
    for code in [0, 0x9f, 0xb0, 0xc5, 0xdf, 0xf0, 0xff] {
        assert_eq!(event_type_for_code(code), None);
    }
    let new = parse_therapy_entry(&[30, 15, 2, 3, 0xa2, 1, 2, 12], 0).unwrap();
    assert_eq!(new.time.iso, "2030-02-03T01:02:15");
    assert_eq!(new.event_type, "centralApnea");
    assert_eq!(new.pressure, Some(6.0));
    let old = parse_therapy_entry(&[0xee, 7, 2, 3, 0xa1, 1, 2, 12], 0).unwrap();
    assert_eq!(old.time.iso, "2030-02-03T01:02:00");
    for bytes in [
        [30, 15, 2, 3, 0xf1, 1, 2, 12],
        [30, 15, 2, 3, 0x91, 1, 2, 12],
        [0xff, 15, 2, 3, 0xa1, 1, 2, 12],
        [30, 15, 2, 31, 0xa1, 1, 2, 12],
    ] {
        assert!(parse_therapy_entry(&bytes, 0).is_none());
    }
    assert!(parse_therapy_entry(&[], usize::MAX).is_none());
}

#[test]
fn validates_whole_event_blocks_and_ignores_partial_trailers() {
    let mut block = [0xff; 128];
    block[..8].copy_from_slice(&[30, 0, 3, 1, 0xae, 22, 0, 0]);
    block[8..16].copy_from_slice(&[30, 0, 3, 1, 0xa4, 23, 0, 4]);
    block[16..24].copy_from_slice(&[30, 0, 3, 2, 0xaf, 6, 0, 0]);
    let mut bytes = fixture();
    bytes.extend(block);
    let result = parse_bkp(&bytes);
    assert_eq!(result.events.len(), 39);
    assert_eq!(result.stats.unknown_blocks, 0);
    assert_eq!(result.sessions.len(), 3);
    assert_eq!(result.settings.len(), 3);
    assert_eq!(result.counters.len(), 3);
    assert_eq!(result.text_events.len(), 5);
    assert_eq!(parse_bkp(&block).stats.skipped_entries, 13);
    block[24..32].copy_from_slice(&[30, 0, 3, 2, 0xb1, 6, 0, 0]);
    assert_eq!(parse_bkp(&block).stats.unknown_blocks, 1);
    assert!(parse_bkp(&block).events.is_empty());
    let mut partial = block.to_vec();
    partial.extend([0; 127]);
    assert_eq!(parse_bkp(&partial).stats.blocks, 1);
}

#[test]
fn counter_block_requires_both_halves_valid_or_empty() {
    let bytes = fixture();
    let mut block = bytes
        .as_chunks::<128>()
        .0
        .iter()
        .find(|b| parse_bkp(*b).stats.counter_blocks == 1)
        .unwrap()
        .to_vec();
    block[127] = 0x77;
    let result = parse_bkp(&block);
    assert_eq!(result.stats.counter_blocks, 0);
    assert_eq!(result.stats.unknown_blocks, 1);
    assert!(result.counters.is_empty());
    block[64..].fill(0);
    assert_eq!(parse_bkp(&block).counters.len(), 1);
    block[8..12].fill(0);
    block[12..16].fill(0xff);
    assert!(parse_bkp(&block).counters.is_empty());
}
