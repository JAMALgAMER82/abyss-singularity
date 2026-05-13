use super::proxy_protocol::parse_line;

#[test]
fn parses_a_tcp4_proxy_v1_line() {
    let h = parse_line("PROXY TCP4 100.64.0.5 100.64.0.1 12345 47992").unwrap();
    assert_eq!(h.src_ip, "100.64.0.5");
    assert_eq!(h.src_port, 12345);
}

#[test]
fn parses_a_tcp6_proxy_v1_line() {
    let h = parse_line("PROXY TCP6 fd7a:115c:a1e0::5 fd7a:115c:a1e0::1 5000 47992").unwrap();
    assert_eq!(h.src_ip, "fd7a:115c:a1e0::5");
    assert_eq!(h.src_port, 5000);
}

#[test]
fn accepts_unknown_family() {
    let h = parse_line("PROXY UNKNOWN").unwrap();
    assert_eq!(h.src_ip, "0.0.0.0");
    assert_eq!(h.src_port, 0);
}

#[test]
fn rejects_missing_prefix() {
    let err = parse_line("WRONG TCP4 1.2.3.4 5.6.7.8 1 2").unwrap_err();
    assert!(err.to_string().contains("PROXY"));
}

#[test]
fn rejects_malformed_port() {
    let err = parse_line("PROXY TCP4 1.2.3.4 5.6.7.8 not_a_port 47992").unwrap_err();
    assert!(err.to_string().contains("src port"));
}

#[test]
fn rejects_unknown_family() {
    let err = parse_line("PROXY MARS 1.2.3.4 5.6.7.8 1 2").unwrap_err();
    assert!(err.to_string().contains("unknown PROXY family"));
}
