# Checklist and status report formats

Fill every bracketed field from a labelled source, or leave the line out. Do not print an empty placeholder. All names, books and scores below are placeholders.

## 1. Detailed lesson checklist, one block per student

```text
======================================================================
 <학생> | <등원예정/상태> | 잔여리마인드: <OPEN 리마인더 또는 없음>
======================================================================
 [진도 게이지] <교재/대단원> [<12칸 바>] <관측/완료 현황> | DT/클리닉 요약
----------------------------------------------------------------------
 오늘 흐름: 등원 -> 숙제 확인 -> 이월 활동 -> 계획 활동(PLANNED) -> 예습 상태
----------------------------------------------------------------------
<교재> [S:서버 | C:클리닉/DT | T:교사판정]
├── [완료/이전 대단원] ── [S:... | C:... | T:...] (1줄 요약)
└── [진행 대단원] ◀◀ ACTIVE
    ├── <소단원> [예습/DT 요약]
    │   └── [<섹션>] [S:<점수(정답/전체), 오답번호> | C:<클리닉상태> | T:<교사판정>]
    └── ...
```

- `S` is what the server shows, with `observedAt`. `C` is the clinic or DailyTest record. `T` is the teacher's judgment: `TEACHER_CONFIRMED`, `UNCONFIRMED` or `PLANNED`.
- A DailyTest attempt shows its level (`기본`, `발전`), score and question count.
- A line meant for a DailyRecord uses the form `- <교재>, <단원> <활동>`.
- Put open reminders above the first student block.

## 2. Status report

```text
[YYYY-MM-DD 학원 수업 맥락 / 1부 <학생A>]

1. <학생A>
  1.1 당일 흐름
    - 15:00 출석, 기존 숙제 확인 완료: 당일 교사 보고.
    - 개념백지테스트 진행 → 교사 완료 확정.
    - <교재A> <단원> 오답클리닉: 진행 중 보고 후 당일 교사 완료로 갱신.
    - 기존 총괄평가와 당일 재도전 프린트가 같은 회차인지는 미확인.

  1.2 <교재A> / 현재 교재
    <단원 1>: 과거 응시 점수. 이전 수업의 오답·유사클리닉 교사 완료 유지.
    <단원 2>: 보강일 DT 2회 완료 보고는 그날 이력으로 보존.
    <단원 3>: 과거 숙제 미수행 → 당일 숙제 확인 완료 보고로 갱신.
    <대단원> 대단원총괄평가: 기존 응시와 당일 재도전을 따로 유지.
    <후속 단원>: 배정·수행 근거 미확인, 미이행 과제로 세지 않음.

  1.3 <교재B> / 이전 교재
    <단원>: 과거 오답클리닉 완료와 새 DT 숙제를 구분.

  1.4 새 숙제·지도 방향
    - 최종 새 숙제: <교재B> <단원> DT 기본·발전, 총 2회.
    - 처음 계획했던 활동 중 교사가 뺀 것은 제외로 적음.
    - 제출기한은 미명시. 지도계획 종료일을 제출기한으로 바꾸지 않음.
    - 오늘 확인할 것: <미확인 사항>.
```

Rules of this structure:
- Two books with the same unit code are different units: keep each under its own book.
- Keep each assessment kind apart: a test in the book, a printed chapter test from the LMS, an attainment test, a monthly test and a DailyTest.
- A planned unit with no evidence of assignment is not a missing assignment.
- A deadline the teacher did not state stays unstated.
- A one-unit update keeps the title and numbering and shows only `당일 변경`, the affected book and unit, and any changed homework.
