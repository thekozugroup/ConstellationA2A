use constellation_discovery::tailscale::parse_status_json;

const FIXTURE: &str = r#"{
  "Self": { "TailscaleIPs": ["100.76.147.110"], "Online": true, "HostName": "atmos-vnic" },
  "Peer": {
    "abc": { "TailscaleIPs": ["100.76.147.42"], "Online": true, "HostName": "kraken" },
    "def": { "TailscaleIPs": ["100.76.147.43"], "Online": false, "HostName": "offline" }
  }
}"#;

#[test]
fn parses_online_peers_only() {
    let peers = parse_status_json(FIXTURE).expect("parse");
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].host, "kraken");
    assert_eq!(peers[0].ip.to_string(), "100.76.147.42");
}
