use sleeplens_core::{dat::*, settings::*, time::*};
fn fixture(date: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/../../fixtures/synthetic-as2x/TEST0000203001/{date}.dat",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}
#[test]
fn header_and_settings_match_reference() {
    let h = parse_dat_header(&fixture("20300115"));
    assert_eq!(
        (
            h.format_version,
            h.records_supported,
            h.device_type,
            h.record_count,
            h.trailing_bytes
        ),
        (2, true, 1, 600, 0)
    );
    assert_eq!(
        (
            h.device_model.as_str(),
            h.model_name.as_deref(),
            h.serial.as_str()
        ),
        ("AS2X", Some("AS100A"), "TEST0000")
    );
    assert_eq!(h.software_version.as_deref(), Some("9.01.00.00"));
    assert_eq!(h.hardware_version.as_deref(), Some("2.00"));
    assert_eq!(h.created.unwrap().iso, "2030-01-15T22:30:00");
    let s = h.settings.unwrap();
    assert_eq!(s.pressure_fields, [50, 60, 45, 80]);
    assert_eq!(
        (
            s.start_pressure,
            s.fixed_pressure,
            s.min_pressure,
            s.max_pressure
        ),
        (5., 6., 4.5, 8.)
    );
    assert_eq!(
        (
            s.mode,
            s.mode_label.as_str(),
            s.comfort_time,
            s.comfort_level
        ),
        (1, "Auto CPAP", 3, 2)
    );
    assert_eq!(
        (
            s.leak_alert,
            s.humidifier_level,
            s.ramp_minutes,
            s.auto_on,
            s.auto_off
        ),
        (1, 4, 15, true, false)
    );
    assert_eq!(
        (s.pressure_unit, s.pressure_unit_label.as_str(), s.raw[0x2e]),
        (1, "hPa", 3)
    );
    assert_eq!(s.time.unwrap().iso, "2030-01-15T22:30:00");
    let mut b = vec![0; 128];
    b[..2].fill(0xc5);
    b[2] = 7;
    assert_eq!(parse_settings_block(&b, 0).unwrap().mode_label, "unknown");
    b[..2].fill(0);
    assert!(parse_settings_block(&b, 0).is_none());
    assert!(parse_settings_block(&b, usize::MAX).is_none());
    for unit in [0, 2, 255] {
        assert_eq!(pressure_unit_label(unit), "unknown");
    }
    assert_eq!(pressure_unit_label(1), "hPa");
}
#[test]
fn model_and_version_formatting() {
    for (id, name) in MODEL_NAMES {
        assert_eq!(model_name_for_id(id), Some(name));
    }
    assert_eq!(model_name_for_id("as4x"), Some("AS100A-R"));
    assert_eq!(model_name_for_id("AS9X"), None);
    assert_eq!(model_name_for_id(""), None);
    for (bytes, expected) in [
        ([35, 1, 1, 0], "1.01.35"),
        ([4, 3, 2, 1], "1.02.03.04"),
        ([5, 1, 0, 0], "1.05"),
    ] {
        assert_eq!(format_device_version(&bytes).as_deref(), Some(expected));
    }
    assert_eq!(format_device_version(&[0; 4]), None);
    assert_eq!(format_device_version(&[255; 4]), None);
}
#[test]
fn streams_channels_and_clock_drift() {
    let b = fixture("20300115");
    let mut it = iterate_dat_records(&b, true);
    let r = it.next().unwrap();
    assert_eq!(r.time.unwrap().iso, "2030-01-15T22:30:00");
    assert_eq!(r.weekday, 1);
    assert_eq!(
        (r.set_pressure, r.total_leak, r.excess_leak),
        (5., 20., 10.)
    );
    assert_eq!(r.total_leak, r.leak);
    assert_eq!(r.excess_leak, r.leak_flow);
    assert!(r.snore > 0.);
    assert!(r.flow_amplitude > 20.);
    let w = r.waveforms.unwrap();
    assert_eq!(w.set_pressure.len(), 50);
    assert_eq!(w.set_pressure[0], 5.);
    assert_eq!(&w.leak_flow[..2], &[9., 11.]);
    assert!(w.pressure_feedback[0] > 5.);
    assert!(w.patient_flow.iter().any(|&f| f > 15.));
    let records: Vec<_> = iterate_dat_records(&b, false).collect();
    assert_eq!(
        records[341].time.as_ref().unwrap().epoch_sec
            - records[340].time.as_ref().unwrap().epoch_sec,
        2
    );
    assert!(records[125].flow_amplitude < 10.);
    assert!(records[100].flow_amplitude > 15.);
}
#[test]
fn summaries_match_reference() {
    let s = summarize_dat(&fixture("20300115"), true);
    assert_eq!(
        (
            s.record_count,
            s.record_seconds,
            s.invalid_records,
            s.usage_seconds
        ),
        (600, 600, 0, 601)
    );
    assert_eq!(s.usage_minutes, 601. / 60.);
    assert_eq!(s.sessions.len(), 1);
    assert_eq!(s.start.unwrap().iso, "2030-01-15T22:30:00");
    assert_eq!(s.end.unwrap().iso, "2030-01-15T22:40:01");
    assert_eq!(
        s.pressure,
        PressureStats {
            min: 5.,
            mean: 6.4,
            p90: 8.,
            p95: 8.,
            max: 8.
        }
    );
    assert_eq!(
        s.total_leak,
        LeakStats {
            median: 24.,
            p95: 29.,
            max: 29.
        }
    );
    assert_eq!(
        s.excess_leak,
        LeakStats {
            median: 15.,
            p95: 95.,
            max: 95.
        }
    );
    assert_eq!(s.leak, s.total_leak);
    assert_eq!(s.leak_flow, s.excess_leak);
    let series = s.series.unwrap();
    assert_eq!(series.pressure.len(), 600);
    assert_eq!(
        (
            series.pressure[0],
            series.pressure[599],
            series.total_leak[3],
            series.excess_leak[450],
            series.excess_leak[3]
        ),
        (5., 8., 23., 95., 13.)
    );
    assert_eq!(series.leak, series.total_leak);
    assert_eq!(series.leak_flow, series.excess_leak);
    let s = summarize_dat(&fixture("20300116"), false);
    assert!(s.series.is_none());
    assert_eq!(s.usage_minutes, 12.);
    assert_eq!(s.end.unwrap().iso, "2030-01-16T23:17:00");
    assert_eq!(
        s.pressure,
        PressureStats {
            min: 5.,
            mean: 6.,
            p90: 7.5,
            p95: 7.5,
            max: 7.5
        }
    );
    assert_eq!(
        s.total_leak,
        LeakStats {
            median: 35.,
            p95: 41.,
            max: 41.
        }
    );
    assert_eq!(
        s.excess_leak,
        LeakStats {
            median: 25.,
            p95: 31.,
            max: 31.
        }
    );
}
#[test]
fn pauses_split_sessions_but_drift_does_not() {
    let s = summarize_dat(&fixture("20300117"), false);
    assert_eq!(s.record_count, 600);
    assert_eq!(s.usage_seconds, 601);
    assert_eq!(s.sessions.len(), 2);
    assert_eq!(s.pressure.p90, 7.);
    assert_eq!(
        s.end.as_ref().unwrap().epoch_sec - s.start.as_ref().unwrap().epoch_sec,
        2401
    );
    assert_eq!(
        s.sessions
            .iter()
            .map(|s| (s.start.iso.as_str(), s.end.iso.as_str(), s.minutes))
            .collect::<Vec<_>>(),
        vec![
            ("2030-01-17T21:15:08", "2030-01-17T21:20:09", 301. / 60.),
            ("2030-01-17T21:50:09", "2030-01-17T21:55:09", 5.)
        ]
    );
}
#[test]
fn truncated_invalid_and_v1_inputs_are_safe() {
    for n in [0, 1, 8, 80, 96, 2047] {
        let b = vec![0; n];
        assert_eq!(iterate_dat_records(&b, false).count(), 0);
        assert_eq!(parse_dat_header(&b).record_count, 0);
    }
    let source = fixture("20300115");
    let mut b = source[..DAT_HEADER_SIZE + DAT_RECORD_SIZE].to_vec();
    b.push(0x4d);
    assert_eq!(parse_dat_header(&b).trailing_bytes, 1);
    assert_eq!(iterate_dat_records(&b, false).count(), 1);
    b[DAT_HEADER_SIZE] = 0;
    assert!(!iterate_dat_records(&b, false).next().unwrap().valid);
    assert_eq!(summarize_dat(&b, false).invalid_records, 1);
    b[DAT_HEADER_SIZE] = 0x4d;
    b[DAT_HEADER_SIZE + 6] = 13;
    assert!(!iterate_dat_records(&b, false).next().unwrap().valid);
    let mut b = vec![0; DAT_HEADER_SIZE + 512 * 3 + 10];
    b[..DAT_HEADER_SIZE].copy_from_slice(&source[..DAT_HEADER_SIZE]);
    b[8] = 1;
    let h = parse_dat_header(&b);
    assert_eq!(
        (h.records_supported, h.record_count, h.trailing_bytes),
        (false, 3, 10)
    );
    assert_eq!(iterate_dat_records(&b, false).count(), 0);
    let s = summarize_dat(&b, true);
    assert_eq!(s.record_seconds, 0);
    assert!(s.start.is_none());
}
#[test]
fn signed_channels_and_histogram_saturation() {
    let mut b = fixture("20300115")[..DAT_HEADER_SIZE + DAT_RECORD_SIZE].to_vec();
    for k in 0..50 {
        for slot in [slot::SNORE + k, slot::LEAK_FLOW + k] {
            let o = DAT_HEADER_SIZE + 12 + 2 * slot;
            b[o..o + 2].copy_from_slice(&(-10i16).to_le_bytes());
        }
        let o = DAT_HEADER_SIZE + 12 + 2 * (slot::SET_PRESSURE + k);
        b[o..o + 2].copy_from_slice(&65535u16.to_le_bytes());
    }
    let r = iterate_dat_records(&b, true).next().unwrap();
    assert_eq!((r.snore, r.excess_leak), (-10., -10.));
    assert_eq!(r.waveforms.unwrap().snore[0], -10.);
    let s = summarize_dat(&b, false);
    assert_eq!(s.pressure.p90, 204.7);
    assert_eq!(s.excess_leak.median, 0.);
    assert_eq!(s.excess_leak.max, 0.);
}
#[test]
fn timestamp_validation_and_rollovers() {
    assert!(make_local_time(2030, 2, 31, 0, 0, 0).is_none());
    assert!(make_local_time(1999, 1, 1, 0, 0, 0).is_none());
    assert!(make_local_time(2030, 1, 1, 24, 0, 0).is_none());
    assert!(read_timestamp(&[0; 6], 0).is_none());
    assert!(read_timestamp(&[], usize::MAX).is_none());
    let t = make_local_time(2030, 1, 1, 0, 0, 0).unwrap();
    assert_eq!(add_seconds(&t, -1).iso, "2029-12-31T23:59:59");
    assert_eq!(date_key(&t), "20300101");
}

#[test]
fn pressure_waveforms_round_once_into_float32() {
    let mut b = fixture("20300115")[..DAT_HEADER_SIZE + DAT_RECORD_SIZE].to_vec();
    for (channel, value) in [
        (slot::SET_PRESSURE, 21u16),
        (slot::PRESSURE_FEEDBACK, 73u16),
        (slot::MASK_PRESSURE, 21u16),
    ] {
        let offset = DAT_HEADER_SIZE + 12 + 2 * channel;
        b[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    let w = iterate_dat_records(&b, true)
        .next()
        .unwrap()
        .waveforms
        .unwrap();
    assert_eq!(
        w.set_pressure[0].to_bits(),
        ((21f64 / 10.) as f32).to_bits()
    );
    assert_eq!(
        w.pressure_feedback[0].to_bits(),
        ((73f64 / 10.) as f32).to_bits()
    );
    assert_eq!(
        w.mask_pressure[0].to_bits(),
        ((21f64 / 10.) as f32).to_bits()
    );
}
