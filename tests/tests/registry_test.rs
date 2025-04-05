use async_trait::async_trait;
use regex::Regex;
use rstest::rstest;
use stream_download::registry::{Input, Registry, RegistryEntry, Rule, Source};

#[rstest]
#[case("", vec![], false)]
#[case("not-a-url", vec![Rule::any_string()], true)]
#[case("/some/path", vec![Rule::any_string()], true)]
#[case("file://test", vec![Rule::any_string()], false)]
#[case("test", vec![Rule::string("test")], true)]
#[case("test2", vec![Rule::string("test")], false)]
#[case("not-a-url", vec![Rule::any_string(), Rule::any_url()], true)]
#[case("http://test", vec![Rule::any_string(), Rule::any_url()], true)]
#[case("test2", vec![Rule::string(Regex::new(r"test").unwrap())], true)]
#[case("http://test", vec![Rule::url_scheme("http")], true)]
#[case("http://test", vec![Rule::url("http", "test")], true)]
#[case("http://test", vec![Rule::url("https", "test")],false)]
#[case("https://test", vec![Rule::url_scheme("http")], false)]
#[case("https://test", vec![Rule::any_http()], true)]
#[case("http://test", vec![Rule::any_http()], true)]
#[case("http://test", vec![Rule::url_scheme(Regex::new(r"^https?$").unwrap())], true)]
#[case("https://test", vec![Rule::url_scheme(Regex::new(r"^https?$").unwrap())], true)]
#[case("http://test.com", vec![Rule::url_domain("test.com")], true)]
#[case("http://test.com", vec![Rule::http_domain("test.com")], true)]
#[case("http://test.com", vec![Rule::url_domain(Regex::new(r"^(www\.)?test\.com$").unwrap())], true)]
#[case("http://www.test.com", vec![Rule::url_domain(Regex::new(r"^(www\.)?test\.com$").unwrap())], true)]
#[case("http://www2.test.com", vec![Rule::url_domain(Regex::new(r"^(www\.)?test\.com$").unwrap())], false)]
#[case("custom://test", vec![Rule::prefix("custom://")], true)]
#[case("custom://http://test", vec![Rule::prefix("custom://")], true)]
#[case("http://custom://test", vec![Rule::prefix("custom://")], false)]
#[tokio::test]
async fn single_entry(#[case] input: &str, #[case] rules: Vec<Rule>, #[case] found: bool) {
    struct Matcher {
        rules: Vec<Rule>,
    }

    #[async_trait]
    impl RegistryEntry<()> for Matcher {
        fn priority(&self) -> u32 {
            1
        }

        fn rules(&self) -> &[Rule] {
            &self.rules
        }

        async fn handler(&mut self, _input: Input) {}
    }
    let mut registry = Registry::new().entry(Matcher { rules });
    assert_eq!(found, registry.find_match(input).await.is_some());
}

#[rstest]
#[case("", vec![], vec![], None)]
#[case("str", vec![Rule::any_string()], vec![], Some(1))]
#[case("str", vec![], vec![Rule::any_string()], Some(2))]
#[case("str", vec![Rule::any_string()], vec![Rule::any_string()], Some(1))]
#[case("test", vec![Rule::string("test")], vec![Rule::any_string()], Some(1))]
#[case("test", vec![Rule::any_string()], vec![Rule::string("test")], Some(1))]
#[case("http://test.com", vec![Rule::url_domain("test.com")], vec![Rule::any_url()], Some(1))]
#[case("test.com", vec![Rule::url_domain("test.com")], vec![Rule::any_url()], None)]
#[case("custom://http://test.com", vec![Rule::url_domain("test.com")], vec![Rule::prefix("custom://")],Some(2))]
#[tokio::test]
async fn priority(
    #[case] input: &str,
    #[case] matcher1_rules: Vec<Rule>,
    #[case] matcher2_rules: Vec<Rule>,
    #[case] expected: Option<u32>,
) {
    struct Matcher1 {
        rules: Vec<Rule>,
    }

    #[async_trait]
    impl RegistryEntry<u32> for Matcher1 {
        fn priority(&self) -> u32 {
            1
        }

        fn rules(&self) -> &[Rule] {
            &self.rules
        }

        async fn handler(&mut self, _input: Input) -> u32 {
            1
        }
    }

    struct Matcher2 {
        rules: Vec<Rule>,
    }

    #[async_trait]
    impl RegistryEntry<u32> for Matcher2 {
        fn priority(&self) -> u32 {
            2
        }

        fn rules(&self) -> &[Rule] {
            &self.rules
        }

        async fn handler(&mut self, _input: Input) -> u32 {
            2
        }
    }

    // verify insertion order doesn't matter
    let mut registry = Registry::new()
        .entry(Matcher2 {
            rules: matcher2_rules.clone(),
        })
        .entry(Matcher1 {
            rules: matcher1_rules.clone(),
        });
    assert_eq!(expected, registry.find_match(input).await);

    let mut registry = Registry::new()
        .entry(Matcher1 {
            rules: matcher1_rules,
        })
        .entry(Matcher2 {
            rules: matcher2_rules,
        });
    assert_eq!(expected, registry.find_match(input).await);
}

#[rstest]
#[case("test", vec![Rule::any_string()], Input { prefix: None, source: Source::String("test".to_string()) })]
#[case("http://test/", vec![Rule::any_url()], Input { prefix: None, source: Source::Url("http://test".parse().unwrap()) })]
#[case("custom://test", vec![Rule::prefix("custom://")], Input { prefix: Some("custom://".to_string()), source: Source::String("test".to_string()) })]
#[case("custom://http://test/", vec![Rule::prefix("custom://")], Input { prefix: Some("custom://".to_string()), source: Source::Url("http://test".parse().unwrap()) })]
#[tokio::test]
async fn parse_input(#[case] s: &str, #[case] rules: Vec<Rule>, #[case] expected: Input) {
    struct Matcher {
        rules: Vec<Rule>,
    }

    #[async_trait]
    impl RegistryEntry<Input> for Matcher {
        fn priority(&self) -> u32 {
            1
        }

        fn rules(&self) -> &[Rule] {
            &self.rules
        }

        async fn handler(&mut self, input: Input) -> Input {
            input
        }
    }
    let mut registry = Registry::new().entry(Matcher { rules });
    let output = registry.find_match(s).await;
    assert_eq!(Some(&expected), output.as_ref());
    let output = output.unwrap();
    assert_eq!(s, output.clone().into_raw());
    // Running the rules on the parsed input should yield the same results.
    assert_eq!(Some(expected), registry.find_match(output).await);
}
