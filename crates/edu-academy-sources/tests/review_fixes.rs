//! Frozen actual-original comparisons. No expected values derive from this Rust code.
use edu_academy_sources::{data::*, http, presentation};
#[test]
fn unseen_original_presentation_ast_values_spans_and_refusals() {
    let input = parse(include_str!(
        "../../../migration/source-adapter-review-fixes-v2/STIMULI.json"
    ))
    .unwrap();
    let expected = parse(include_str!(
        "../../../migration/source-adapter-review-fixes-v2/ORIGINAL-EXPECTED.json"
    ))
    .unwrap();
    for (case, original) in input
        .get("presentation")
        .array()
        .iter()
        .zip(expected.get("presentation").array())
    {
        assert_eq!(case.get("id"), original.get("id"));
        let actual = observe(presentation::inspect(
            &string(case.get("html")),
            &string(case.get("helpers")),
        ));
        assert_eq!(
            actual,
            *original.get("result"),
            "case {}",
            string(case.get("id"))
        );
    }
}
#[test]
fn shared_total_selector_fence_preserves_placement_and_threshold() {
    let pair = ("reqCmd".into(), "SelSglRecord".into());
    for count in 0..=4 {
        let extra = vec![pair.clone(); count];
        assert_eq!(
            http::admit_operation(
                http::ORIGIN,
                http::SELECTED,
                "GET",
                std::slice::from_ref(&pair),
                &extra
            )
            .is_some(),
            count <= 1
        );
        assert!(
            http::admit_operation(http::ORIGIN, http::SELECTED, "GET", &extra, &[]).is_some()
                == (count == 1)
        );
    }
    for selector in [
        "udtPrg",
        "udtHw",
        "udtMemo",
        "udtAttn",
        "SetDailyTest",
        "SetHomeWorkRate",
    ] {
        assert!(
            http::admit_operation(
                http::ORIGIN,
                http::MAIN,
                "GET",
                &[("reqCmd".into(), selector.into())],
                &[]
            )
            .is_none()
        );
    }
    let post = ("p_process".into(), "Main".into());
    for count in 0..=4 {
        assert_eq!(
            http::admit_operation(
                http::ORIGIN,
                http::MAIN,
                "POST",
                &vec![post.clone(); count],
                std::slice::from_ref(&post)
            )
            .is_some(),
            count <= 1
        );
    }
}

