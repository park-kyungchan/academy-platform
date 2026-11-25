//! Private Daechi class domain workspace skeleton.
//!
//! Planned modules (documentation only; no domain behavior is implemented):
//! - `owner`: private academy and instructor ownership.
//! - `scope`: class, student, lesson and date scope.
//! - `teacher_workflow`: teacher review and domain use cases.
//! - `preparation`: lesson preparation and readiness.
//! - `assessment`: assessment evidence and teacher judgments.
//! - `whole_lens`: whole-class and student learning views.
//! - `preclass`: preclass review and attention queues.
//! - `sheet_projection`: reviewed teacher-facing projections.
//! - `book_bindings`: source-bound textbook identities.
//! - `learning_policy`: private learning and carry-forward policy.
//! - `retained_query`: bounded queries of retained domain evidence.
//!
//! Public education-platform dependencies will be pinned in a later change.

#[cfg(test)]
mod tests {
    #[test]
    fn synthetic_fixture_smoke() {
        // Scaffold acceptance only; this does not exercise teacher behavior.
        let synthetic_students = ["synthetic-student-01", "synthetic-student-02"];
        assert_eq!(synthetic_students.len(), 2);
        assert_ne!(synthetic_students[0], synthetic_students[1]);
    }
}
