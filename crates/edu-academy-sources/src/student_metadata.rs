//! Full owner-scoped metadata batches and four-operation observation reader.
//! MCP dispatch/release wiring is deliberately owned by E09c, not this module.
use crate::{
    data::*,
    dom::{self, Document},
    http,
    owner_admission::{TransportAdmission, TransportScope},
    student_course::{self as course, *},
};
use std::collections::HashSet;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Period {
    pub from: String,
    pub through: String,
}
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Query {
    StudentPapers {
        period: Period,
    },
    DailyMetadata,
    DailyInventory,
    DailyHistory {
        periods: Vec<String>,
    },
    PrintedPaperMetadata {
        #[serde(rename = "paperCode")]
        paper_code: String,
        period: Period,
    },
}
impl Query {
    fn parse(v: &Js) -> Result<Self> {
        // Serde unit variants can ignore extra fields. Require the exact
        // live shape before deserialization or any batch source read.
        match string(v.get("kind")).as_str() {
            "daily_metadata" | "daily_inventory" => exact(v, &["kind"])?,
            "daily_history" => {
                exact(v, &["kind", "periods"])?;
                let periods = v.get("periods");
                require(
                    array(periods)
                        && (1..=6).contains(&periods.array().len())
                        && periods.array().iter().all(Js::string),
                    "tool_input_invalid",
                )?;
            }
            "student_papers" | "printed_paper_metadata" => {
                if v.get("kind").is("student_papers") {
                    exact(v, &["kind", "period"])?;
                } else {
                    exact(v, &["kind", "paperCode", "period"])?;
                    require(v.get("paperCode").string(), "tool_input_invalid")?;
                }
                let period = v.get("period");
                exact(period, &["from", "through"])?;
                require(
                    period.get("from").string() && period.get("through").string(),
                    "tool_input_invalid",
                )?;
            }
            _ => return Err(Error::refusal("tool_input_invalid")),
        }
        // Every admitted field is now scalar or a bounded flat collection.
        // Never recursively stringify arbitrary caller data to validate it.
        let q: Self =
            serde_json::from_str(&json(v)).map_err(|_| Error::refusal("tool_input_invalid"))?;
        match &q {
            Self::StudentPapers { period } | Self::PrintedPaperMetadata { period, .. } => {
                date(&period.from.as_str().into())?;
                date(&period.through.as_str().into())?;
                require(period.from <= period.through, "tool_input_invalid")?;
            }
            Self::DailyHistory { periods } => {
                require(
                    (1..=6).contains(&periods.len())
                        && periods
                            .iter()
                            .all(|s| ["11", "12", "13", "14", "15", "16"].contains(&s.as_str()))
                        && periods.iter().collect::<HashSet<_>>().len() == periods.len(),
                    "tool_input_invalid",
                )?;
            }
            _ => {}
        }
        if let Self::PrintedPaperMetadata { paper_code, .. } = &q {
            crate::history::decode_code(&paper_code.as_str().into())?;
        }
        Ok(q)
    }
    fn kind(&self) -> &str {
        match self {
            Self::StudentPapers { .. } => "student_papers",
            Self::DailyMetadata => "daily_metadata",
            Self::DailyInventory => "daily_inventory",
            Self::DailyHistory { .. } => "daily_history",
            Self::PrintedPaperMetadata { .. } => "printed_paper_metadata",
        }
    }
    fn value(&self) -> Js {
        parse(&serde_json::to_string(self).expect("query")).expect("query data")
    }
    fn period(&self) -> Option<&Period> {
        match self {
            Self::StudentPapers { period } | Self::PrintedPaperMetadata { period, .. } => {
                Some(period)
            }
            _ => None,
        }
    }
}
/// Separate namespace policy; the transport admission and configured owner are
/// also required. A DailyRecord identity is never metadata permission.
pub struct MetadataPolicy {
    teacher_key: String,
    from: Option<String>,
    through: Option<String>,
    selectors: Vec<String>,
    operations: Vec<String>,
}
impl MetadataPolicy {
    pub fn parse(v: &Js) -> Result<Self> {
        let keys = [
            "schemaVersion",
            "sourceNamespace",
            "teacherId",
            "allowedFrom",
            "allowedThrough",
            "studentScope",
            "relativeSelectors",
            "operations",
        ];
        exact(v, &keys)?;
        require(
            v.get("schemaVersion").is("academy.metadata-admission/1")
                && v.get("sourceNamespace").is(METADATA_NS)
                && (v.get("studentScope").is("configured")
                    || v.get("studentScope").is("source_verified")),
            "metadata_namespace_not_admitted",
        )?;
        let nullable_date = |v: &Js| {
            if v == &Js::Null {
                Ok(None)
            } else {
                date(v).map(Some)
            }
        };
        let from = nullable_date(v.get("allowedFrom"))?;
        let through = nullable_date(v.get("allowedThrough"))?;
        require(
            from.as_ref()
                .zip(through.as_ref())
                .is_none_or(|(f, t)| f <= t),
            "metadata_namespace_not_admitted",
        )?;
        let list = |k: &str, allowed: &[&str], min: usize, max: usize| -> Result<Vec<String>> {
            let a = v.get(k);
            require(
                array(a)
                    && (min..=max).contains(&a.array().len())
                    && a.array().iter().all(|x| allowed.iter().any(|s| x.is(s)))
                    && edu_academy_core::js::unique(a.array()).len() == a.array().len(),
                "metadata_namespace_not_admitted",
            )?;
            Ok(a.array().iter().map(string).collect())
        };
        Ok(Self {
            teacher_key: key(v.get("teacherId"))?,
            from,
            through,
            selectors: list(
                "relativeSelectors",
                &["1", "11", "12", "13", "14", "15", "16"],
                0,
                7,
            )?,
            operations: list(
                "operations",
                &[
                    "student_papers",
                    "daily_metadata",
                    "daily_inventory",
                    "daily_history",
                    "printed_paper_metadata",
                ],
                1,
                5,
            )?,
        })
    }
    fn date_admitted(&self, d: &str) -> bool {
        self.from.as_ref().is_none_or(|f| d >= f.as_str())
            && self.through.as_ref().is_none_or(|t| d <= t.as_str())
    }
    fn authorize(&self, queries: &[Query], d: &str) -> Result<()> {
        require(
            self.date_admitted(d)
                && queries.iter().all(|q| {
                    self.operations.iter().any(|k| k == q.kind())
                        && q.period().is_none_or(|p| {
                            self.date_admitted(&p.from) && self.date_admitted(&p.through)
                        })
                        && match q {
                            Query::StudentPapers { .. } => true,
                            Query::DailyHistory { periods } => std::iter::once(&"1".to_string())
                                .chain(periods)
                                .all(|s| self.selectors.contains(s)),
                            _ => self.selectors.iter().any(|s| s == "1"),
                        }
                }),
            "metadata_namespace_not_admitted",
        )
    }
}
/// Owner-published structural form pin, independent from source JavaScript.
/// The source response may only be compared to this pin, never create it.
/// R12 supplies the real owner pin; tests supply invented form/filter options.
pub struct PaperFormPin {
    value: Js,
}
impl PaperFormPin {
    pub fn parse(v: &Js) -> Result<Self> {
        exact(v, &["schemaVersion", "action", "method", "fields"])?;
        require(
            v.get("schemaVersion").is("academy.paper-search-form/1")
                && v.get("method").is("post")
                && v.get("action").string(),
            "contract_drift",
        )?;
        let action = string(v.get("action"));
        let url = url::Url::parse(http::ORIGIN)
            .expect("origin")
            .join(&action)
            .map_err(|_| Error::refusal("contract_drift"))?;
        require(
            url.origin().ascii_serialization() == http::ORIGIN
                && url.path() == PAPER_PATH
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none()
                && url.query_pairs().collect::<Vec<_>>()
                    == vec![
                        ("p_process".into(), "UserBySearchTestResult".into()),
                        ("ass_no".into(), "1001".into()),
                    ],
            "contract_drift",
        )?;
        let fields = v.get("fields");
        let expected = [
            "clg_no",
            "cls_no",
            "p_pageno",
            "check_da_test",
            "check_fa_test",
            "check_na_test",
            "stu_name",
            "chioce_cls",
            "sort_date1",
            "sort_date2",
        ];
        require(
            array(fields) && fields.array().len() == expected.len(),
            "contract_drift",
        )?;
        let mut names = HashSet::new();
        for field in fields.array() {
            exact(
                field,
                &["name", "tag", "type", "value", "checked", "options"],
            )?;
            let n = string(field.get("name"));
            require(
                expected.contains(&n.as_str())
                    && names.insert(n)
                    && ["input", "select"].iter().any(|t| field.get("tag").is(t))
                    && (field.get("type") == &Js::Null || field.get("type").string())
                    && array(field.get("options"))
                    && field.get("options").array().len() <= 200,
                "contract_drift",
            )?;
            require(
                field.get("value") == &Js::Null
                    || field.get("value").string() && field.get("value").units().len() <= 64,
                "contract_drift",
            )?;
            require(
                !field.get("type").is("checkbox") || field.get("value").string(),
                "contract_drift",
            )?;
            require(
                if field.get("type").is("checkbox") {
                    matches!(field.get("checked"), Js::Bool(_))
                } else {
                    field.get("checked") == &Js::Null
                },
                "contract_drift",
            )?;
            let mut values = HashSet::new();
            for option in field.get("options").array() {
                exact(option, &["value", "label"])?;
                require(
                    option.get("value").string()
                        && option.get("value").units().len() <= 64
                        && option.get("label").string()
                        && option.get("label").units().len() <= 200
                        && values.insert(json(option.get("value"))),
                    "contract_drift",
                )?;
            }
            require(
                !field.get("tag").is("select") || !field.get("options").array().is_empty(),
                "contract_drift",
            )?;
            require(
                !field.get("tag").is("input") || field.get("options").array().is_empty(),
                "contract_drift",
            )?;
        }
        Ok(Self { value: v.clone() })
    }
    fn verify(&self, d: &Document) -> Result<()> {
        let forms = d.select("form[name=search]");
        require(forms.len() == 1, "contract_drift")?;
        let form = forms[0];
        verify_form_ownership(d, form)?;
        require(
            dom::select(form, "fieldset[disabled],optgroup[disabled]").is_empty(),
            "contract_drift",
        )?;
        require(
            form.value().attr("action") == Some(&string(self.value.get("action")))
                && form
                    .value()
                    .attr("method")
                    .is_some_and(|m| m.eq_ignore_ascii_case("post")),
            "contract_drift",
        )?;
        let fields = dom::select(form, "[name]");
        require(
            fields.len() == self.value.get("fields").array().len(),
            "contract_drift",
        )?;
        for pin in self.value.get("fields").array() {
            let name = string(pin.get("name"));
            let nodes = fields
                .iter()
                .filter(|e| e.value().attr("name") == Some(name.as_str()))
                .collect::<Vec<_>>();
            require(nodes.len() == 1, "contract_drift")?;
            let node = **nodes.first().expect("one");
            require(
                pin.get("tag").is(node.value().name())
                    && match node.value().attr("type") {
                        Some(t) => pin.get("type").is(t),
                        None => pin.get("type") == &Js::Null,
                    }
                    && node.value().attr("disabled").is_none(),
                "contract_drift",
            )?;
            if pin.get("value") != &Js::Null {
                require(pin.get("value").is(&dom::value(node)), "contract_drift")?;
            }
            if let Js::Bool(checked) = pin.get("checked") {
                require(
                    *checked == node.value().attr("checked").is_some(),
                    "contract_drift",
                )?;
            }
            let options = dom::select(node, "option");
            require(
                options.len() == pin.get("options").array().len(),
                "contract_drift",
            )?;
            for (option, expected) in options.iter().zip(pin.get("options").array()) {
                require(
                    expected.get("value").is(&dom::value(*option))
                        && expected.get("label").is(&dom::content(*option))
                        && option.value().attr("disabled").is_none(),
                    "contract_drift",
                )?;
            }
        }
        Ok(())
    }
}
/// Owns its port so a failed source occurrence is permanently latched for all
/// families. No into_inner/raw-client escape or automatic retry is exposed.
pub struct StudentSource<P: ReadPort> {
    port: P,
    owner: Owner,
    transport: TransportAdmission,
    metadata: MetadataPolicy,
    paper_form: Option<PaperFormPin>,
    latched: bool,
    requests: usize,
}
impl<P: ReadPort> StudentSource<P> {
    pub fn new(
        port: P,
        owner: Owner,
        transport: TransportAdmission,
        metadata: MetadataPolicy,
        paper_form: Option<PaperFormPin>,
    ) -> Result<Self> {
        owner.validate_transport(&transport)?;
        let actual = serde_json::to_value(port.transport_admission())
            .map_err(|_| Error::refusal("student_read_owner_transport_conflict"))?;
        let expected = serde_json::to_value(&transport)
            .map_err(|_| Error::refusal("student_read_owner_transport_conflict"))?;
        require(actual == expected, "student_read_owner_transport_conflict")?;
        require(
            owner.teacher_key == metadata.teacher_key,
            "native_metadata_scope_refused",
        )?;
        Ok(Self {
            port,
            owner,
            transport,
            metadata,
            paper_form,
            latched: false,
            requests: 0,
        })
    }
    pub fn requests(&self) -> usize {
        self.requests
    }
    fn contracts(&self, ids: &[&str]) -> Result<()> {
        for id in ids {
            check_contract(&self.transport, id)?;
        }
        Ok(())
    }
    fn preflight(&self, s: &Student, d: &str, ns: &str) -> Result<TransportScope> {
        let scope = self.owner.scope(s, d, ns);
        self.transport.authorize(&scope)?;
        Ok(scope)
    }
    fn read(
        &mut self,
        scope: &TransportScope,
        id: &str,
        args: Vec<(String, String)>,
    ) -> Result<String> {
        check_contract(&self.transport, id)?;
        self.transport.request(scope, id, &args, false)?;
        require(self.requests < 256, "student_read_request_budget_exceeded")?;
        self.requests += 1;
        response_text(self.port.read(scope, id, &args)?)
    }
    /// Both live tool aliases use the same validation/composition. Named lookup
    /// independently verifies the dated occurrence before any metadata read.
    pub fn metadata(&mut self, tool: &str, input: &Js, observed_at: &str) -> Result<Js> {
        instant(observed_at)?;
        require(
            ["academy_student_metadata_get", "academy_student_metadata"].contains(&tool),
            "student_metadata_tool_not_supported",
        )?;
        let named = tool == "academy_student_metadata_get";
        if named {
            exact(
                input,
                &[
                    "displayName",
                    "lessonDate",
                    "purpose",
                    "currentRequestRef",
                    "queries",
                ],
            )?;
        } else {
            exact(input, &["scope", "purpose", "currentRequestRef", "queries"])?;
            exact(
                input.get("scope"),
                &["sourceNamespace", "teacherId", "studentId", "lessonDate"],
            )?;
        }
        for k in ["purpose", "currentRequestRef"] {
            let v = input.get(k);
            require(
                v.string()
                    && !trim(v.units()).is_empty()
                    && v.units().len() <= 256
                    && !v
                        .units()
                        .iter()
                        .any(|c| matches!(*c, 0..=8 | 11 | 12 | 14..=31 | 127)),
                "tool_input_invalid",
            )?;
        }
        let (student, d) = if named {
            let n = name(&Js::Str(trim(input.get("displayName").units()).to_vec()))?;
            (
                self.owner.student(&n)?.clone(),
                date(input.get("lessonDate"))?,
            )
        } else {
            let s = input.get("scope");
            require(
                s.get("sourceNamespace").is(METADATA_NS)
                    && s.get("teacherId").is(&self.owner.teacher_key),
                "native_metadata_scope_refused",
            )?;
            let id = key(s.get("studentId"))?;
            let student = self
                .owner
                .students
                .iter()
                .find(|s| s.student_key == id)
                .ok_or_else(|| Error::refusal("native_metadata_scope_refused"))?
                .clone();
            (student, date(s.get("lessonDate"))?)
        };
        let qs = input.get("queries");
        require(
            array(qs) && (1..=8).contains(&qs.array().len()),
            "tool_input_invalid",
        )?;
        let queries = qs
            .array()
            .iter()
            .map(Query::parse)
            .collect::<Result<Vec<_>>>()?;
        self.metadata.authorize(&queries, &d)?;
        self.preflight(&student, &d, METADATA_NS)?;
        for q in &queries {
            self.query_contracts(q)?;
            if let Some(p) = q.period() {
                self.preflight(&student, &p.from, METADATA_NS)?;
                self.preflight(&student, &p.through, METADATA_NS)?;
            }
        }
        if named {
            self.day_preflight(&student, &d)?;
        }
        require(!self.latched, "source_failure_latched_no_retry")?;
        let result = (|| {
            if named {
                let rows = self.day_records(&student, &d, None, observed_at)?;
                require(rows.len() <= 1, "native_dated_name_ambiguous")?;
                require(rows.len() == 1, "native_dailyrecord_occurrence_unverified")?;
                require(
                    rows[0].get("studentKey").is(&student.student_key),
                    "native_dated_name_scope_conflict",
                )?;
            }
            let scope = self.owner.scope(&student, &d, METADATA_NS);
            let mut results = vec![];
            for q in &queries {
                let observation = self.query(&scope, &student, q, observed_at)?;
                results.push(obj(&[("query", q.value()), ("observation", observation)]));
                require(
                    json(&Js::Arr(results.clone())).len() <= 8 * 1024 * 1024,
                    "student_read_output_budget_exceeded",
                )?;
            }
            Ok(obj(&[
                ("status", "observed".into()),
                (
                    "scope",
                    obj(&[
                        ("sourceNamespace", METADATA_NS.into()),
                        ("teacherId", self.owner.teacher_key.as_str().into()),
                        ("studentId", student.student_key.as_str().into()),
                        ("lessonDate", d.as_str().into()),
                    ]),
                ),
                ("results", Js::Arr(results)),
                ("academyWrites", 0.into()),
                ("parentSends", 0.into()),
            ]))
        })();
        if result.is_err() {
            self.latched = true;
        }
        result
    }
    fn query_contracts(&self, q: &Query) -> Result<()> {
        if matches!(
            q,
            Query::StudentPapers { .. } | Query::PrintedPaperMetadata { .. }
        ) {
            require(self.paper_form.is_some(), "contract_drift")?;
            self.contracts(&["metadata.paper.initial"])?;
            for mask in 0..8 {
                check_contract(&self.transport, &format!("metadata.paper.search.{mask}"))?;
            }
        }
        if !matches!(q, Query::StudentPapers { .. }) {
            self.contracts(&["metadata.daily"])?;
        }
        Ok(())
    }
    fn daily(
        &mut self,
        scope: &TransportScope,
        s: &Student,
        selector: &str,
        now: &str,
    ) -> Result<(Js, Vec<(String, String)>)> {
        let html = self.read(
            scope,
            "metadata.daily",
            vec![
                ("sort_date".into(), selector.into()),
                ("stu_name".into(), s.student_name.clone()),
            ],
        )?;
        let rows = metadata_rows(&html, s)?;
        let d = Document::parse(&html);
        let controls = d.select("select[name=sort_date]");
        require(controls.len() <= 1, "daily_history_selector_drift")?;
        let mut selectors = vec![];
        if let Some(control) = controls.first() {
            require(control.value().attr("form").is_none(), "contract_drift")?;
            if let Some(form) = dom::closest(*control, "form") {
                verify_form_ownership(&d, form)?;
            }
            for option in dom::select(*control, "option") {
                if option.value().attr("disabled").is_some() {
                    continue;
                }
                let value = dom::value(option);
                let label = dom::content(option);
                require(
                    selectors.len() < 64
                        && !selectors.iter().any(|(s, _)| s == &value)
                        && label.encode_utf16().count() <= 200,
                    "daily_history_selector_drift",
                )?;
                selectors.push((value, label));
            }
        }
        Ok((
            obj(&[
                ("rows", Js::Arr(rows)),
                ("operation", "DtResultSearch".into()),
                ("page", 1.into()),
                ("sourcePeriod", Js::Null),
                ("observedAt", now.into()),
                (
                    "coverage",
                    "one_observed_student_search_page_not_complete_history".into(),
                ),
                ("generatedMembership", "unqualified".into()),
                ("fixedSourceFilter", obj(&[("sort_date", selector.into())])),
                ("teacherJudgment", Js::Null),
            ]),
            selectors,
        ))
    }
    fn paper(&mut self, scope: &TransportScope, s: &Student, p: &Period, now: &str) -> Result<Js> {
        let initial = self.read(scope, "metadata.paper.initial", vec![])?;
        let d = Document::parse(&initial);
        self.paper_form
            .as_ref()
            .ok_or_else(|| Error::refusal("contract_drift"))?
            .verify(&d)?;
        require(
            field(&d, "form[name=userForm] [name=pri_no]")? == self.owner.teacher_key,
            "student_paper_owner_drift",
        )?;
        let forms = d.select("form[name=search]");
        require(
            forms.len() == 1
                && forms[0]
                    .value()
                    .attr("method")
                    .is_some_and(|s| s.eq_ignore_ascii_case("post")),
            "student_paper_search_method_drift",
        )?;
        // Rust constructs the fixed READ wire itself, never invoking or copying
        // onSearchPage, form.action, navigation or a source-provided function.
        let mut args = vec![];
        let mut mask = 0;
        for k in [
            "clg_no",
            "cls_no",
            "check_da_test",
            "check_fa_test",
            "check_na_test",
            "chioce_cls",
        ] {
            let nodes = dom::select(forms[0], &format!("[name={k}]"));
            require(nodes.len() == 1, "student_paper_filter_drift")?;
            let node = nodes[0];
            let v = dom::value(node);
            if let Some(bit) = ["check_da_test", "check_fa_test", "check_na_test"]
                .iter()
                .position(|n| *n == k)
            {
                require(
                    node.value().attr("type") == Some("checkbox"),
                    "student_paper_filter_drift",
                )?;
                if node.value().attr("checked").is_none() {
                    continue;
                }
                mask |= 1 << bit;
            }
            require(v.len() <= 64, "student_paper_filter_drift")?;
            args.push((k.into(), v));
        }
        require(
            args.iter()
                .any(|(k, v)| k == "clg_no" && v == &self.owner.college_key)
                && s.student_name.encode_utf16().count() >= 2,
            "student_paper_college_scope_conflict",
        )?;
        let class = args
            .iter()
            .find(|(k, _)| k == "chioce_cls")
            .expect("collected")
            .1
            .clone();
        args.iter_mut()
            .find(|(k, _)| k == "cls_no")
            .expect("collected")
            .1 = class;
        args.extend([
            ("stu_name".into(), s.student_name.clone()),
            ("sort_date1".into(), p.from.clone()),
            ("sort_date2".into(), p.through.clone()),
        ]);
        let html = self.read(scope, &format!("metadata.paper.search.{mask}"), args)?;
        let d = Document::parse(&html);
        require(
            field(&d, "form[name=userForm] [name=pri_no]")? == self.owner.teacher_key,
            "student_paper_owner_drift",
        )?;
        Ok(obj(&[
            ("rows", Js::Arr(metadata_rows(&html, s)?)),
            ("operation", "UserBySearchTestResult".into()),
            ("page", 1.into()),
            (
                "sourcePeriod",
                obj(&[
                    ("from", p.from.as_str().into()),
                    ("to", p.through.as_str().into()),
                ]),
            ),
            ("observedAt", now.into()),
            (
                "coverage",
                "one_observed_student_search_page_not_complete_history".into(),
            ),
            ("generatedMembership", "unqualified".into()),
            ("teacherJudgment", Js::Null),
        ]))
    }
    fn query(&mut self, scope: &TransportScope, s: &Student, q: &Query, now: &str) -> Result<Js> {
        match q {
            Query::DailyMetadata => self.daily(scope, s, "1", now).map(|x| x.0),
            Query::DailyInventory => {
                let mut page = self.daily(scope, s, "1", now)?.0;
                page.set("expectedPaperDefinitions", "not_observed".into());
                Ok(page)
            }
            Query::StudentPapers { period } => self.paper(scope, s, period, now),
            Query::DailyHistory { periods } => {
                let (current, selectors) = self.daily(scope, s, "1", now)?;
                require(
                    periods
                        .iter()
                        .all(|p| selectors.iter().any(|(s, _)| s == p)),
                    "daily_history_selector_drift",
                )?;
                let mut pages = vec![];
                for selector in periods {
                    let mut page = self.daily(scope, s, selector, now)?.0;
                    page.set("selector", selector.as_str().into());
                    page.set(
                        "label",
                        selectors
                            .iter()
                            .find(|(s, _)| s == selector)
                            .expect("qualified")
                            .1
                            .as_str()
                            .into(),
                    );
                    page.set("expectedPaperDefinitions", "not_observed".into());
                    pages.push(page);
                }
                Ok(obj(&[
                    ("status", "observed".into()),
                    ("studentKey", s.student_key.as_str().into()),
                    ("studentName", s.student_name.as_str().into()),
                    (
                        "currentFilterQualification",
                        obj(&[
                            ("observedAt", now.into()),
                            ("selector", "1".into()),
                            ("rows", current.get("rows").clone()),
                        ]),
                    ),
                    ("observations", Js::Arr(pages)),
                    (
                        "calendarBounds",
                        "source_relative_labels_only_not_exact_calendar_bounds".into(),
                    ),
                    ("expectedPaperDefinitions", "not_observed".into()),
                    (
                        "coverage",
                        "all_parsed_embedded_rows_on_each_observed_response_not_complete_history"
                            .into(),
                    ),
                    ("teacherJudgment", Js::Null),
                ]))
            }
            Query::PrintedPaperMetadata { paper_code, period } => {
                let paper_key = crate::history::decode_code(&paper_code.as_str().into())?;
                let primary = self.paper(scope, s, period, now)?;
                let mut matches = primary
                    .get("rows")
                    .array()
                    .iter()
                    .filter(|r| r.get("paperKey") == &paper_key)
                    .cloned()
                    .collect::<Vec<_>>();
                let mut searched = vec![search_summary(&primary, "student_assessment", true)];
                let mut family = if matches.is_empty() {
                    Js::Null
                } else {
                    "student_assessment".into()
                };
                let mut source_period = primary.get("sourcePeriod").clone();
                if matches.is_empty() {
                    let daily = self.daily(scope, s, "1", now)?.0;
                    searched.push(search_summary(&daily, "daily_zero", false));
                    matches = daily
                        .get("rows")
                        .array()
                        .iter()
                        .filter(|r| r.get("paperKey") == &paper_key)
                        .cloned()
                        .collect();
                    if !matches.is_empty() {
                        family = "daily_zero".into();
                        source_period = Js::Null;
                    }
                }
                Ok(obj(&[
                    ("paperCode", paper_code.as_str().into()),
                    ("paperKey", paper_key),
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
                    ("page", 1.into()),
                    ("observedAt", now.into()),
                    (
                        "coverage",
                        "observed_source_pages_not_complete_history".into(),
                    ),
                    ("matchedSourceFamily", family),
                    ("generatedMembership", "unqualified".into()),
                    ("searchedSources", Js::Arr(searched)),
                    ("teacherJudgment", Js::Null),
                ]))
            }
        }
    }
    fn day_preflight(&self, s: &Student, d: &str) -> Result<()> {
        self.preflight(s, d, DAY_NS)?;
        self.contracts(&[
            "student.day.groups",
            "student.day.main",
            "student.day.selected",
        ])
    }
    fn day_records(
        &mut self,
        s: &Student,
        d: &str,
        groups: Option<&[Js]>,
        now: &str,
    ) -> Result<Vec<Js>> {
        let transport = DayTransport {
            port: &mut self.port,
            admission: &self.transport,
            scope: self.owner.scope(s, d, DAY_NS),
            requests: &mut self.requests,
        };
        let mut client = http::Client::new(self.owner.principal(), transport)?;
        client.authenticated = true;
        let discovered;
        if let Some(g) = groups {
            discovered = g.to_vec();
        } else {
            let v = client.discover(
                &obj(&[
                    ("teacherLabel", self.owner.teacher_label.as_str().into()),
                    ("date", d.into()),
                ]),
                now,
            )?;
            discovered = v
                .get("groups")
                .array()
                .iter()
                .map(|g| g.get("groupId").clone())
                .collect();
        }
        let v = client.execute(
            &obj(&[
                ("family", "day_record".into()),
                ("teacherLabel", self.owner.teacher_label.as_str().into()),
                ("date", d.into()),
                ("groupIds", Js::Arr(discovered)),
                (
                    "studentNames",
                    Js::Arr(vec![s.student_name.as_str().into()]),
                ),
            ]),
            now,
        )?;
        let rows = v.get("dayRecords").array().to_vec();
        require(
            rows.iter().all(|r| {
                r.get("studentName").is(&s.student_name)
                    && r.get("studentKey").is(&s.student_key)
                    && r.get("teacherKey").is(&self.owner.teacher_key)
                    && r.get("date").is(d)
            }),
            "native_dated_name_scope_conflict",
        )?;
        Ok(rows)
    }
    /// Owner_scope is pure configuration; other operations require an explicit
    /// admitted date, including course/book reads that had none in the old shim.
    pub fn observe(&mut self, input: &Js, lesson_date: &str, observed_at: &str) -> Result<Js> {
        let operation = input.get("operation");
        if operation.is("owner_scope") {
            exact(input, &["operation"])?;
            return Ok(obj(&[
                ("status", "observed".into()),
                ("page", self.owner.scope_page()),
                ("requests", 0.into()),
                ("academyWrites", 0.into()),
            ]));
        }
        instant(observed_at)?;
        let d = date(&lesson_date.into())?;
        let course = operation.is("student_course") || operation.is("discover_books");
        require(
            course || operation.is("day_record"),
            "student_read_operation_not_supported",
        )?;
        let students;
        let groups;
        if course {
            exact(input, &["operation", "studentName"])?;
            let n = name(input.get("studentName"))?;
            students = vec![self.owner.student(&n)?.clone()];
            groups = vec![];
            self.preflight(&students[0], &d, COURSE_NS)?;
            self.contracts(&["student.course.initial", "student.course.search"])?;
            if operation.is("discover_books") {
                self.contracts(&["student.books"])?;
            }
        } else {
            exact(input, &["operation", "date", "groupIds", "studentNames"])?;
            require(input.get("date").is(&d), "student_read_date_invalid")?;
            let ids = input.get("groupIds");
            let ns = input.get("studentNames");
            require(
                array(ids)
                    && (1..=12).contains(&ids.array().len())
                    && array(ns)
                    && (1..=64).contains(&ns.array().len()),
                "student_read_scope_size_invalid",
            )?;
            // Js equality is recursive. Qualify every element as a bounded
            // scalar before duplicate checks can compare caller-built values.
            for id in ids.array() {
                key(id)?;
            }
            let student_names = ns.array().iter().map(name).collect::<Result<Vec<_>>>()?;
            require(
                edu_academy_core::js::unique(ids.array()).len() == ids.array().len()
                    && edu_academy_core::js::unique(ns.array()).len() == ns.array().len(),
                "student_read_duplicate_scope",
            )?;
            groups = ids.array().to_vec();
            students = student_names
                .iter()
                .map(|n| self.owner.student(n).cloned())
                .collect::<Result<Vec<_>>>()?;
            require(
                self.requests + students.len() * groups.len() * 2 <= 256,
                "student_read_request_budget_exceeded",
            )?;
            for s in &students {
                self.day_preflight(s, &d)?;
            }
        }
        require(!self.latched, "source_failure_latched_no_retry")?;
        let result = (|| {
            let page = if course {
                let s = &students[0];
                let scope = self.owner.scope(s, &d, COURSE_NS);
                let initial = self.read(&scope, "student.course.initial", vec![])?;
                let member = course_principal(&Document::parse(&initial), &self.owner)?;
                let html = self.read(
                    &scope,
                    "student.course.search",
                    vec![
                        ("clg_no".into(), self.owner.college_key.clone()),
                        ("g_clg_no".into(), self.owner.college_key.clone()),
                        ("g_mem_type".into(), member),
                        ("student_name".into(), s.student_name.clone()),
                    ],
                )?;
                let courses = course::courses(&html, &self.owner, s)?;
                let mut page=obj(&[("schemaVersion",1.into()),("sourceOperation",if operation.is("student_course"){"StudyCourse"}else{"selected_student_course_book_discovery"}.into()),("sourceKind","teacher_website_read".into()),("studentName",s.student_name.as_str().into()),("studentKey",s.student_key.as_str().into()),("courses",Js::Arr(courses.clone())),("observedAt",observed_at.into()),("coverage",if operation.is("student_course"){"selected_student_observed_course_rows_not_complete_roster_or_effective_dates"}else{"selected_student_observed_course_book_bindings_not_whole_current_teacher_roster"}.into()),("currentScreen",screen("server_records_do_not_observe_device_screen")),("clockMeaning","query_observation_time_not_student_action_time".into()),("teacherJudgment",Js::Null)]);
                if operation.is("discover_books") {
                    let html = self.read(
                        &scope,
                        "student.books",
                        vec![("student_name".into(), s.student_name.clone())],
                    )?;
                    page.set("books", Js::Arr(books(&html, s, &courses)?));
                }
                page
            } else {
                let mut rows = vec![];
                for s in &students {
                    rows.extend(self.day_records(s, &d, Some(&groups), observed_at)?);
                }
                obj(&[
                    ("schemaVersion", 1.into()),
                    ("sourceOperation", "day_record".into()),
                    ("sourceKind", "teacher_website_read".into()),
                    (
                        "source",
                        obj(&[
                            ("dayRecords", Js::Arr(rows)),
                            ("observedAt", observed_at.into()),
                            (
                                "coverage",
                                "selected_students_and_groups_not_complete_roster".into(),
                            ),
                        ]),
                    ),
                    (
                        "currentScreen",
                        screen("server_records_do_not_observe_device_screen"),
                    ),
                    ("teacherJudgment", Js::Null),
                ])
            };
            Ok(obj(&[
                ("status", "observed".into()),
                ("page", page),
                ("requests", self.requests.into()),
                ("academyWrites", 0.into()),
            ]))
        })();
        if result.is_err() {
            self.latched = true;
        }
        result
    }
}
fn search_summary(page: &Js, family: &str, period: bool) -> Js {
    obj(&[
        ("family", family.into()),
        ("operation", page.get("operation").clone()),
        ("page", 1.into()),
        ("sourcePeriod", page.get("sourcePeriod").clone()),
        ("requestedPeriodApplied", period.into()),
        ("coverage", page.get("coverage").clone()),
        ("observedAt", page.get("observedAt").clone()),
    ])
}
// These bounds precede recursive parsing, AST projection and recursive drops.
// A token budget also bounds unary/binary/member chains without delimiters.
const METADATA_MAX_DEPTH: usize = 32;
const METADATA_MAX_SCRIPT_TOKENS: usize = 256;
const METADATA_MAX_JSON_VALUES: usize = 100_000;