struct RedirectFake {
    index: usize,
    location: String,
    second_redirect: bool,
    calls: Vec<http::Request>,
}
impl http::Transport for RedirectFake {
    fn dispatch(&mut self, request: &http::Request) -> Result<http::Response> {
        self.calls.push(request.clone());
        self.index += 1;
        let (body, status, headers) = match self.index {
            1 => (
                include_str!("../../../migration/source-adapter-reference-v1/corpus/login.html")
                    .to_string(),
                200,
                vec![("Set-Cookie".into(), "JSESSIONID=redirect; Path=/".into())],
            ),
            2 => (
                String::new(),
                302,
                vec![
                    ("Location".into(), self.location.clone()),
                    ("Set-Cookie".into(), "JSESSIONID=follow; Path=/".into()),
                ],
            ),
            3 if request.path != "/servlet/controller.coursemanage.CourseManageServlet" => (
                String::new(),
                if self.second_redirect { 302 } else { 200 },
                if self.second_redirect {
                    vec![("Location".into(), "/again".into())]
                } else {
                    vec![]
                },
            ),
            _ => (
                include_str!(
                    "../../../migration/source-adapter-reference-v1/corpus/principal.html"
                )
                .to_string(),
                200,
                vec![],
            ),
        };
        Ok(http::Response {
            bytes: body.into_bytes(),
            status,
            headers,
            url: format!("{}{}", http::ORIGIN, request.path),
            stalled: false,
        })
    }
}
#[test]
fn original_relative_redirect_queries_cookies_and_error_order() {
    let expected = parse(include_str!(
        "../../../migration/source-adapter-review-fixes-v2/ORIGINAL-EXPECTED.json"
    ))
    .unwrap();
    let principal = parse(
        r#"{"teacherLabel":"S09 Synthetic Teacher","teacherKey":"99001","collegeKey":"88001"}"#,
    )
    .unwrap();
    let mut evidence = vec![];
    for case in expected.get("redirects").array() {
        let mut client = http::Client::new(
            principal.clone(),
            RedirectFake {
                index: 0,
                location: string(case.get("location")),
                second_redirect: case.get("id") == &Js::Num(11.),
                calls: vec![],
            },
        )
        .unwrap();
        let result = observe(client.authenticate("s09-fictional-user", "s09-fictional-password"));
        assert_eq!(
            &result,
            case.get("result"),
            "location {:?}",
            case.get("location")
        );
        let calls = client
            .transport
            .calls
            .iter()
            .map(|r| {
                let selector = r
                    .query
                    .iter()
                    .find(|(k, _)| k == "reqCmd")
                    .or_else(|| r.body.iter().find(|(k, _)| k == "p_process"))
                    .map_or(Js::Null, |(_, v)| v.clone().into());
                obj(&[
                    ("path", r.path.clone().into()),
                    (
                        "query",
                        Js::Arr(
                            r.query
                                .iter()
                                .map(|(k, v)| Js::Arr(vec![k.clone().into(), v.clone().into()]))
                                .collect(),
                        ),
                    ),
                    ("method", r.method.clone().into()),
                    ("selector", selector),
                    (
                        "cookie",
                        r.headers
                            .iter()
                            .find(|(k, _)| k == "Cookie")
                            .map_or(Js::Null, |(_, v)| v.clone().into()),
                    ),
                ])
            })
            .collect::<Vec<_>>();
        assert_eq!(
            Js::Arr(calls.clone()),
            *case.get("calls"),
            "location {:?}",
            case.get("location")
        );
        client.close();
        assert!(!client.authenticated);
        evidence.push(obj(&[
            ("location", case.get("location").clone()),
            ("result", result),
            ("calls", Js::Arr(calls)),
            ("authenticatedAfterClose", client.authenticated.into()),
        ]));
    }
    if let Ok(path) = std::env::var("S09_REVIEW_REDIRECT_EVIDENCE") {
        std::fs::write(path, format!("{}\n", json(&Js::Arr(evidence)))).unwrap();
    }
}
#[test]
fn unseen_original_regexp_bodies_and_es2022_version_restrictions() {
    let input = parse(include_str!(
        "../../../migration/source-adapter-review-fixes-v2/REGEXP-STIMULI.json"
    ))
    .unwrap();
    let expected = parse(include_str!(
        "../../../migration/source-adapter-review-fixes-v2/REGEXP-EXPECTED.json"
    ))
    .unwrap();
    for (case, original) in input
        .get("presentation")
        .array()
        .iter()
        .zip(expected.get("presentation").array())
    {
        let actual = observe(presentation::inspect(
            &string(case.get("html")),
            &string(case.get("helpers")),
        ));
        assert_eq!(
            &actual,
            original.get("result"),
            "{}",
            string(case.get("id"))
        );
    }
}
#[test]
fn original_ast_limits_computed_properties_and_refusal_precedence() {
    let input = parse(include_str!(
        "../../../migration/source-adapter-review-fixes-v2/AST-STIMULI.json"
    ))
    .unwrap();
    let expected = parse(include_str!(
        "../../../migration/source-adapter-review-fixes-v2/AST-EXPECTED.json"
    ))
    .unwrap();
    for (case, original) in input
        .get("presentation")
        .array()
        .iter()
        .zip(expected.get("presentation").array())
    {
        let actual = observe(presentation::inspect(
            &string(case.get("html")),
            &string(case.get("helpers")),
        ));
        assert_eq!(
            &actual,
            original.get("result"),
            "{}",
            string(case.get("id"))
        );
    }
}

#[test]
fn original_es2022_unicode_property_vocabulary() {
    let input = parse(include_str!(
        "../../../migration/source-adapter-review-fixes-v2/UNICODE-STIMULI.json"
    ))
    .unwrap();
    let expected = parse(include_str!(
        "../../../migration/source-adapter-review-fixes-v2/UNICODE-EXPECTED.json"
    ))
    .unwrap();
    for (case, original) in input
        .get("presentation")
        .array()
        .iter()
        .zip(expected.get("presentation").array())
    {
        assert_eq!(
            observe(presentation::inspect(
                &string(case.get("html")),
                &string(case.get("helpers"))
            )),
            *original.get("result"),
            "{}",
            string(case.get("id"))
        );
    }
}

#[test]
fn original_ast_node_budget_and_class_pattern_blocks() {
    let input = parse(include_str!(
        "../../../migration/source-adapter-review-fixes-v2/BUDGET-STIMULI.json"
    ))
    .unwrap();
    let expected = parse(include_str!(
        "../../../migration/source-adapter-review-fixes-v2/BUDGET-EXPECTED.json"
    ))
    .unwrap();
    for (case, original) in input
        .get("presentation")
        .array()
        .iter()
        .zip(expected.get("presentation").array())
    {
        assert_eq!(
            observe(presentation::inspect(
                &string(case.get("html")),
                &string(case.get("helpers"))
            )),
            *original.get("result"),
            "{}",
            string(case.get("id"))
        );
    }
}

#[test]
fn original_assignment_spread_pattern_node_budgets() {
    let input = parse(include_str!(
        "../../../migration/source-adapter-review-fixes-v2/CONTAINERS-STIMULI.json"
    ))
    .unwrap();
    let expected = parse(include_str!(
        "../../../migration/source-adapter-review-fixes-v2/CONTAINERS-EXPECTED.json"
    ))
    .unwrap();
    for (case, original) in input
        .get("presentation")
        .array()
        .iter()
        .zip(expected.get("presentation").array())
    {
        assert_eq!(
            observe(presentation::inspect(
                &string(case.get("html")),
                &string(case.get("helpers"))
            )),
            *original.get("result"),
            "{}",
            string(case.get("id"))
        );
    }
}
