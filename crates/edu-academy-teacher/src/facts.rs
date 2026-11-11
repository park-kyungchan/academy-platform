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

pub trait TeacherFacts {
    fn class(&self, name: &str) -> Option<ClassFact>;
    fn moved_lessons(&self, class_name: &str) -> &[MovedLesson];
}

/// Seeded implementation from the scenario file's `classes` section.
#[derive(Clone, Debug, Default)]
pub struct SeededFacts {
    classes: Vec<ClassFact>,
    moved: Vec<(String, Vec<MovedLesson>)>,
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
        Self { classes, moved }
    }
}

impl TeacherFacts for SeededFacts {
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
