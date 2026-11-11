//! Resolve once, enumerate the reference tree, then read only the selected scope.
use crate::{
    backend::{BackendError, ReadClock, SourceBackend},
    display_labels as l,
    envelope::{Ask, AskField, AskOption, Coverage, Envelope, SourceRead, Stop, StopCode},
    facts::TeacherFacts,
    render::cell,
    unit_data::*,
    unit_sources::{self, DailyTestReader},
};
use edu_academy_sources::{
    history::Port,
    native::SourcePort,
    prestudy::PrestudyPort,
    textbook_index::{BookScope, TextbookSectionPort, TextbookStudent},
};

pub fn status<D: SourcePort, P: PrestudyPort, T: TextbookSectionPort, H: Port>(
    backend: &mut SourceBackend<D, P, T>,
    daily: &mut DailyTestReader<H>,
    facts: &dyn TeacherFacts,
    request: &UnitStatusRequest,
    clock: &ReadClock,
) -> Envelope {
    let lesson_at = backend.read_at().to_string();
    backend.set_read_at(&clock.read_at);
    let result = compute(backend, daily, facts, request, clock);
    backend.set_read_at(&lesson_at);
    match result {
        Ok(result) => result,
        Err(error) => error_envelope(error),
    }
}
fn stop(code: StopCode, display: &str) -> BackendError {
    BackendError::Stop(Stop::new(code, display))
}
fn ask(field: AskField, question: String, values: Vec<String>, meaning: &str) -> BackendError {
    if values.is_empty() {
        return stop(StopCode::UnitScopeUnavailable, l::UNIT_SCOPE_STOP);
    }
    BackendError::Ask(Ask {
        field,
        question_ko: question,
        options: values
            .into_iter()
            .map(|v| AskOption {
                value: v.clone(),
                label_ko: cell(&v),
                meaning_ko: meaning.into(),
            })
            .collect(),
    })
}
fn error_envelope(error: BackendError) -> Envelope {
    match error {
        BackendError::Stop(stop) => Envelope::stop(
            stop.message_ko.clone(),
            serde_json::json!({}),
            stop,
            Coverage::default(),
        ),
        BackendError::Ask(ask) => Envelope::ask(
            ask.question_ko.clone(),
            serde_json::json!({}),
            ask,
            Coverage::default(),
        )
        .expect("nonempty options"),
    }
}
fn unit_ask(leaves: &[Leaf]) -> BackendError {
    ask(
        AskField::Unit,
        l::UNIT_ASK.into(),
        leaves.iter().map(leaf_label).collect(),
        l::UNIT_MEANING,
    )
}
pub fn leaf_label(leaf: &Leaf) -> String {
    format!("{} {}", leaf.minor.code, leaf.minor.title)
}
fn code_in<'a>(text: &str, leaves: &'a [Leaf]) -> Vec<&'a Leaf> {
    leaves
        .iter()
        .filter(|leaf| {
            text == leaf.minor.code
                || text == leaf_label(leaf)
                || text
                    .split_whitespace()
                    .any(|token| token == leaf.minor.code)
        })
        .collect()
}
fn infer_topic(request: &UnitStatusRequest) -> Result<String, BackendError> {
    if let Some(topic) = &request.topic {
        return if l::UNIT_TOPICS.contains(&topic.as_str()) {
            Ok(topic.clone())
        } else {
            Err(ask(
                AskField::Topic,
                l::UNIT_TOPIC_ASK.into(),
                l::UNIT_TOPICS.map(str::to_string).to_vec(),
                l::UNIT_TOPIC_MEANING,
            ))
        };
    }
    let text = [
        &request.book,
        &request.unit,
        &request.unit_from,
        &request.unit_to,
    ]
    .into_iter()
    .flatten()
    .cloned()
    .collect::<Vec<_>>()
    .join(" ");
    let mut signals = Vec::new();
    for topic in &l::UNIT_TOPICS[1..] {
        if text.contains(topic) {
            signals.push(*topic);
        }
    }
    if l::UNIT_ACTIVITIES.iter().any(|a| text.contains(a)) && signals.is_empty() {
        signals.push(l::UNIT_TOPICS[3]);
    }
    match signals.as_slice() {
        [] => Ok(l::UNIT_TOPICS[0].into()),
        [one] => Ok((*one).into()),
        _ => Err(ask(
            AskField::Topic,
            l::UNIT_TOPIC_ASK.into(),
            l::UNIT_TOPICS.map(str::to_string).to_vec(),
            l::UNIT_TOPIC_MEANING,
        )),
    }
}
fn compute<D: SourcePort, P: PrestudyPort, T: TextbookSectionPort, H: Port>(
    backend: &mut SourceBackend<D, P, T>,
    daily: &mut DailyTestReader<H>,
    facts: &dyn TeacherFacts,
    r: &UnitStatusRequest,
    clock: &ReadClock,
) -> Result<Envelope, BackendError> {
    if r.student.trim().is_empty() || facts.class(r.student.trim()).is_some() {
        return Err(stop(StopCode::StudentRequired, l::UNIT_STUDENT_REQUIRED));
    }
    let student = backend.resolve_student(r.student.trim(), None)?;
    let topic = infer_topic(r)?;
    let bindings = backend.bindings_for(&student)?;
    if bindings.is_empty() {
        return Err(stop(StopCode::BookNotFound, l::UNIT_BOOK_STOP));
    }
    let labels: Vec<_> = bindings
        .iter()
        .enumerate()
        .map(|(i, b)| {
            if bindings
                .iter()
                .filter(|other| other.book_label == b.book_label)
                .count()
                > 1
            {
                l::unit_course_book(
                    &b.book_label,
                    bindings[..=i]
                        .iter()
                        .filter(|other| other.book_label == b.book_label)
                        .count(),
                )
            } else {
                b.book_label.clone()
            }
        })
        .collect();
    let book_text = r.book.as_deref().unwrap_or("");
    let matches = |label: &str| {
        book_text == label
            || book_text
                .strip_prefix(label)
                .is_some_and(|tail| tail.starts_with(char::is_whitespace))
    };
    // Resolve the registered book name before its course-choice decoration.
    // A shorter registered name must never consume the start of a longer one.
    let matched_book = bindings
        .iter()
        .map(|b| b.book_label.as_str())
        .filter(|name| matches(name))
        .max_by_key(|name| name.len());
    let label_match = labels
        .iter()
        .enumerate()
        .filter(|(i, label)| {
            Some(bindings[*i].book_label.as_str()) == matched_book && matches(label)
        })
        .max_by_key(|(_, label)| label.len())
        .map(|(i, _)| i);
    let matched_label = label_match.map(|i| labels[i].as_str()).or(matched_book);
    let book_suffix = matched_label
        .and_then(|label| book_text.strip_prefix(label))
        .unwrap_or("");
    let explicit: Vec<_> = if let Some(i) = label_match {
        vec![&bindings[i]]
    } else {
        bindings
            .iter()
            .filter(|b| Some(b.book_label.as_str()) == matched_book)
            .collect()
    };
    if !book_text.is_empty() && explicit.is_empty() {
        return Err(stop(StopCode::BookNotFound, l::UNIT_BOOK_STOP));
    }
    let mut text = [
        r.unit.as_deref(),
        r.unit_from.as_deref(),
        r.unit_to.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    if !book_suffix.is_empty() && r.unit.is_none() && r.unit_from.is_none() && r.unit_to.is_none() {
        text = format!("{book_suffix} {text}");
    }
    let compatible: Vec<_> = bindings
        .iter()
        .filter(|b| {
            facts
                .edition_tree(&b.book_label)
                .is_some_and(|t| !code_in(&text, &t.leaves()).is_empty())
        })
        .collect();
    let mut candidates = if !explicit.is_empty() {
        explicit
    } else if !compatible.is_empty() {
        compatible
    } else {
        bindings.iter().collect()
    };
    if candidates.len() > 1 && book_text.is_empty() && topic != l::UNIT_TOPICS[2] {
        let context = facts
            .reports()
            .into_iter()
            .filter(|f| {
                f.student_key.as_deref() == Some(&student.key)
                    && candidates.iter().any(|b| b.book_label == f.book)
                    && (text.trim().is_empty()
                        || facts
                            .edition_tree(&f.book)
                            .is_some_and(|t| !code_in(&text, &t.leaves()).is_empty()))
            })
            .max_by(|a, b| a.assigned_on.cmp(&b.assigned_on));
        if let Some(f) = context {
            let scoped: Vec<_> = candidates
                .iter()
                .copied()
                .filter(|b| {
                    b.book_label == f.book
                        && f.course_key.as_ref().is_none_or(|key| key == &b.course_key)
                        && f.book_no.as_ref().is_none_or(|key| key == &b.book_no)
                })
                .collect();
            if !scoped.is_empty() {
                candidates = scoped;
            }
        }
    }
    let binding = if let [one] = candidates.as_slice() {
        *one
    } else {
        let mut books = Vec::new();
        for b in &candidates {
            if !books.contains(&b.book_label.as_str()) {
                books.push(b.book_label.as_str());
            }
        }
        let duplicates: Vec<_> = books
            .iter()
            .filter_map(|book| {
                let count = bindings
                    .iter()
                    .filter(|b| b.book_label.as_str() == *book)
                    .count();
                (count > 1).then(|| l::unit_course_registration(&cell(book), count))
            })
            .collect();
        let question = if duplicates.is_empty() {
            if topic == l::UNIT_TOPICS[2] {
                l::unit_book_ask(&cell(&student.name), candidates.len())
            } else {
                l::unit_book_other_ask(&cell(&student.name))
            }
        } else {
            l::unit_course_ask(&cell(&student.name), &duplicates.join("\n"))
        };
        // The book field is replaced on the next stateless call. Carry its
        // original unit suffix in every choice rather than widening the scope.
        let suffix = book_suffix;
        return Err(ask(
            AskField::Book,
            question,
            bindings
                .iter()
                .enumerate()
                .filter(|(_, b)| candidates.iter().any(|other| std::ptr::eq(*other, *b)))
                .map(|(i, _)| format!("{}{suffix}", labels[i]))
                .collect(),
            if topic == l::UNIT_TOPICS[2] {
                l::UNIT_BOOK_MEANING
            } else {
                l::UNIT_BOOK_OTHER_MEANING
            },
        ));
    };
    let tree = facts
        .edition_tree(&binding.book_label)
        .ok_or_else(|| stop(StopCode::EditionTreeUnavailable, l::UNIT_TREE_STOP))?;
    let all = tree.leaves();
    if all.is_empty() {
        return Err(stop(StopCode::UnitScopeUnavailable, l::UNIT_SCOPE_STOP));
    }
    // A unit answer replaces ambiguous activity words retained in range endpoints.
    let activity_text = r.unit.as_deref().unwrap_or(&text);
    let mut activity_signals: Vec<_> = l::UNIT_ACTIVITIES
        .iter()
        .filter(|a| activity_text.contains(**a))
        .collect();
    activity_signals.dedup();
    let activity = activity_signals.first().map(|v| (**v).to_string());
    let levels: Vec<_> = l::UNIT_LEVELS
        .iter()
        .filter(|level| text.contains(**level))
        .collect();
    let level = if let [level] = levels.as_slice() {
        Some((**level).to_string())
    } else {
        None
    };
    let unit_text = r
        .unit
        .as_deref()
        .or_else(|| (!book_text.is_empty()).then_some(text.trim()))
        .filter(|s| !s.is_empty());
    let mut minors = if r.unit_from.is_some() || r.unit_to.is_some() {
        let from = r.unit_from.as_deref().and_then(|s| {
            let found = code_in(s, &all);
            if let [leaf] = found.as_slice() {
                Some(leaf.minor.code.clone())
            } else {
                None
            }
        });
        let to = r.unit_to.as_deref().and_then(|s| {
            let found = code_in(s, &all);
            if let [leaf] = found.as_slice() {
                Some(leaf.minor.code.clone())
            } else {
                None
            }
        });
        let start = from
            .as_ref()
            .and_then(|code| all.iter().position(|l| &l.minor.code == code));
        let end = to
            .as_ref()
            .and_then(|code| all.iter().position(|l| &l.minor.code == code));
        if start.is_none() {
            let choices = &all[..=end.unwrap_or(all.len() - 1)];
            return Err(ask(
                AskField::UnitFrom,
                l::UNIT_FROM_ASK.into(),
                choices.iter().map(leaf_label).collect(),
                l::UNIT_MEANING,
            ));
        }
        let start = start.expect("validated start");
        if end.is_none() || end.is_some_and(|end| end < start) {
            return Err(ask(
                AskField::UnitTo,
                l::UNIT_TO_ASK.into(),
                all[start..].iter().map(leaf_label).collect(),
                l::UNIT_MEANING,
            ));
        }
        let end = end.expect("validated end");
        let range = all[start..=end].to_vec();
        if r.unit.as_deref().is_some_and(|u| {
            let codes = code_in(u, &all);
            codes.len() != 1 || !range.contains(codes[0])
        }) {
            return Err(unit_ask(&range));
        }
        range
    } else if let Some(unit) = unit_text {
        let found = code_in(unit, &all);
        if found.len() != 1 {
            return Err(unit_ask(&all));
        }
        vec![found[0].clone()]
    } else if !book_text.is_empty() || topic == l::UNIT_TOPICS[2] {
        all.clone()
    } else {
        let reports = facts.reports();
        let context = reports
            .iter()
            .filter(|f| {
                f.student_key.as_deref() == Some(&student.key)
                    && f.book == binding.book_label
                    && f.kind == "assigned_stages"
            })
            .max_by(|a, b| a.assigned_on.cmp(&b.assigned_on));
        let index = context
            .and_then(|f| all.iter().position(|l| l.minor.code == f.unit))
            .ok_or_else(|| unit_ask(&all))?;
        all[index.saturating_sub(2)..=index].to_vec()
    };
    if activity_signals.len() > 1 {
        return Err(unit_ask(&minors));
    }
    // Bound reads as well as output. Renderer may omit additional whole leaves for bytes.
    let first_omitted = minors.get(60).map(leaf_label);
    minors.truncate(60);
    let mut data = UnitData {
        course_key: binding.course_key.clone(),
        book_no: binding.book_no.clone(),
        student_key: student.key.clone(),
        student: student.name.clone(),
        edition: binding.book_label.clone(),
        family: tree.family,
        topic: topic.clone(),
        default_read_at: clock.read_at.clone(),
        read_at: clock.read_at.clone(),
        tree_read_at: tree.read_at,
        minors,
        activity,
        level,
        daily: None,
        sections: None,
        judgments: facts
            .unit_judgments(&student.key)
            .into_iter()
            .filter(|j| {
                j.student_key == student.key
                    && j.edition == binding.book_label
                    && j.course_key
                        .as_ref()
                        .is_none_or(|key| key == &binding.course_key)
                    && j.book_no.as_ref().is_none_or(|key| key == &binding.book_no)
            })
            .collect(),
        excluded: facts
            .excluded_sections(&student.key)
            .into_iter()
            .filter(|j| {
                j.student_key == student.key
                    && j.edition == binding.book_label
                    && j.state == "active"
                    && j.course_key
                        .as_ref()
                        .is_none_or(|key| key == &binding.course_key)
                    && j.book_no.as_ref().is_none_or(|key| key == &binding.book_no)
            })
            .collect(),
        clinic: Vec::new(),
        first_omitted,
        stages: Vec::new(),
        prestudy: Vec::new(),
    };
    let supplementary = data.family == "supplementary";
    let all_topics = topic == l::UNIT_TOPICS[0];
    let mut requested = 0;
    let mut answered = 0;
    let mut coverage = Coverage::default();
    coverage.sources.push(SourceRead {
        name: "student_book_bindings".into(),
        read_at: clock.read_at.clone(),
        scope: format!("{} / {}", student.key, data.edition),
    });
    coverage.sources.push(SourceRead {
        name: "edition_tree".into(),
        read_at: data.tree_read_at.clone(),
        scope: data.edition.clone(),
    });
    if (all_topics || topic == l::UNIT_TOPICS[1]) && !supplementary {
        requested += 1;
        match daily.read(&student) {
            Ok(mut read) => {
                answered += 1;
                read.attempts.retain(|a| {
                    (a.edition_mapping.is_none()
                        || a.edition_mapping.as_deref() == Some(&data.edition))
                        && (a.unit_mapping.is_none()
                            || data
                                .minors
                                .iter()
                                .any(|l| Some(&l.minor.code) == a.unit_mapping.as_ref()))
                        && data.level.as_ref().is_none_or(|level| {
                            a.level.as_ref().is_none_or(|actual| actual == level)
                        })
                });
                read.expected_today.retain(|e| {
                    e.edition == data.edition
                        && data.minors.iter().any(|l| l.minor.code == e.unit)
                        && data.level.as_ref().is_none_or(|level| &e.level == level)
                });
                data.read_at = read.read_at.clone();
                coverage.sources.extend(read.sources.clone());
                coverage.unknowns.extend(read.unknowns.clone());
                data.daily = Some(read);
            }
            Err(_) => coverage.unknowns.push("daily_unavailable".into()),
        }
    }
    if all_topics || topic == l::UNIT_TOPICS[2] || topic == l::UNIT_TOPICS[3] {
        requested += 1;
        let ts = TextbookStudent::new(&student.key, &student.name)
            .map_err(|_| BackendError::source_unavailable())?;
        let scope = BookScope::new(&binding.course_key, &binding.book_no)
            .map_err(|_| BackendError::source_unavailable())?;
        let labels: Vec<_> = bindings.iter().map(|b| b.book_label.clone()).collect();
        match unit_sources::sections(
            backend,
            &unit_sources::SectionSelection {
                student: &ts,
                scope: &scope,
                edition: &data.edition,
                tree: &all,
                leaves: &data.minors,
                activity: data.activity.as_deref(),
                bound_editions: &labels,
            },
        ) {
            Ok(read) => {
                answered += 1;
                coverage.sources.push(SourceRead {
                    name: "section_index".into(),
                    read_at: clock.read_at.clone(),
                    scope: format!(
                        "{} / wrong-note observed / paper_search={}",
                        data.edition, read.paper_observed
                    ),
                });
                if !read.records.is_empty()
                    || read
                        .observations
                        .iter()
                        .any(|o| o.state == SectionState::Absent)
                {
                    coverage.sources.push(SourceRead {
                        name: "section_grades".into(),
                        read_at: clock.read_at.clone(),
                        scope: data.edition.clone(),
                    });
                }
                coverage.unknowns.extend(read.unknowns.clone());
                data.sections = Some(read);
            }
            Err(_) => coverage.unknowns.push("section_unavailable".into()),
        }
        data.clinic = clinic(&data);
        let reports = facts.reports();
        for leaf in &data.minors {
            for activity in leaf
                .minor
                .activities
                .iter()
                .filter(|a| data.activity.as_ref().is_none_or(|filter| filter == *a))
            {
                let matches = |f: &&crate::facts::TeacherFactReport| {
                    f.student_key.as_deref() == Some(&data.student_key)
                        && f.book == data.edition
                        && f.course_key.as_deref() == Some(&data.course_key)
                        && f.book_no.as_deref() == Some(&data.book_no)
                        && f.unit == leaf.minor.code
                        && (f.activity.is_empty() || f.activity == *activity)
                        && f.snapshot.as_ref().is_none_or(|key| {
                            data.sections.as_ref().is_some_and(|s| {
                                s.observations.iter().any(|o| {
                                    o.attempt_key == *key
                                        && o.unit.as_deref() == Some(&leaf.minor.code)
                                        && o.activity.as_deref() == Some(activity)
                                })
                            })
                        })
                };
                let scoped_reports: Vec<_> = reports.iter().filter(matches).collect();
                let snapshots: Vec<Option<String>> =
                    if scoped_reports.iter().any(|f| f.snapshot.is_some()) {
                        let mut keys = Vec::new();
                        if let Some(sections) = &data.sections {
                            for o in &sections.observations {
                                if o.unit.as_deref() == Some(&leaf.minor.code)
                                    && o.activity.as_deref() == Some(activity)
                                    && !keys.contains(&Some(o.attempt_key.clone()))
                                {
                                    keys.push(Some(o.attempt_key.clone()));
                                }
                            }
                        }
                        keys
                    } else {
                        vec![None]
                    };
                for snapshot in snapshots {
                    let in_attempt = |f: &&&crate::facts::TeacherFactReport| {
                        f.snapshot.is_none() || f.snapshot == snapshot
                    };
                    let prerequisite = scoped_reports
                        .iter()
                        .filter(in_attempt)
                        .filter(|f| f.kind == "original_work")
                        .max_by(|a, b| a.reported_on.cmp(&b.reported_on))
                        .and_then(|f| f.completed);
                    for (index, name) in [
                        "original_clinic_solving",
                        "original_clinic_grading",
                        "similar_clinic",
                        "advanced_clinic",
                        "cism_support",
                        "printed",
                        "handout",
                    ]
                    .iter()
                    .enumerate()
                    {
                        let teacher = scoped_reports
                            .iter()
                            .filter(in_attempt)
                            .filter(|f| f.kind == "clinic_stage" && f.stage == *name)
                            .max_by(|a, b| a.reported_on.cmp(&b.reported_on));
                        let verdict = if teacher.is_some_and(|f| f.completed == Some(true)) {
                            UnitVerdict::Confirmed
                        } else {
                            dependent_stage(prerequisite, index < 2)
                        };
                        data.stages.push(Stage {
                            unit: leaf.minor.code.clone(),
                            activity: Some(activity.clone()),
                            snapshot: snapshot.clone(),
                            stage: (*name).into(),
                            verdict,
                        });
                    }
                }
            }
        }
    }

    if all_topics || topic == l::UNIT_TOPICS[4] {
        let verdicts = if supplementary {
            vec![UnitVerdict::NotApplicable; data.minors.len()]
        } else if !backend.unique_source_name(&student.name) {
            // The source has no keyed association: this is an unresolved identity, not outage.
            requested += 1;
            answered += 1;
            coverage.unknowns.push("prestudy_name_ambiguous".into());
            vec![UnitVerdict::Unknown; data.minors.len()]
        } else {
            requested += 1;
            match backend.read_prestudy_lists(&student.name) {
                Ok(lists) => {
                    answered += 1;
                    coverage.sources.push(SourceRead {
                        name: "prestudy".into(),
                        read_at: lists.observed_at.clone(),
                        scope: data.edition.clone(),
                    });
                    data.minors
                        .iter()
                        .map(|leaf| {
                            match SourceBackend::<D, P, T>::project_prestudy(
                                &lists,
                                &data.edition,
                                &leaf.minor.code,
                            )
                            .0
                            {
                                crate::render::Verdict::Waiting => UnitVerdict::Waiting,
                                crate::render::Verdict::Completed => UnitVerdict::Completed,
                                crate::render::Verdict::NotObserved => UnitVerdict::NotObserved,
                                _ => UnitVerdict::Unknown,
                            }
                        })
                        .collect()
                }
                Err(_) => {
                    coverage.unknowns.push("prestudy_unavailable".into());
                    vec![UnitVerdict::Unknown; data.minors.len()]
                }
            }
        };
        for (leaf, verdict) in data.minors.iter().zip(verdicts) {
            data.prestudy.push(Stage {
                unit: leaf.minor.code.clone(),
                activity: None,
                snapshot: None,
                stage: l::VIDEO.into(),
                verdict,
            });
        }
    }
    for leaf in &data.minors {
        for stage in ["dailytest", "chapter_print", "prestudy"] {
            if supplementary {
                data.stages.push(Stage {
                    unit: leaf.minor.code.clone(),
                    activity: None,
                    snapshot: None,
                    stage: stage.into(),
                    verdict: UnitVerdict::NotApplicable,
                });
            }
        }
    }
    if requested > 0 && answered == 0 {
        return Err(BackendError::source_unavailable());
    }
    for f in facts.reports().iter().filter(|f| {
        f.student_key.as_deref() == Some(&data.student_key)
            && f.book == data.edition
            && (f.course_key.is_none() || f.book_no.is_none())
            && f.course_key
                .as_ref()
                .is_none_or(|key| key == &data.course_key)
            && f.book_no.as_ref().is_none_or(|key| key == &data.book_no)
            && (f.kind == "original_work" || f.kind == "clinic_stage")
            && data
                .activity
                .as_ref()
                .is_none_or(|a| f.activity.is_empty() || &f.activity == a)
            && data.minors.iter().any(|leaf| leaf.minor.code == f.unit)
            && (all_topics || topic == l::UNIT_TOPICS[2])
    }) {
        let mut scope: Vec<_> = [&f.unit, &f.activity]
            .into_iter()
            .filter(|value| !value.is_empty())
            .map(String::as_str)
            .collect();
        if f.kind == "clinic_stage" && !f.stage.is_empty() {
            scope.push(l::unit_stage_name(&f.stage));
        }
        coverage
            .unknowns
            .push(format!("teacher_fact_course_unknown:{}", scope.join(" ")));
    }
    if data
        .judgments
        .iter()
        .any(|j| data.minors.iter().any(|l| l.minor.code == j.unit))
        || !data.excluded.is_empty()
        || facts.reports().iter().any(|f| {
            f.student_key.as_deref() == Some(&data.student_key)
                && f.book == data.edition
                && f.course_key
                    .as_ref()
                    .is_none_or(|key| key == &data.course_key)
                && f.book_no.as_ref().is_none_or(|key| key == &data.book_no)
                && data.minors.iter().any(|l| l.minor.code == f.unit)
                && (f.kind == "clinic_stage" || f.kind == "original_work")
        })
    {
        coverage.sources.push(SourceRead {
            name: "teacher_facts".into(),
            read_at: clock.read_at.clone(),
            scope: student.key,
        });
    }
    let display = crate::unit_render::render(&mut data, &mut coverage);
    if !crate::unit_render::lint(&display) {
        return Err(stop(StopCode::DisplayInvalid, l::UNIT_SCOPE_STOP));
    }
    Ok(Envelope::ok(
        display,
        serde_json::to_value(data).expect("unit data serializes"),
        coverage,
    ))
}
pub fn clinic(data: &UnitData) -> Vec<ClinicCell> {
    let mut cells = Vec::new();
    for leaf in &data.minors {
        for activity in &leaf.minor.activities {
            let observations: Vec<_> = data
                .sections
                .as_ref()
                .map(|s| {
                    s.observations
                        .iter()
                        .filter(|o| {
                            o.unit.as_deref() == Some(&leaf.minor.code)
                                && o.activity.as_deref() == Some(activity)
                        })
                        .collect()
                })
                .unwrap_or_default();
            let excluded_attempt = |attempt: &str| {
                data.excluded.iter().any(|e| {
                    exclusion_applies(data, e, &leaf.minor.code, activity)
                        && e.snapshot.as_deref().is_none_or(|key| key == attempt)
                })
            };
            let records: Vec<_> = data
                .sections
                .as_ref()
                .map(|s| {
                    s.records
                        .iter()
                        .filter(|r| {
                            r.unit == leaf.minor.code
                                && r.activity == *activity
                                && !excluded_attempt(&r.attempt_key)
                        })
                        .collect()
                })
                .unwrap_or_default();
            let labels: Vec<_> = records
                .iter()
                .flat_map(|r| {
                    r.items
                        .iter()
                        .filter(|i| i.scoring == "X")
                        .filter_map(|i| i.label.clone())
                })
                .collect();
            let wrong = records
                .iter()
                .any(|r| r.items.iter().any(|i| i.scoring == "X"));
            let excluded = data.excluded.iter().any(|e| {
                e.snapshot.is_none() && exclusion_applies(data, e, &leaf.minor.code, activity)
            }) || !observations.is_empty()
                && observations
                    .iter()
                    .all(|o| excluded_attempt(&o.attempt_key));
            let unread = observations
                .iter()
                .any(|o| o.state != SectionState::Graded && !excluded_attempt(&o.attempt_key));
            let ambiguous_exclusion = data
                .excluded
                .iter()
                .any(|e| exclusion_scope_ambiguous(data, e, &leaf.minor.code, activity));
            let verdict = if ambiguous_exclusion {
                UnitVerdict::Unknown
            } else if excluded {
                UnitVerdict::Excluded
            } else if !observations.is_empty()
                && observations.iter().all(|o| o.state == SectionState::Absent)
            {
                UnitVerdict::NotObservedInEitherSource
            } else if wrong {
                UnitVerdict::Eligible
            } else if unread || records.is_empty() {
                UnitVerdict::Unknown
            } else {
                UnitVerdict::NotEligible
            };
            cells.push(ClinicCell {
                unit: leaf.minor.code.clone(),
                activity: activity.clone(),
                verdict,
                labels,
            });
        }
    }
    cells
}
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct Stage {
    pub activity: Option<String>,
    pub snapshot: Option<String>,
    pub unit: String,
    pub stage: String,
    pub verdict: UnitVerdict,
}
pub fn dependent_stage(prerequisite_finished: Option<bool>, teacher_only: bool) -> UnitVerdict {
    if prerequisite_finished == Some(false) {
        UnitVerdict::NotYetExpected
    } else if teacher_only {
        UnitVerdict::TeacherReportedNoServer
    } else {
        UnitVerdict::Unknown
    }
}

pub fn exclusion_applies(data: &UnitData, e: &ExcludedSection, unit: &str, activity: &str) -> bool {
    exclusion_matches_scope(data, e, unit, activity)
        && !exclusion_scope_ambiguous(data, e, unit, activity)
        && e.snapshot.as_ref().is_none_or(|snapshot| {
            data.sections.as_ref().is_some_and(|s| {
                s.observations.iter().any(|o| {
                    o.attempt_key == *snapshot
                        && o.unit.as_deref() == Some(unit)
                        && o.activity.as_deref() == Some(activity)
                })
            })
        })
}

pub fn exclusion_scope_ambiguous(
    data: &UnitData,
    e: &ExcludedSection,
    unit: &str,
    activity: &str,
) -> bool {
    e.snapshot.is_none()
        && exclusion_matches_scope(data, e, unit, activity)
        && data.sections.as_ref().is_some_and(|s| {
            s.observations
                .iter()
                .filter(|o| {
                    o.unit.as_deref() == Some(unit) && o.activity.as_deref() == Some(activity)
                })
                .map(|o| &o.attempt_key)
                .collect::<std::collections::HashSet<_>>()
                .len()
                > 1
        })
}

fn exclusion_matches_scope(
    data: &UnitData,
    e: &ExcludedSection,
    unit: &str,
    activity: &str,
) -> bool {
    data.activity
        .as_ref()
        .is_none_or(|filter| filter == activity)
        && e.student_key == data.student_key
        && e.edition == data.edition
        && e.course_key.as_deref() == Some(&data.course_key)
        && e.book_no.as_deref() == Some(&data.book_no)
        && e.unit == unit
        && e.activity == activity
        && e.state == "active"
}