fn metadata_script_budget(source: &str) -> Result<()> {
    use swc_common::BytePos;
    use swc_ecma_ast::EsVersion;
    use swc_ecma_parser::{StringInput, Syntax, lexer::Lexer};
    let lexer = Lexer::new(
        Syntax::Es(Default::default()),
        EsVersion::Es2022,
        StringInput::new(source, BytePos(1), BytePos(source.len() as u32 + 1)),
        None,
    );
    let mut delimiters = Vec::new();
    for (count, token) in lexer.enumerate() {
        require(
            count < METADATA_MAX_SCRIPT_TOKENS,
            "metadata_parser_resource_limit",
        )?;
        let raw = source
            .get((token.span.lo.0 - 1) as usize..(token.span.hi.0 - 1) as usize)
            .ok_or_else(|| Error::refusal("metadata_parser_resource_limit"))?;
        // Regexp and template subgrammars can hide recursive expressions from
        // this standalone lexer. Refuse them rather than guess parser context.
        // Ordinary quoted strings and comments remain lexically inert.
        require(
            !raw.starts_with(['/', '`']),
            "metadata_parser_resource_limit",
        )?;
        match raw {
            "(" | "[" | "{" => {
                require(
                    delimiters.len() < METADATA_MAX_DEPTH,
                    "metadata_parser_resource_limit",
                )?;
                delimiters.push(raw);
            }
            ")" | "]" | "}" => {
                let expected = match raw {
                    ")" => "(",
                    "]" => "[",
                    _ => "{",
                };
                require(
                    delimiters.pop() == Some(expected),
                    "metadata_parser_resource_limit",
                )?;
            }
            _ => {}
        }
    }
    require(delimiters.is_empty(), "metadata_parser_resource_limit")
}

