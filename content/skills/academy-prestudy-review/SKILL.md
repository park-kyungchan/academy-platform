---
name: academy-prestudy-review
description: "Use when reviewing a student's pre-study videos or reports (예습영상 검사, 예습 리포트, 가우스 예습): what to check, which tools read it, and how to report the result."
---

# Academy pre-study review

Before a Gauss minor unit, the student records a pre-study video (`예습영상`). The LMS keeps the upload and an AI report on it. The teacher checks the student's understanding in class and decides whether the pre-study is complete. This skill prepares that check: what the unit demands, what the AI report says against it, and what the teacher still has to confirm.

## 1. When a review applies

- A homework that starts a new Gauss minor unit (`소단원 시작`) needs a pre-study upload.
- Continuing a later section of the same minor unit needs no new upload, but it does not cancel an earlier missing one.
- Other book families (for example Davinci) are `NOT_APPLICABLE`.
- Re-recording is not a completion condition: never ask for one.

## 2. Read

1. `academy_prestudy_get` for the student, unless a reading of today is already in the day's `STATE.md`.
2. `academy_prestudy_report_get` only for the upload under review: the one waiting for the teacher's check, or the one the teacher names.

Read one tool call at a time. Use the tool descriptions for the arguments.

## 3. Keep the evidence channels apart

| Channel | What can be said |
| --- | --- |
| Upload | Waiting or completed, with its date, as the server shows it. An upload status of `OK` is intake telemetry, not the teacher's acceptance. |
| AI report | Its sections and text, with `observedAt`. Name each section by its title. |
| Transcript | `UNOBSERVED`: no tool returns it. Never call report prose a transcript. |
| Video and frames | Never watched by the agent. Every claim about the board or the speech is `report-attributed`. |
| Pre-study notebook | `UNCONFIRMED` until the teacher looks at it. |
| Teacher judgment | `UNCONFIRMED` or `TEACHER_CONFIRMED`; only the teacher's words change it. |

AI scores are AI scores. Never map an unlabelled score list to rubric items, and never turn an AI score into a grade, a pass or a completion threshold.

## 4. Build the unit's tree first

Before reading the AI report, build the whole minor unit's thought-action tree from the mathematics alone, so that the report cannot shrink it. Seven layers:

1. `P` prerequisites, each with why it is needed;
2. `D` definitions with their necessary and sufficient conditions (for example a non-zero coefficient, a restricted domain);
3. `R` representation translations, one per direction (table to formula, formula to graph, graph to formula …);
4. `S` covariation and invariants;
5. `M` boundaries, non-examples and misconceptions;
6. `A` observable thought-actions, each with one verb: Recognize, Distinguish, Check Applicability, Translate, Justify, Verify;
7. `Q` neutral diagnostic questions, each aimed at an `A` or `M` item.

The blueprint of the layers and the rules for splitting and stopping a node are in `/home/palantirkc/work/edu-lessons/math-pedagogy-research/` (`02-thought-action-tree-blueprint.md`, and section 3 of `01-cognitive-frameworks.md`). If several students share the same book, edition and unit, build one tree and put each student's evidence under it.

## 5. Map the report to the tree

- Bind each observation or omission of the AI report to an `A` or `M` ID: `AI_REPORTED`, `CONCERN` (missing or doubtful) or `UNOBSERVED`.
- Mark the points the teacher should confirm first.
- Write `Q` questions that are neutral: no worked answer, no model solution, no script for the teacher.

## 6. Report to the teacher

Write in Korean, in five parts, in this order. Keep field names and labels in their original form.

```text
======================================================================
 [가우스 예습 검사] <학생> | <교재> | <소단원>
======================================================================
(1) 교재·단원·숙제 범위
- 교재, 소단원, 숙제 범위, 새 업로드가 필요한지

(2) 증거 채널별 상태
- 업로드: <waiting/completed, 날짜>
- AI 리포트: <observedAt, 받은 섹션>
- 원본 transcript: UNOBSERVED
- 영상·프레임: 미시청 (report-attributed)
- 예습노트 실물: UNCONFIRMED
- 교사 완료판정: UNCONFIRMED / TEACHER_CONFIRMED
- 재촬영: 불필요

(3) 소단원 7계층 사고-행동 트리
<소단원>
├── [L1 선수 개념] P1, P2, ...
├── [L2 정의와 필요·충분조건] D1, D2, ...
├── [L3 방향별 표상 변환] R1(표->식), R2(식->그래프), ...
├── [L4 공변 추론·불변량] S1, ...
├── [L5 반례·오개념 경계] M1, M2, ...
└── [L6 관측 가능한 사고-행동] A1 [Recognize], A2 [Distinguish], ...

(4) 리포트 근거 매핑 · 우선 확인 포인트 · 중립 진단 질문
- A1: AI_REPORTED / CONCERN / UNOBSERVED - <리포트 섹션 제목>
- Q1 (대상 A2): <중립 질문>

(5) 교사 완료판정이 남은 범위
- 수업에서 교사가 확인한 뒤 완료를 판정할 정확한 범위
```

When the review is part of a lesson checklist, put this block under the student in the checklist of `academy-lesson-day`.
