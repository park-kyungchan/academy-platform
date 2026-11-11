//! Teacher-stated facts: one trait, seeded now, Ontology later (plan s5).
use crate::synthetic::Scenario;
use chrono::{Datelike, NaiveDate, Weekday};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassFact {
    pub name: String,
    pub weekdays: Vec<Weekday>,
    pub roster: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovedLesson {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

/// One homework item the teacher said was not brought, until judged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotBrought {
    pub homework_line: String,
    pub assigned_on: NaiveDate,
    pub judged: bool,
}

pub trait TeacherFacts {
    fn class(&self, name: &str) -> Option<ClassFact>;
    fn moved_lessons(&self, class_name: &str) -> &[MovedLesson];
    fn class_names(&self) -> Vec<String> {
        Vec::new()
    }
    /// `student_key` is the resolved student's key, never the spoken name.
    fn not_brought(&self, _student_key: &str) -> Vec<NotBrought> {
        Vec::new()
    }
}

/// Seeded implementation from the scenario file's `classes` section.
#[derive(Clone, Debug, Default)]
pub struct SeededFacts {
    classes: Vec<ClassFact>,
    moved: Vec<(String, Vec<MovedLesson>)>,
    carry_over: Vec<(String, NotBrought)>,
}

impl SeededFacts {
    pub fn from_scenario(scenario: &Scenario) -> Self {
        let mut classes = Vec::new();
        let mut moved = Vec::new();
        for class in &scenario.classes {
            let weekdays = class
                .weekdays
                .iter()
                .filter_map(|w| parse_weekday(w))
                .collect();
            classes.push(ClassFact {
                name: class.name.clone(),
                weekdays,
                roster: class.roster.clone(),
            });
            let lessons = class
                .moved
                .iter()
                .filter_map(|m| {
                    Some(MovedLesson {
                        from: NaiveDate::parse_from_str(&m.from, "%Y-%m-%d").ok()?,
                        to: NaiveDate::parse_from_str(&m.to, "%Y-%m-%d").ok()?,
                    })
                })
                .collect();
            moved.push((class.name.clone(), lessons));
        }
        // Scenario rows name a student; store them under the resolved
        // student's key. An explicit `student_key` row names the key
        // directly, which is how same-name students are told apart.
        let carry_over = scenario
            .carry_over
            .iter()
            .filter_map(|item| {
                let student_key = item.student_key.clone().or_else(|| {
                    let mut matches = scenario.students.iter().filter(|s| s.name == item.student);
                    match (matches.next(), matches.next()) {
                        (Some(only), None) => Some(only.key.clone()),
                        _ => None,
                    }
                });
                Some((
                    student_key.unwrap_or_else(|| item.student.clone()),
                    NotBrought {
                        homework_line: item.homework_line.clone(),
                        assigned_on: NaiveDate::parse_from_str(&item.assigned_on, "%Y-%m-%d")
                            .ok()?,
                        judged: item.judged,
                    },
                ))
            })
            .collect();
        Self {
            classes,
            moved,
            carry_over,
        }
    }
}

impl TeacherFacts for SeededFacts {
    fn class_names(&self) -> Vec<String> {
        self.classes.iter().map(|c| c.name.clone()).collect()
    }
    fn not_brought(&self, student_key: &str) -> Vec<NotBrought> {
        self.carry_over
            .iter()
            .filter(|(key, _)| key == student_key)
            .map(|(_, item)| item.clone())
            .collect()
    }
    fn class(&self, name: &str) -> Option<ClassFact> {
        self.classes.iter().find(|c| c.name == name).cloned()
    }
    fn moved_lessons(&self, class_name: &str) -> &[MovedLesson] {
        self.moved
            .iter()
            .find(|(n, _)| n == class_name)
            .map_or(&[], |(_, v)| v.as_slice())
    }
}

pub fn parse_weekday(value: &str) -> Option<Weekday> {
    match value {
        "Mon" => Some(Weekday::Mon),
        "Tue" => Some(Weekday::Tue),
        "Wed" => Some(Weekday::Wed),
        "Thu" => Some(Weekday::Thu),
        "Fri" => Some(Weekday::Fri),
        "Sat" => Some(Weekday::Sat),
        "Sun" => Some(Weekday::Sun),
        _ => None,
    }
}

pub fn weekday_of(date: NaiveDate) -> Weekday {
    date.weekday()
}