fn metadata_json_budget(source: &str) -> Result<()> {
    let mut depth = 0usize;
    let mut values = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for c in source.bytes() {
        if quoted {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                quoted = false;
            }
            continue;
        }
        match c {
            b'"' => quoted = true,
            b'[' | b'{' => {
                depth += 1;
                require(
                    depth <= METADATA_MAX_DEPTH,
                    "metadata_parser_resource_limit",
                )?;
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        // Bound potential values/properties as well as recursive depth. String
        // contents are excluded; the existing response-size bound covers them.
        if matches!(c, b'[' | b'{' | b',' | b':') {
            values += 1;
            require(
                values <= METADATA_MAX_JSON_VALUES,
                "metadata_parser_resource_limit",
            )?;
        }
    }
    Ok(())
}

/// Inert ECMAScript AST selection, never evaluation. A comment, string, external
/// script or dynamically constructed JSON is not a metadata source record.
pub fn metadata_rows(html: &str, student: &Student) -> Result<Vec<Js>> {
    key(&student.student_key.as_str().into())?;
    name(&student.student_name.as_str().into())?;
    require(
        html.encode_utf16().count() <= 2_000_000,
        "assessment_search_source_rejected",
    )?;
    let d = Document::parse(html);
    let mut lists = vec![];
    for script in d.select("script:not([src])") {
        let source = dom::content(script);
        metadata_script_budget(&source)?;
        let ast = crate::script::Parser::new(&source)?.program()?;
        for n in crate::script::walk(&ast)? {
            if n.kind != "Property" {
                continue;
            }
            require(
                !n.computed || !n.name.is_empty(),
                "result_list_missing_or_ambiguous",
            )?;
            if n.name != "testResultList" {
                continue;
            }
            let v = n.children.last().expect("property");
            require(
                v.kind == "CallExpression"
                    && v.children.len() == 2
                    && v.children[0].reference().as_deref() == Some("JSON.parse"),
                "result_list_missing_or_ambiguous",
            )?;
            let literal = v.children[1]
                .scalar()
                .filter(Js::string)
                .ok_or_else(|| Error::refusal("result_list_missing_or_ambiguous"))?;
            let text = String::from_utf16(literal.units())
                .map_err(|_| Error::refusal("student_search_list_rejected"))?;
            metadata_json_budget(&text)?;
            lists.push(parse(&text).map_err(|_| Error::refusal("student_search_list_rejected"))?);
        }
    }
    require(lists.len() == 1, "result_list_missing_or_ambiguous")?;
    let list = &lists[0];
    require(
        array(list) && list.array().len() <= 2000,
        "student_search_list_rejected",
    )?;
    let mut seen = HashSet::new();
    let mut rows = vec![];
    for r in list.array() {
        require(object(r), "invalid_capture_shape")?;
        let source_key = |v: &Js| -> Result<String> {
            match v {
                Js::Num(n)
                    if n.is_finite()
                        && *n > 0.0
                        && n.fract() == 0.0
                        && *n <= 9_007_199_254_740_991.0 =>
                {
                    Ok(format!("{n:.0}"))
                }
                Js::Str(_) => key(v),
                _ => Err(Error::refusal("invalid_source_key")),
            }
        };
        let id = source_key(r.get("priNo"))?;
        if id != student.student_key {
            require(
                r.get("userName") != &student.student_name.as_str().into(),
                "student_search_identity_conflict",
            )?;
            continue;
        }
        require(
            name(r.get("userName"))? == student.student_name,
            "student_search_identity_conflict",
        )?;
        let paper = source_key(r.get("pNo"))?;
        let attempt = source_key(r.get("testingNo"))?;
        require(
            seen.insert((paper.clone(), attempt.clone())),
            "duplicate_student_search_row",
        )?;
        let mut titles = obj(&[]);
        for k in [
            "testpageName",
            "testPageName",
            "pName",
            "examName",
            "title",
            "testName",
            "testingName",
            "printTestingName",
            "testTypeName",
            "applyDate",
        ] {
            let v = r.get(k);
            if v.string() && v.units().len() < 2000 {
                titles.set(k, v.clone());
            }
        }
        let raw_id = r.get("_id");
        let row_id = if raw_id.string() {
            string(raw_id)
        } else if matches!(raw_id,Js::Num(n) if n.is_finite()) {
            json(raw_id)
        } else {
            String::new()
        };
        let row_id = if !row_id.is_empty()
            && row_id.len() <= 64
            && row_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            row_id.into()
        } else {
            Js::Null
        };
        let score = match r.get("score") {
            Js::Num(n) if n.is_finite() => Js::Num(*n),
            _ => Js::Null,
        };
        rows.push(obj(&[
            ("studentKey", id.into()),
            ("studentName", student.student_name.as_str().into()),
            ("paperKey", paper.into()),
            ("attemptKey", attempt.into()),
            ("sourceRowId", row_id),
            ("sourceScore", score),
            ("titles", titles),
        ]));
    }
    Ok(rows)
}
