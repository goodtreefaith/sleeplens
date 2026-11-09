use sleeplens_core::{
    assemble_card, detect::detect_as2x, evening_date, model::Source, source::CardSource,
    stream_card, time::make_local_time,
};
use std::{collections::BTreeMap, io};

fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/synthetic-as2x")
}
struct MemoryCard(BTreeMap<String, Vec<u8>>);
impl CardSource for MemoryCard {
    fn list(&self) -> io::Result<Vec<String>> {
        Ok(self.0.keys().cloned().collect())
    }
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        self.0
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }
}
fn memory_card() -> MemoryCard {
    let mut files = BTreeMap::new();
    for name in [
        "TEST0000.bkp",
        "TEST0000203001/20300115.dat",
        "TEST0000203001/20300116.dat",
        "TEST0000203001/20300117.dat",
    ] {
        files.insert(name.into(), std::fs::read(fixture().join(name)).unwrap());
    }
    MemoryCard(files)
}
#[test]
fn app_records_prefer_machine_values_and_align_wall_clock_gaps() {
    let import = assemble_card(&memory_card()).unwrap();
    assert_eq!(import.nights.len(), 3);
    let device = import.metadata.device.unwrap();
    assert_eq!(device.serial_masked, "•••00");
    assert_eq!(device.model, "AS100A");
    assert_eq!(device.pressure_unit, "hPa");
    let first = &import.nights[0];
    assert_eq!(first.list.date, "2030-01-15");
    assert_eq!(first.list.usage_minutes, 11.0);
    assert_eq!(first.list.usage_source, Source::Machine);
    assert_eq!(first.list.pressure_mean, Some(6.4));
    assert_eq!(first.list.pressure_p90, Some(8.0));
    assert_eq!(first.list.excess_leak_median, Some(15.0));
    assert_eq!(first.list.event_counts.obstructive, 1);
    assert_eq!(first.list.event_counts.central, 1);
    assert_eq!(first.list.event_counts.hypopnea, 1);
    assert_eq!(first.summary.ai, Some(10.9));
    assert_eq!(first.summary.cai, Some(5.5));
    assert_eq!(first.summary.oai, Some(5.4));
    assert!(first.events.iter().any(|e| e.kind == "snore"));
    assert!(first.events.iter().any(|e| e.kind == "leakLevel"));
    let third = &import.nights[2];
    let series = third.series.as_ref().unwrap();
    assert_eq!(third.sessions.len(), 2);
    assert_eq!(series.pressure.len(), 2401);
    assert_eq!(series.pressure.iter().filter(|p| p.is_some()).count(), 600);
    assert!(series.pressure[301..2101].iter().all(Option::is_none));
    assert_eq!(series.pressure.len(), series.excess_leak.len());
    assert_eq!(series.total_leak.len(), series.snore.len());
    let json = serde_json::to_value(first).unwrap();
    assert_eq!(json["date"], "2030-01-15");
    assert_eq!(json["usageSource"], "machine");
    assert!(json.get("list").is_none());
    assert!(json["settings"]["leakAlert"].is_boolean());
    let device_json = serde_json::to_string(&device).unwrap();
    assert!(!device_json.contains("TEST0000"));
    assert!(!first.list.settings_changed);
}
#[test]
fn absent_backup_uses_computed_and_null_machine_summary() {
    let mut card = memory_card();
    card.0.remove("TEST0000.bkp");
    let import = assemble_card(&card).unwrap();
    let first = &import.nights[0];
    assert_eq!(first.list.usage_source, Source::Computed);
    assert!((first.list.usage_minutes - 601.0 / 60.0).abs() < 1e-9);
    assert_eq!(first.list.pressure_mean, Some(6.4));
    assert!(first.list.ahi.is_none());
    assert!(first.summary.ai.is_none());
    assert!(first.events.is_empty());
}
#[test]
fn evening_rule_handles_noon_and_year_boundary() {
    assert_eq!(
        evening_date(&make_local_time(2030, 1, 1, 0, 0, 0).unwrap()),
        "2029-12-31"
    );
    assert_eq!(
        evening_date(&make_local_time(2030, 1, 1, 11, 59, 59).unwrap()),
        "2029-12-31"
    );
    assert_eq!(
        evening_date(&make_local_time(2030, 1, 1, 12, 0, 0).unwrap()),
        "2030-01-01"
    );
}
#[test]
fn events_survive_an_absent_machine_summary() {
    let mut card = memory_card();
    let bkp = card.0.get_mut("TEST0000.bkp").unwrap();
    for block in bkp.as_chunks_mut::<128>().0 {
        if block[..2] == [0xa5, 0xa5] {
            block.fill(0xff);
        }
    }
    let import = assemble_card(&card).unwrap();
    assert_eq!(import.nights[0].list.usage_source, Source::Computed);
    assert_eq!(import.nights[0].list.event_counts.central, 1);
}
#[test]
fn after_midnight_summary_and_events_belong_to_file_evening() {
    let mut card = memory_card();
    let bkp = card.0.get_mut("TEST0000.bkp").unwrap();
    for block in bkp.as_chunks_mut::<128>().0 {
        if block[..2] == [0xa5, 0xa5] {
            // summary timestamp at offset 4
            if block[8..10] == [1, 15] {
                block[9] = 16;
                block[10] = 1;
            }
        } else if block[4] >= 0xe0 && block[4] <= 0xef {
            for entry in block.as_chunks_mut::<8>().0 {
                if entry[2..4] == [1, 15] {
                    entry[3] = 16;
                    entry[5] = 1;
                }
            }
        }
    }
    assert_eq!(
        sleeplens_core::bkp::parse_bkp(card.0.get("TEST0000.bkp").unwrap()).sessions[0]
            .start
            .as_ref()
            .unwrap()
            .iso,
        "2030-01-16T01:30:00"
    );
    let import = assemble_card(&card).unwrap();
    assert_eq!(import.nights[0].list.usage_source, Source::Machine);
    assert_eq!(import.nights[0].list.event_counts.central, 1);
}
#[test]
fn corrupt_century_span_is_rejected_before_allocating_series() {
    let mut card = memory_card();
    let bytes = card.0.get_mut("TEST0000203001/20300115.dat").unwrap();
    let end = 0x800 + 599 * 812 + 4;
    bytes[end..end + 2].copy_from_slice(&2100u16.to_le_bytes());
    assert_eq!(
        assemble_card(&card).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}
#[test]
fn detection_keeps_stable_ties_and_windows_paths() {
    let d = detect_as2x(&[
        "prefixAB.bkp",
        "B203001\\20300115.DAT",
        "AB203001/20300116.dat",
    ]);
    assert_eq!(d.serial.as_deref(), Some("B"));
    assert_eq!(d.bkp.as_deref(), Some("prefixAB.bkp"));
    let d = detect_as2x(&[
        "TEST0000.bkp",
        "TEST0000203001/20300116.dat",
        "TEST0000203001/20300115.dat",
    ]);
    assert_eq!(d.nights[0].date, "20300115");
    assert_eq!(d.months, ["203001"]);
    assert!(!detect_as2x(&["DATALOG/x.edf", "STR.edf"]).detected);
}
#[test]
fn streaming_delivers_each_night_before_reading_next() {
    use std::cell::Cell;
    struct OrderedCard {
        inner: MemoryCard,
        reads: Cell<usize>,
        delivered: Cell<usize>,
    }
    impl CardSource for OrderedCard {
        fn list(&self) -> io::Result<Vec<String>> {
            self.inner.list()
        }
        fn read(&self, path: &str) -> io::Result<Vec<u8>> {
            if path.ends_with(".dat") {
                assert_eq!(self.reads.get(), self.delivered.get());
                self.reads.set(self.reads.get() + 1);
            }
            self.inner.read(path)
        }
    }
    let card = OrderedCard {
        inner: memory_card(),
        reads: Cell::new(0),
        delivered: Cell::new(0),
    };
    stream_card(&card, |_| card.delivered.set(card.delivered.get() + 1)).unwrap();
    assert_eq!(card.delivered.get(), 3);
}
#[cfg(feature = "fs")]
#[test]
fn fs_source_confines_reads_to_card_root() {
    use sleeplens_core::source::FsCardSource;
    let source = FsCardSource::new(fixture());
    assert!(source.read("../synthetic-as2x/TEST0000.bkp").is_err());
    assert!(source.read("/etc/passwd").is_err());
    assert_eq!(
        source
            .list()
            .unwrap()
            .iter()
            .filter(|p| p.ends_with(".dat"))
            .count(),
        3
    );
}
#[test]
fn settings_fall_back_to_backup_and_changes_compare_decoded_values() {
    let mut card = memory_card();
    card.0.get_mut("TEST0000203001/20300116.dat").unwrap()[0x140 + 0x17] = 5;
    let third = card.0.get_mut("TEST0000203001/20300117.dat").unwrap();
    third[0x140] = 0;
    third[0x141] = 0;
    let import = assemble_card(&card).unwrap();
    assert_eq!(
        import.nights[0].settings.as_ref().unwrap().humidifier_level,
        4
    );
    assert_eq!(
        import.nights[1].settings.as_ref().unwrap().humidifier_level,
        5
    );
    assert!(import.nights[1].list.settings_changed);
    assert!(import.nights[2].settings.is_some());
    assert!(import.nights[2].list.settings_changed);
}
#[test]
fn short_serial_is_fully_masked() {
    let mut card = memory_card();
    for (path, bytes) in &mut card.0 {
        if path.ends_with(".dat") {
            bytes[0x54..0x68].fill(0);
            bytes[0x54] = b'X';
        }
    }
    assert_eq!(
        assemble_card(&card)
            .unwrap()
            .metadata
            .device
            .unwrap()
            .serial_masked,
        "•••"
    );
}

#[test]
fn stages_follow_record_timestamps_and_leak_p95_uses_nearest_rank() {
    let import = assemble_card(&memory_card()).unwrap();
    for night in &import.nights {
        let stages = night.stages.as_ref().unwrap();
        let series = night.series.as_ref().unwrap();
        assert_eq!(stages.start, series.series_start);
        assert_eq!(stages.stages.len(), series.pressure.len().div_ceil(30));
        let mut leak: Vec<_> = series.excess_leak.iter().flatten().copied().collect();
        leak.sort_by(f32::total_cmp);
        let expected = leak[((leak.len() as f64 * 0.95).ceil() as usize) - 1];
        assert!((night.list.excess_leak_p95.unwrap() - expected as f64).abs() < 1e-5);
        assert_eq!(
            night.list.interruptions,
            night.sessions.len().saturating_sub(1)
        );
        assert_eq!(
            night.list.sleep_score,
            night.score.as_ref().map(|s| s.total)
        );
        assert!(!night.tips.is_empty());
    }
    let third = &import.nights[2];
    assert!(third.stages.as_ref().unwrap().stages[11..69]
        .iter()
        .all(|s| *s == sleeplens_core::SleepStage::NoData));
}

#[test]
fn only_mode_and_pressure_changes_are_prescription_changes() {
    let import = assemble_card(&memory_card()).unwrap();
    let settings = import.nights[0].settings.as_ref().unwrap();
    let mut changed = settings.clone();
    changed.humidifier_level = changed.humidifier_level.wrapping_add(1);
    changed.ramp_minutes = changed.ramp_minutes.wrapping_add(1);
    changed.comfort_level = changed.comfort_level.wrapping_add(1);
    assert!(!sleeplens_core::prescription_differs(settings, &changed));
    changed.min_pressure += 1.;
    assert!(sleeplens_core::prescription_differs(settings, &changed));
    let mut bkp = sleeplens_core::bkp::parse_bkp(&memory_card().0["TEST0000.bkp"]);
    let mut before = bkp.settings[0].clone();
    before.time = make_local_time(2030, 1, 15, 19, 0, 0);
    let mut during = before.clone();
    during.time = make_local_time(2030, 1, 15, 22, 0, 0);
    during.min_pressure += 1.;
    bkp.settings = vec![before, during];
    let start = make_local_time(2030, 1, 15, 21, 0, 0).unwrap().epoch_sec;
    assert!(sleeplens_core::prescription_changed_within(
        &bkp,
        Some(start),
        Some(start + 7200)
    ));
    assert!(!sleeplens_core::prescription_changed_within(
        &bkp,
        Some(start),
        Some(start + 60)
    ));
}
