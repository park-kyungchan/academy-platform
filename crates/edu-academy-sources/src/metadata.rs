//! Owner-admitted metadata algorithms over raw finite source ports. No transport.
//! Caller schemas are strict; source shape failures never become caller failures.
use crate::{
    data::*,
    history::{History, Port, date, instant, require},
};
use edu_academy_core::js::test;

pub const COVERAGE: &str = "one_observed_student_search_page_not_complete_history";
fn strict(v: &Js, keys: &[&str]) -> Result<()> {
    require(
        object(v)
            && v.props()
                .iter()
                .all(|(k, _)| keys.iter().any(|s| k == &text(s))),
        "tool_input_invalid",
    )
}
fn schema(name: &str, v: &Js) -> Result<Js> {
    edu_academy_core::schema::validate(name, v).map_err(|_| Error::error("tool_input_invalid"))
}
fn period(v: &Js) -> Result<Js> {
    strict(v, &["from", "through"])?;
    require(
        date(v.get("from"))
            && date(v.get("through"))
            && v.get("from").units() <= v.get("through").units(),
        "tool_input_invalid",
    )?;
    Ok(obj(&[
        ("from", v.get("from").clone()),
        ("through", v.get("through").clone()),
    ]))
}
pub fn query(v: &Js) -> Result<Js> {
    let kind = v.get("kind");
    if kind.is("student_papers") || kind.is("printed_paper_metadata") {
        let printed = kind.is("printed_paper_metadata");
        strict(
            v,
            if printed {
                &["kind", "paperCode", "period"]
            } else {
                &["kind", "period"]
            },
        )?;
        let mut q = obj(&[("kind", kind.clone())]);
        if printed {
            require(
                v.get("paperCode").string()
                    && test(
                        "^#[3456789ABCDEFGHJKLMNPQRSTUVWXYZ][23456789ABCDEFGHJKLMNPQRSTUVWXYZ]{0,10}$",
                        v.get("paperCode"),
                    ),
                "tool_input_invalid",
            )?;
            q.set("paperCode", v.get("paperCode").clone());
        }
        q.set("period", period(v.get("period"))?);
        return Ok(q);
    }
    if kind.is("daily_metadata") || kind.is("daily_inventory") {
        strict(v, &["kind"])?;
        return Ok(obj(&[("kind", kind.clone())]));
    }
    strict(v, &["kind", "periods"])?;
    let selectors = v.get("periods");
    require(
        kind.is("daily_history")
            && array(selectors)
            && (1..=6).contains(&selectors.array().len())
            && selectors.array().iter().enumerate().all(|(i, s)| {
                ["11", "12", "13", "14", "15", "16"].iter().any(|x| s.is(x))
                    && !selectors.array()[..i].contains(s)
            }),
        "tool_input_invalid",
    )?;
    Ok(obj(&[
        ("kind", kind.clone()),
        ("periods", selectors.clone()),
    ]))
}
/// Shared by the endpoint and direct source entry; no caller authority fields.
pub fn prepare(input: &Js, by_name: bool) -> Result<Js> {
    strict(
        input,
        if by_name {
            &[
                "displayName",
                "lessonDate",
                "purpose",
                "currentRequestRef",
                "queries",
            ]
        } else {
            &["purpose", "currentRequestRef", "scope", "queries"]
        },
    )?;
    let mut out = obj(&[]);
    if by_name {
        require(input.get("displayName").string(), "tool_input_invalid")?;
        let name = trim(input.get("displayName").units());
        require(!name.is_empty() && name.len() <= 200, "tool_input_invalid")?;
        out.set("displayName", Js::Str(name.to_vec()));
        out.set(
            "lessonDate",
            schema("dailyRecordDateSchema", input.get("lessonDate"))?,
        );
    }
    for k in ["purpose", "currentRequestRef"] {
        out.set(k, schema("dailyRecordIdSchema", input.get(k))?);
    }
    if !by_name {
        let scope = schema("dailyRecordScopeSchema", input.get("scope"))?;
        require(
            scope.get("sourceNamespace").is("academy.lms.metadata"),
            "tool_input_invalid",
        )?;
        out.set("scope", scope);
    }
    require(
        array(input.get("queries")) && (1..=8).contains(&input.get("queries").array().len()),
        "tool_input_invalid",
    )?;
    out.set(
        "queries",
        Js::Arr(
            input
                .get("queries")
                .array()
                .iter()
                .map(query)
                .collect::<Result<Vec<_>>>()?,
        ),
    );
    Ok(out)
}
fn admitted_date(v: &Js, policy: &Js) -> bool {
    date(v)
        && (policy.get("allowedFrom") == &Js::Null
            || v.units() >= policy.get("allowedFrom").units())
        && (policy.get("allowedThrough") == &Js::Null
            || v.units() <= policy.get("allowedThrough").units())
}
impl<P: Port> History<P> {
    /// Owner-only configured date for legacy selected-student admissions whose
    /// routing admission spans multiple dates. Never supplied by tool arguments.
    pub fn with_selected_metadata_date(mut self, configured_date: Js) -> Result<Self> {
        require(date(&configured_date), "native_metadata_scope_refused")?;
        self.selected_metadata_date = configured_date;
        Ok(self)
    }
    fn authorize_batch(&self, input: &Js, scope: &Js) -> Result<()> {
        let p = &self.metadata_admission;
        require(
            p != &Js::Null
                && scope.get("teacherId") == p.get("teacherId")
                && admitted_date(scope.get("lessonDate"), p)
                && input.get("queries").array().iter().all(|q| {
                    let kind = q.get("kind");
                    p.get("operations").array().contains(kind)
                        && (!q.has("period")
                            || admitted_date(q.get("period").get("from"), p)
                                && admitted_date(q.get("period").get("through"), p))
                        && (kind.is("student_papers")
                            || p.get("relativeSelectors").array().contains(&"1".into()))
                        && (!kind.is("daily_history")
                            || q.get("periods")
                                .array()
                                .iter()
                                .all(|s| p.get("relativeSelectors").array().contains(s)))
                }),
            "metadata_namespace_not_admitted",
        )
    }
    /// No parallel source borrowing: &mut self is the same serial source loan as
    /// I1. A failed source invocation shares the accepted source-failure latch.
    pub fn student_metadata(&mut self, value: &Js, by_name: bool) -> Result<Js> {
        let input = prepare(value, by_name)?;
        let mut scope;
        let configured;
        if by_name {
            scope = obj(&[
                ("sourceNamespace", "academy.lms.metadata".into()),
                ("teacherId", self.routing_admission.get("teacherId").clone()),
                ("studentId", "1".into()),
                ("lessonDate", input.get("lessonDate").clone()),
            ]);
            self.authorize_batch(&input, &scope)?;
            configured = self
                .students
                .iter()
                .find(|s| s.get("displayName") == input.get("displayName"))
                .cloned();
            require(
                self.managed_names.contains(input.get("displayName"))
                    && (configured.is_some()
                        || self
                            .metadata_admission
                            .get("studentScope")
                            .is("source_verified")),
                "metadata_namespace_not_admitted",
            )?;
        } else {
            scope = input.get("scope").clone();
            configured = self
                .students
                .iter()
                .find(|s| s.get("studentId") == scope.get("studentId"))
                .cloned();
            require(
                scope.get("teacherId") == self.routing_admission.get("teacherId")
                    && configured.is_some(),
                "native_metadata_scope_refused",
            )?;
            if self.metadata_admission == Js::Null {
                require(
                    scope.get("lessonDate") == &self.selected_metadata_date
                        && date(&self.selected_metadata_date),
                    "native_metadata_scope_refused",
                )?;
            } else {
                self.authorize_batch(&input, &scope)?;
            }
        }
        for q in input.get("queries").array() {
            require(
                self.port.metadata_method_available(&string(q.get("kind"))),
                "native_metadata_method_unavailable",
            )?;
        }
        require(!self.latched, "source_failure_latched_no_retry")?;
        let result = (|| {
            let student = if by_name {
                let student = self.dated_student(&input)?;
                scope.set("studentId", student.get("studentKey").clone());
                student
            } else {
                let c = configured.as_ref().expect("checked configured student");
                obj(&[
                    ("studentKey", c.get("studentId").clone()),
                    ("studentName", c.get("displayName").clone()),
                ])
            };
            let mut results = vec![];
            for q in input.get("queries").array() {
                let observation = self.read_metadata_query(&student, q)?;
                results.push(obj(&[("query", q.clone()), ("observation", observation)]));
            }
            Ok(obj(&[("scope", scope), ("results", Js::Arr(results))]))
        })();
        if result.is_err() {
            self.latched = true;
        }
        result.map_err(source_error)
    }
    fn read_metadata_query(&mut self, student: &Js, q: &Js) -> Result<Js> {
        let kind = q.get("kind");
        if kind.is("daily_history") {
            return self.read_history(student, q, true);
        }
        if kind.is("student_papers") {
            return self.read_student_papers(student, q.get("period"));
        }
        if kind.is("daily_metadata") || kind.is("daily_inventory") {
            return self.read_daily_metadata(student, kind.is("daily_inventory"));
        }
        let paper = crate::history::decode_code(q.get("paperCode"))?;
        let initial = self.read_student_papers(student, q.get("period"))?;
        let mut matches: Vec<Js> = initial
            .get("rows")
            .array()
            .iter()
            .filter(|r| r.get("paperKey") == &paper)
            .cloned()
            .collect();
        let mut searched = vec![searched_source(&initial, "student_assessment", true)];
        let mut family = if matches.is_empty() {
            Js::Null
        } else {
            "student_assessment".into()
        };
        let mut source_period = initial.get("sourcePeriod").clone();
        if matches.is_empty() {
            let daily = self.read_daily_metadata(student, false)?;
            searched.push(searched_source(&daily, "daily_zero", false));
            matches = daily
                .get("rows")
                .array()
                .iter()
                .filter(|r| r.get("paperKey") == &paper)
                .cloned()
                .collect();
            if !matches.is_empty() {
                family = "daily_zero".into();
                source_period = Js::Null;
            }
        }
        Ok(obj(&[
            ("paperCode", q.get("paperCode").clone()),
            ("paperKey", paper),
            (
                "status",
                if matches.is_empty() {
                    "not_found_in_scope"
                } else {
                    "observed"
                }
                .into(),
            ),
            ("rows", Js::Arr(matches)),
            ("sourcePeriod", source_period),
            ("page", Js::Num(1.0)),
            (
                "observedAt",
                searched.last().unwrap().get("observedAt").clone(),
            ),
            (
                "coverage",
                "observed_source_pages_not_complete_history".into(),
            ),
            ("matchedSourceFamily", family),
            ("generatedMembership", "unqualified".into()),
            ("searchedSources", Js::Arr(searched)),
        ]))
    }
    fn read_student_papers(&mut self, student: &Js, period: &Js) -> Result<Js> {
        self.requests += 1;
        let raw = self.port.student_paper_page(student, period)?;
        require(
            raw.get("operation").is("UserBySearchTestResult") && raw.get("page") == &Js::Num(1.0),
            "native_metadata_operation_conflict",
        )?;
        let source_period = raw.get("sourcePeriod");
        require(
            object(source_period)
                && date(source_period.get("from"))
                && date(source_period.get("to")),
            "source_response_malformed",
        )?;
        require(
            source_period.get("from") == period.get("from")
                && source_period.get("to") == period.get("through"),
            "native_metadata_period_conflict",
        )?;
        let rows = crate::history::project_rows(raw.get("rawRows"), student, true)?;
        Ok(obj(&[
            ("rows", Js::Arr(rows)),
            ("operation", "UserBySearchTestResult".into()),
            ("page", Js::Num(1.0)),
            (
                "sourcePeriod",
                obj(&[
                    ("from", source_period.get("from").clone()),
                    ("to", source_period.get("to").clone()),
                ]),
            ),
            ("observedAt", source_instant(raw.get("observedAt"))?),
            ("coverage", COVERAGE.into()),
            ("generatedMembership", "unqualified".into()),
        ]))
    }
    fn read_daily_metadata(&mut self, student: &Js, inventory: bool) -> Result<Js> {
        self.requests += 1;
        let raw = self.port.daily_page(student, &"1".into())?;
        project_daily_page(&raw, student, &"1".into(), inventory)
    }
}
fn searched_source(page: &Js, family: &str, requested: bool) -> Js {
    obj(&[
        ("family", family.into()),
        ("operation", page.get("operation").clone()),
        ("page", Js::Num(1.0)),
        ("sourcePeriod", page.get("sourcePeriod").clone()),
        ("requestedPeriodApplied", requested.into()),
        ("coverage", COVERAGE.into()),
        ("observedAt", page.get("observedAt").clone()),
    ])
}
fn source_error(error: Error) -> Error {
    match error.message.as_str() {
        "invalid_capture_shape"
        | "invalid_source_key"
        | "student_search_list_rejected"
        | "invalid_student_name"
        | "native_metadata_row_identity_conflict" => Error::error("source_response_malformed"),
        _ => error,
    }
}
fn source_instant(value: &Js) -> Result<Js> {
    instant(value).map_err(|_| Error::error("source_response_malformed"))
}
pub(crate) fn project_daily_page(
    raw: &Js,
    student: &Js,
    selector: &Js,
    inventory: bool,
) -> Result<Js> {
    // The accepted row and selector algorithms are shared with I1, including
    // duplicate checks and source-key normalization; only the projection grows.
    source_instant(raw.get("observedAt"))?;
    let page = crate::history::project_page_with_titles(raw, student, selector, true)?;
    let mut out = obj(&[
        ("rows", page.get("rows").clone()),
        ("operation", "DtResultSearch".into()),
        ("page", Js::Num(1.0)),
        ("sourcePeriod", Js::Null),
        ("observedAt", page.get("observedAt").clone()),
        ("coverage", COVERAGE.into()),
        ("generatedMembership", "unqualified".into()),
        ("fixedSourceFilter", obj(&[("sort_date", selector.clone())])),
    ]);
    if inventory {
        out.set("expectedPaperDefinitions", "not_observed".into());
    }
    if !selector.is("1") {
        out.set("selector", selector.clone());
    }
    Ok(out)
}
/// Validate the service result independently of owner algorithm construction.
/// A source adapter cannot change the batch's scope, query identity or order.
pub fn qualify_observation(value: &Js, input: &Js, by_name: bool) -> Result<Js> {
    require(
        object(value) && array(value.get("results")),
        "source_response_malformed",
    )?;
    let scope = schema("dailyRecordScopeSchema", value.get("scope"))
        .map_err(|_| Error::error("source_response_malformed"))?;
    require(
        scope.get("sourceNamespace").is("academy.lms.metadata"),
        "source_response_malformed",
    )?;
    let conflict = if by_name {
        "native_metadata_row_identity_conflict"
    } else {
        "metadata_scope_or_query_conflict"
    };
    require(
        (if by_name {
            scope.get("lessonDate") == input.get("lessonDate")
        } else {
            json(&scope) == json(input.get("scope"))
        }) && value.get("results").array().len() == input.get("queries").array().len()
            && value
                .get("results")
                .array()
                .iter()
                .zip(input.get("queries").array())
                .all(|(r, q)| object(r) && json(r.get("query")) == json(q)),
        conflict,
    )?;
    Ok(obj(&[
        ("scope", scope),
        ("results", value.get("results").clone()),
    ]))
}
