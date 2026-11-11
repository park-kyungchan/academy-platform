//! The one code-to-label table for the two assessment fields (M10, M13).
//! `label` is the advertised Korean label, `wire_code` the LMS wire code,
//! and `aliases` other accepted teacher words for the same code. Attendance
//! labels are not in this table; they stay as advertised in the tool table.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssessmentLabel {
    pub field: &'static str,
    pub label: &'static str,
    pub wire_code: i32,
    pub aliases: &'static [&'static str],
}

pub static ASSESSMENT_LABELS: &[AssessmentLabel] = &[
    AssessmentLabel {
        field: "daily_test",
        label: "미확인",
        wire_code: -1,
        aliases: &[],
    },
    AssessmentLabel {
        field: "daily_test",
        label: "미실시",
        wire_code: 0,
        aliases: &[],
    },
    AssessmentLabel {
        field: "daily_test",
        label: "10점",
        wire_code: 1,
        aliases: &[],
    },
    AssessmentLabel {
        field: "daily_test",
        label: "20점",
        wire_code: 2,
        aliases: &[],
    },
    AssessmentLabel {
        field: "daily_test",
        label: "30점",
        wire_code: 3,
        aliases: &[],
    },
    AssessmentLabel {
        field: "daily_test",
        label: "40점",
        wire_code: 4,
        aliases: &[],
    },
    AssessmentLabel {
        field: "daily_test",
        label: "50점",
        wire_code: 5,
        aliases: &[],
    },
    AssessmentLabel {
        field: "daily_test",
        label: "60점",
        wire_code: 6,
        aliases: &[],
    },
    AssessmentLabel {
        field: "daily_test",
        label: "70점",
        wire_code: 7,
        aliases: &[],
    },
    AssessmentLabel {
        field: "daily_test",
        label: "80점",
        wire_code: 8,
        aliases: &[],
    },
    AssessmentLabel {
        field: "daily_test",
        label: "90점",
        wire_code: 9,
        aliases: &[],
    },
    AssessmentLabel {
        field: "daily_test",
        label: "100점",
        wire_code: 10,
        aliases: &[],
    },
    AssessmentLabel {
        field: "prev_homework",
        label: "미확인",
        wire_code: -1,
        aliases: &[],
    },
    AssessmentLabel {
        field: "prev_homework",
        label: "미숙제",
        wire_code: 0,
        aliases: &["미실시"],
    },
    AssessmentLabel {
        field: "prev_homework",
        label: "C",
        wire_code: 1,
        aliases: &[],
    },
    AssessmentLabel {
        field: "prev_homework",
        label: "B",
        wire_code: 2,
        aliases: &[],
    },
    AssessmentLabel {
        field: "prev_homework",
        label: "B+",
        wire_code: 3,
        aliases: &[],
    },
    AssessmentLabel {
        field: "prev_homework",
        label: "A",
        wire_code: 4,
        aliases: &[],
    },
    AssessmentLabel {
        field: "prev_homework",
        label: "A+",
        wire_code: 5,
        aliases: &[],
    },
];

/// Wire code for an advertised label or an accepted alias of `field`.
pub fn wire_code(field: &str, label: &str) -> Option<i32> {
    ASSESSMENT_LABELS
        .iter()
        .filter(|entry| entry.field == field)
        .find(|entry| entry.label == label || entry.aliases.contains(&label))
        .map(|entry| entry.wire_code)
}

/// Advertised label for a wire code of `field`.
pub fn label_for_code(field: &str, wire_code: i32) -> Option<&'static str> {
    ASSESSMENT_LABELS
        .iter()
        .find(|entry| entry.field == field && entry.wire_code == wire_code)
        .map(|entry| entry.label)
}
