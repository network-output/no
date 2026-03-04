mod helpers;

use helpers::cli::{exit_code, no_cmd, parse_first_json};

#[test]
fn dns_invalid_record_type() {
  let output = no_cmd().args(["dns", "example.com", "INVALID"]).output().unwrap();
  assert_eq!(
    output.status.code().unwrap(),
    exit_code::INVALID_INPUT,
    "expected exit code {} for invalid record type",
    exit_code::INVALID_INPUT
  );
  let json = parse_first_json(&output);
  assert_eq!(json["type"], "error");
  assert_eq!(json["protocol"], "dns");
  assert_eq!(json["data"]["code"], "INVALID_INPUT");
}

#[test]
#[ignore]
fn dns_a_record() {
  let output = no_cmd().args(["dns", "example.com"]).output().unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["type"], "response");
  assert_eq!(json["protocol"], "dns");
  assert_eq!(json["data"]["type"], "A");
  assert!(json["data"]["records"].as_array().is_some_and(|r| !r.is_empty()));
}

#[test]
#[ignore]
fn dns_aaaa_record() {
  let output = no_cmd().args(["dns", "example.com", "AAAA"]).output().unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["type"], "response");
  assert_eq!(json["protocol"], "dns");
  assert_eq!(json["data"]["type"], "AAAA");
  assert!(json["data"]["records"].as_array().is_some());
}

#[test]
#[ignore]
fn dns_mx_record() {
  let output = no_cmd().args(["dns", "google.com", "MX"]).output().unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["type"], "response");
  assert_eq!(json["data"]["type"], "MX");
  let records = json["data"]["records"].as_array().unwrap();
  assert!(!records.is_empty());
  assert!(
    records[0]["priority"].is_number(),
    "MX records should have priority field"
  );
}

#[test]
#[ignore]
fn dns_cname_record() {
  let output = no_cmd().args(["dns", "www.github.com", "CNAME"]).output().unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["type"], "response");
  assert_eq!(json["data"]["type"], "CNAME");
}

#[test]
#[ignore]
fn dns_reverse_lookup() {
  let output = no_cmd().args(["dns", "8.8.8.8"]).output().unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["type"], "response");
  assert_eq!(json["protocol"], "dns");
  assert_eq!(json["data"]["type"], "PTR");
  assert!(json["data"]["records"].as_array().is_some_and(|r| !r.is_empty()));
}

#[test]
#[ignore]
fn dns_custom_server() {
  let output = no_cmd()
    .args(["dns", "example.com", "--server", "8.8.8.8"])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["type"], "response");
  assert_eq!(json["protocol"], "dns");
  assert!(json["data"]["records"].as_array().is_some_and(|r| !r.is_empty()));
}
