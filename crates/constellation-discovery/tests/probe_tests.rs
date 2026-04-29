use constellation_a2a::{AgentCapabilities, AgentCard, Skill};
use constellation_discovery::probe::probe_card;
use url::Url;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

fn sample_card(url: &str) -> AgentCard {
    AgentCard {
        name: "probed".into(),
        description: None,
        url: Url::parse(url).unwrap(),
        version: "0.1.0".into(),
        capabilities: AgentCapabilities::default(),
        default_input_modes: vec!["text".into()],
        default_output_modes: vec!["text".into()],
        skills: vec![Skill {
            id: "x".into(),
            name: "x".into(),
            description: None,
            tags: vec![],
        }],
    }
}

#[tokio::test]
async fn probe_returns_card_on_200() {
    let server = MockServer::start().await;
    let card = sample_card(&server.uri());
    Mock::given(method("GET"))
        .and(path("/.well-known/agent.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&card))
        .mount(&server)
        .await;
    let got = probe_card(&server.uri()).await.expect("probe ok");
    assert_eq!(got.name, "probed");
}

#[tokio::test]
async fn probe_returns_error_on_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/agent.json"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let res = probe_card(&server.uri()).await;
    assert!(res.is_err());
}
