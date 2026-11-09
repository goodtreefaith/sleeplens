use serde_json::Value;
use std::{path::Path, process::Command};

fn inspect(args: &[&str]) -> String {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/synthetic-as2x");
    let output = Command::new(env!("CARGO_BIN_EXE_sleeplens"))
        .arg("inspect")
        .arg(fixture)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("TEST0000"));
    text
}

#[test]
fn fixture_json_keeps_legacy_fields_and_adds_v2_details() {
    let output: Value = serde_json::from_str(&inspect(&["--json"])).unwrap();
    let nights = output["nights"].as_array().unwrap();
    assert_eq!(nights.len(), 3);
    assert_eq!(nights[0]["usageMinutes"], 11.0);
    assert_eq!(nights[0]["usageSource"], "summary");
    assert_eq!(nights[0]["pressureP90"], 8.0);
    for night in nights {
        assert!(night.get("score").is_some());
        assert!(night.get("sleepScore").is_some());
        assert!(night["excessLeakP95"].is_number());
        assert!(night["interruptions"].is_number());
        assert_eq!(night["stages"]["epochSeconds"], 30);
        assert!(!night["stages"]["stages"].as_array().unwrap().is_empty());
        assert!(!night["tips"].as_array().unwrap().is_empty());
    }
}

#[test]
fn fixture_text_includes_estimates_and_night_filter_keeps_events() {
    let text = inspect(&["--night", "20300115"]);
    assert!(text.contains("estimated sleep score:"));
    assert!(text.contains("estimated stages:"));
    assert!(text.contains("11 min"));
    let output: Value = serde_json::from_str(&inspect(&["--json", "--night", "20300115"])).unwrap();
    assert_eq!(output["nights"].as_array().unwrap().len(), 1);
    assert!(!output["events"].as_array().unwrap().is_empty());
}
