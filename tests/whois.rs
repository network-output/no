mod helpers;

use helpers::cli::{no_cmd, parse_first_json};

#[test]
fn whois_missing_query() {
  let output = no_cmd().args(["whois"]).output().unwrap();
  assert!(!output.status.success(), "expected failure when query is missing");
}

#[test]
#[ignore]
fn whois_domain() {
  let output = no_cmd().args(["whois", "example.com"]).output().unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["type"], "response");
  assert_eq!(json["protocol"], "whois");
  assert_eq!(json["data"]["query"], "example.com");
  assert!(
    json["data"]["response"].as_str().is_some_and(|s| !s.is_empty()),
    "expected non-empty WHOIS response"
  );
}

#[test]
#[ignore]
fn whois_with_server() {
  let output = no_cmd()
    .args(["whois", "example.com", "--server", "whois.verisign-grs.com"])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["type"], "response");
  assert_eq!(json["protocol"], "whois");
  assert_eq!(json["data"]["server"], "whois.verisign-grs.com");
}

#[test]
#[ignore]
fn whois_ip() {
  let output = no_cmd().args(["whois", "8.8.8.8"]).output().unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["type"], "response");
  assert_eq!(json["protocol"], "whois");
  assert_eq!(json["data"]["server"], "whois.arin.net");
}
