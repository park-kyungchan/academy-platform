# Academy MCP runtime-agnostic 문제 정의

작성: 2026-10-26. 범위: Muse·Codex·Hermes 세 runtime에서 같은 academy MCP 도구를 같은 규칙으로 쓰기 위한 구조 개선. 대규모 repo 작업이 진행 중이므로, 이 문서는 문제 정의와 수용 기준만 고정하고 구현은 단계별 소규모 PR로 분리한다.

## P1. 지시 귀속(attribution) 판정 기준 부재

- 정의: 저장 같은 effect는 "어느 runtime의, 어느 세션의, 어떤 사용자 지시"로 승인됐는지를 판정해야 하는데, 현재 판정이 runtime별 세션 로그 형식에 직접 묶여 있다. Codex용 실행분은 Codex rollout 파일을, Muse 실행분은 Muse session.jsonl을 읽는다. 공용 서버가 되면 이 판정을 runtime 중립 형식으로 다시 정의해야 한다.
- 증거 형식(제안): `{runtime, runtimeSessionId, requestRef, requestTextSha256, currentUserEventRef, issuedAt}`. runtime 어댑터는 자기 세션 저장소에서 이 envelope만 만들고, 내용 판정과 소비는 owner 서비스가 한다.
- 판정 규칙(제안): (1) requestRef가 실제 사용자 이벤트로 존재하고 텍스트 해시가 일치한다. (2) 저장 시점에 그 지시가 취소·변경되지 않았다(최신 이벤트 대조). (3) action digest 단위로 한 번만 소비한다. (4) 형식이 하나라도 다르면 거부하고 자동 재시도하지 않는다.
- 수용 기준: 가짜 fixture로 세 runtime envelope를 각각 만들어 owner 판정을 통과·거부하는 테스트가 있다. 실제 학생 저장에는 쓰지 않는다.
- 검토 반영(2026-10-26, platform-redesign Meta-Controller): (1) envelope에 만료(`expiresAt`)를 둔다. 만료된 envelope는 거부한다. (2) envelope는 `actionDigests`에 묶여 그 action들에만 쓰인다. 다른 action 재사용은 거부한다. (3) 취소·변경 대조는 어댑터 주장이 아니라 owner가 실제 이벤트를 조회한 결과여야 한다. (4) `requestTextSha256` 정규화 명세를 고정한다: 사용자 메시지 텍스트 블록을 순서대로 LF로 잇고, 앞뒤 공백은 그대로 두며, UTF-8로 해시한다. runtime마다 다른 정규화를 쓰면 해시가 달라져 거부된다.
- 경계 확인: platform-redesign P4 epoch 소스는 봉인 집합이라 건드리지 않는다. 공용 서버 추출은 education-platform 별도 branch 병행 작업으로 두고, platform-redesign 연결은 P5 결정 이후 새 admission 계약 버전 채택 지점에서만 한다.

## P2. runtime별 도구 노출 불일치

- 정의: Muse bridge가 등록 단계에서 DailyRecord·CISM·item-label 도구를 제외한다. 상류에 있어도 runtime마다 보이는 도구가 다르다.
- 수용 기준: 공용 서버 하나로 세 runtime의 도구 목록이 동일하다. 제외가 필요하면 서버 설정 한 곳에서만 선언한다.

## P3. backend 주입의 runtime 결합

- 정의: 읽기 backend 주입이 runtime별 실행분에 흩어져 있어, 주입이 없는 runtime에서는 도구가 있어도 unavailable이다.
- 수용 기준: backend는 owner 프로세스 한 곳에서 주입되고, runtime은 서버 연결만 한다. credential은 owner 밖으로 나오지 않는다.

## P4. 실패 latch 의미

- 정의: 현재 bridge는 endpoint 단위로 실패를 latch해서 한 번의 실패가 그 세션의 같은 endpoint 전체를 막는다. 의도는 자동 재시도 금지지만, 세션 전체 차단으로 번진다.
- 수용 기준: 실패는 요청 단위로 닫히고, 자동 재시도 금지는 유지하되 새 요청은 새 판정을 받는다. 불확실한 effect 결과는 readback으로만 해소한다.

## P5. 쓰기 경로의 날짜·runtime 고정

- 정의: 저장 worker와 control이 날짜·학생·runtime에 고정된 일회성 실행분으로 존재한다. 새 날짜마다 복제해야 하고, 복제 과정 자체가 위험하다.
- 수용 기준: 저장 도구가 날짜·학생을 입력으로 받는 공용 도구이며, 승인 소비·preflight·readback을 서버가 강제한다. 일회성 실행분은 fixture 검증 후 폐기한다.

## P6. 도구 이름 충돌

- 정의: Hermes 프로필 등 기존 runtime에 같은 이름의 도구가 이미 있을 수 있다. 공용 등록 전에 충돌을 목록화하지 않으면 어느 쪽이 실행되는지 모호해진다.
- 수용 기준: runtime별 기존 도구 목록과 공용 목록의 대조표가 있고, 충돌 시 우선순위가 문서로 정해져 있다.

## P7. CISM 범위 미지정

- 정의: CISM 주간보고서의 집계 기간·제출처·필수 항목·대상 학생이 교사 지정 전이라, 도구를 열어도 입력할 곳이 확정되지 않는다.
- 수용 기준: 이번 범위에서는 읽기(cism_section)와 근거팩까지만 다룬다. 제출 도구는 교사 지정이 온 뒤 별도 문제로 연다. DailyRecord 숫자로 주간 결론을 자동 생성하지 않는다.

## 진행 원칙

- 읽기 공용화(P2·P3·P4) → 귀속 판정(P1) → 쓰기 공용화(P5) 순서. P6은 등록 직전, P7은 교사 지정 후.
- 각 단계는 독립 PR로 하고, 대규모 작업 중인 repo에서는 문서·스키마처럼 충돌이 적은 파일부터 넣는다.
- 실제 학생 데이터로 새 경로를 시험하지 않는다. 검증은 fixture 우선, 실제 저장은 교사 지시가 있을 때만 기존 절차로 한다.
