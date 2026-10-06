import { createUIFixture } from '../../fixtures/ui.ts';
import { SyntheticApplicationService } from '../application/service.ts';
import { bindTrustedSession } from '../interfaces/ports.ts';
import { WorkbenchModel, reviewLabel } from './model.ts';
import type { StudentLessonDetail, SyntheticLesson } from '../contracts/lesson-views.ts';
import { createStudentRepository } from '../../fixtures/student-repository.ts';
import { bookCards, curriculumView } from './curriculum-view.ts';
import { historyView } from './history-view.ts';

const fixture = createUIFixture();
const service = new SyntheticApplicationService(fixture.snapshot, fixture.lessons, [], createStudentRepository());
const model = new WorkbenchModel(bindTrustedSession(service, { actorId: 'synthetic-teacher', role: 'teacher', academyIds: ['synthetic-academy'] }), fixture.lessons[0]!.context);
const root = document.querySelector<HTMLElement>('#app')!;
const escape = (value: unknown) => String(value).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]!));
const badge = (text: string, tone = '') => `<span class="badge ${tone}">${escape(text)}</span>`;
const at = (value: string) => `${value.slice(0, 10)} ${value.slice(11, 16)} UTC`;
const observation = (state: string) => state === 'observed' ? '관찰 기록 있음 · 완료 판정 아님' : state === 'unresolved' ? '미확인 · 근거 부족' : '아직 확인하지 않음';
let selected: StudentLessonDetail | null = null;
let notice = '';
let pendingFocus = '';

function shell(content: string) {
  return `<aside><a class="brand" href="#"><span class="brand-icon">수</span> 수업 노트<small>TEACHER WORKBENCH</small></a><p class="nav-label">수업 준비</p><a class="nav-item active" href="#">▦ <span>오늘 수업</span><span class="nav-count">3</span></a><a class="nav-item history-nav" href="#view=history&student=synthetic-student-1">◷ <span>학생 히스토리</span></a><div class="sidebar-note"><span class="dot"></span> 합성 데모 환경<p>가상 학생 3명으로<br>수업 흐름을 확인합니다.</p></div></aside>
  <div class="workspace"><header><span>나의 수업 공간 <b>/</b> 오늘 수업</span><span class="teacher"><i>교</i> 가상 강사</span></header><main><div class="demo-banner"><strong>합성 자료 전용</strong><span>실제 학생 정보가 아닙니다. 기록은 메모리에만 남으며 새로고침하면 초기화됩니다.</span></div>${content}<p class="footnote">기준 날짜 2026-10-06 · 학원 시간대 Asia/Seoul · 원본 사이트 시간대 미확인</p></main></div><div class="sr-only" aria-live="polite">${escape(notice)}</div>`;
}
function overview() {
  const result = model.overview();
  if (!result.ok) return `<h1>수업을 불러오지 못했습니다</h1><p>${escape(result.code)}</p>`;
  return `<div class="page-heading"><div><p class="eyebrow">수업을 시작하기 전에</p><h1>오늘 수업</h1><p class="subtitle">학생별 준비 자료와 확인할 일을 한눈에 살펴보세요.</p></div><div class="date-chip">10월 6일 <span>화요일 · 데모 기준일</span></div></div>
  <section class="metrics" aria-label="오늘 수업 요약"><div><span>수업 예정</span><strong>3 <small>명</small></strong></div><div><span>개별 확인 필요</span><strong>3 <small>명</small></strong></div><div><span>강사 판단 기록</span><strong>${model.decisions.size} <small>건</small></strong></div><div class="metric-note"><span>확인 원칙</span><p>근거가 부족하면<br><b>미확인으로 남깁니다.</b></p></div></section>
  <section class="panel"><div class="section-title"><h2>오늘의 학생 <span>3</span></h2><span>수업 시간순</span></div><div class="table-wrap"><table><thead><tr><th>시간</th><th>학생</th><th>확인할 일</th><th>준비 자료</th><th>강사 판단</th><th><span class="sr-only">상세</span></th></tr></thead><tbody>${result.data.rows.map((row, i) => `<tr><td class="time">${escape(row.localStartTime)}</td><td><a class="student-link" href="#student=${encodeURIComponent(row.studentId)}"><span class="avatar a${i}">${['별','달','해'][i]}</span>${escape(row.displayName)}</a><span class="student-sub">합성 수학 · 개별 수업</span></td><td>${badge('확인 필요','amber')}<p class="cell-copy">${escape(row.questionsToCheck.join(', '))}</p></td><td>${escape(row.preparation.join(', '))}<p class="cell-copy">합성 근거 ${row.evidenceIds.length}건</p></td><td>${badge(model.decisions.get(row.studentId) ?? '아직 판단 없음')}</td><td><a class="detail-link" aria-label="${escape(row.displayName)} 상세 보기" href="#student=${encodeURIComponent(row.studentId)}">상세 보기 →</a></td></tr>`).join('')}</tbody></table></div></section>
  <div class="bottom-note"><span>ⓘ</span><p>영상 제출, 문제 풀이, 종이 확인은 서로 다른 확인 항목입니다.<br>이전 점수나 부분 조회만으로 새 과제를 완료 처리하지 않습니다.</p></div>`;
}
function lessonCard(lesson: SyntheticLesson, detail: StudentLessonDetail) {
  const videos = detail.videoRequirements.filter(r => r.assignmentId === lesson.assignmentId);
  const problems = detail.problemRequirements.filter(r => r.assignmentId === lesson.assignmentId);
  return `<article class="panel assignment"><div class="section-title"><div><p class="eyebrow">배정 과제 · ${escape(lesson.assignmentId)}</p><h2>${escape(lesson.material.unit)}</h2></div>${badge(reviewLabel(lesson),'amber')}</div>
  <div class="requirement-grid"><section class="requirement"><span class="requirement-icon">▷</span><h3>예습영상 요구</h3>${videos.map(v => `<p class="requirement-main">${Math.ceil(v.requiredSeconds / 60)}분 시청 요구</p><p>${escape(lesson.material.videoVersion)}</p>`).join('') || '<p>요구 정보 미확인</p>'}${badge('제출 미확인','amber')}<p class="muted">시청 요구와 실제 제출 확인은 별개입니다.</p></section><section class="requirement"><span class="requirement-icon">▤</span><h3>문제풀이 섹션 요구</h3>${problems.map(p => `<p class="requirement-main">${escape(lesson.material.section)} · ${p.requiredProblemCount}문항</p><p>${lesson.material.problemRange.first}–${lesson.material.problemRange.last}번 배정</p>`).join('') || '<p>요구 정보 미확인</p>'}${badge('풀이 미확인','amber')}<p class="muted">앱 기록과 종이 풀이를 각각 확인합니다.</p></section></div>
  <dl class="observations"><div><dt>과제 배정</dt><dd>${observation(lesson.observations.assignmentConfirmed)}</dd></div><div><dt>앱 점수</dt><dd>${observation(lesson.observations.appScoreObserved)}</dd></div><div><dt>영상 제출</dt><dd>${observation(lesson.observations.videoSubmitted)}</dd></div><div><dt>교사 검사</dt><dd>${observation(lesson.observations.teacherInspectionObserved)}</dd></div><div><dt>종이 풀이</dt><dd>${observation(lesson.observations.paperWorkConfirmed)}</dd></div></dl>
  <details><summary>배정 범위 · 판본 · 조회 시각 · 근거 · 미확인 사유 펼치기</summary><div class="provenance"><dl><dt>배정 범위</dt><dd>${escape(lesson.material.unit)} / ${escape(lesson.material.section)} / ${lesson.material.problemRange.first}–${lesson.material.problemRange.last}번</dd><dt>교재 판본</dt><dd>${escape(lesson.material.textbookEdition)}</dd><dt>영상 버전</dt><dd>${escape(lesson.material.videoVersion)} · 교재 판본과 별도</dd><dt>배정 시각</dt><dd>${escape(lesson.reviewContext ? at(lesson.reviewContext.assignedAt) : '미확인')}</dd><dt>조회 시각</dt><dd>${escape(lesson.reviewContext ? at(lesson.reviewContext.observedAt) : '미확인')}</dd><dt>조회 범위</dt><dd>${lesson.reviewContext?.coverage === 'all' ? '합성 전체 범위' : '부분 조회 · 전체 분모 미확인'}</dd><dt>근거</dt><dd>${detail.evidence.filter(e => lesson.evidenceIds.includes(e.id)).map(e => `${escape(e.id)} · ${escape(e.sourceReference)} · ${escape(at(e.observedAt))}`).join('<br>') || '연결된 근거 없음'}</dd></dl><div class="reason"><strong>미확인 사유</strong><ul>${(lesson.reviewContext?.reasons ?? ['조회 정보가 없습니다.']).map(r => `<li>${escape(r)}</li>`).join('')}</ul></div></div></details></article>`;
}
function detailView(detail: StudentLessonDetail) {
  return `<a class="back-link" href="#">← 오늘 수업으로</a><div class="page-heading"><div><p class="eyebrow">학생 상세 · 합성 자료</p><h1>${escape(detail.displayName)}</h1><p class="subtitle">${escape(detail.lessons[0]?.localStartTime ?? '')} 수업 · 배정 과제와 확인 근거</p></div><label class="switcher">학생 전환<select id="student-switch" aria-label="학생 전환">${['별','달','해'].map((n, i) => `<option value="synthetic-student-${i + 1}" ${detail.studentId === `synthetic-student-${i + 1}` ? 'selected' : ''}>가상 학생 ${n}</option>`).join('')}</select></label></div>
  ${bookCards(detail.books, detail.studentId)}<div class="detail-layout"><div>${detail.lessons.map(l => lessonCard(l, detail)).join('')}<section class="panel history"><h2>이전 과제 이력</h2>${detail.lessons.flatMap(l => l.history).map(h => `<p>${escape(h.note)}</p>`).join('')}<span class="muted">과거 점수는 이번 배정의 완료 근거로 사용하지 않습니다.</span></section></div><div><section class="panel review"><p class="eyebrow">강사 확인</p><h2>직접 확인하고 판단하기</h2><p>관찰 상태를 확인한 뒤 강사가 명시적으로 기록합니다. 판단을 자동 생성하지 않습니다.</p><div class="decision-status">${escape(model.decisions.get(detail.studentId) ?? '아직 강사 판단이 없습니다.')}</div><button class="primary" id="review-open" ${model.decisions.has(detail.studentId) || !detail.candidates.length ? 'disabled' : ''}>강사 판단 검토</button><p class="muted">이 브라우저 메모리에만 기록됩니다.<br>새로고침하면 사라집니다.</p></section><section class="panel preparation"><h2>수업 준비</h2><ul>${detail.lessons.flatMap(l => l.preparation).map(p => `<li>${escape(p)}</li>`).join('')}</ul></section></div></div>`;
}
function render() {
  model.cancel(); selected = null;
  const params = new URLSearchParams(location.hash.slice(1));
  const isHistory = params.get('view') === 'history';
  const id = params.get('student') ?? (isHistory ? 'synthetic-student-1' : null);
  const result = model.select(id);
  if (id && result?.ok) selected = result.data;
  const book = selected?.books.find(b => b.book.id === params.get('book'));
  root.innerHTML = shell(id ? selected ? isHistory ? historyView(selected, params) : params.has('book') ? book ? curriculumView(book, selected.displayName, params) : '<h1>교재를 찾을 수 없습니다</h1><a href="#">오늘 수업으로</a>' : detailView(selected) : '<h1>학생을 찾을 수 없습니다</h1><a href="#">오늘 수업으로 돌아가기</a>' : overview());
  document.title = selected ? `${selected.displayName} · 수업 노트` : '오늘 수업 · 수업 노트';
  document.querySelector<HTMLSelectElement>('#student-switch')?.addEventListener('change', e => { location.hash = `student=${encodeURIComponent((e.target as HTMLSelectElement).value)}`; });
  document.querySelector('#review-open')?.addEventListener('click', openReview);
  const historyNav = document.querySelector<HTMLAnchorElement>('.history-nav');
  const todayNav = document.querySelector<HTMLAnchorElement>('.nav-item:not(.history-nav)');
  historyNav?.classList.toggle('active',isHistory); todayNav?.classList.toggle('active',!isHistory);
  if(historyNav) historyNav.href = `#view=history&student=${encodeURIComponent(id ?? 'synthetic-student-1')}`;
  if(isHistory) { historyNav?.setAttribute('aria-current','page'); document.title=`${selected?.displayName ?? ''} 히스토리 · 수업 노트`; }
  const breadcrumb = document.querySelector('header > span:first-child');
  if(breadcrumb) breadcrumb.textContent = `나의 수업 공간 / ${isHistory ? '학생 히스토리' : book ? '교재 진행 지도' : selected ? '학생 상세' : '오늘 수업'}`;
  const changeRoute = (patch: Record<string, string | null>, focus = '') => {
    const next = new URLSearchParams(params); for (const [key,value] of Object.entries(patch)) value === null ? next.delete(key) : next.set(key,value);
    pendingFocus = focus; location.hash = next.toString();
  };
  document.querySelector('#progress-filter')?.addEventListener('change', e => changeRoute({filter:(e.target as HTMLSelectElement).value, section:null}, 'progress-filter'));
  document.querySelector('#major-filter')?.addEventListener('change', e => changeRoute({major:(e.target as HTMLSelectElement).value, section:null}, 'major-filter'));
  document.querySelector('#map-zoom')?.addEventListener('click', () => changeRoute({zoom:params.get('zoom')==='1'?null:'1'},'map-zoom'));
  document.querySelector('#history-student')?.addEventListener('change', e => changeRoute({student:(e.target as HTMLSelectElement).value,historyFilter:null},'history-student'));
  document.querySelector('#history-filter')?.addEventListener('change', e => changeRoute({historyFilter:(e.target as HTMLSelectElement).value},'history-filter'));
  if(pendingFocus) { document.getElementById(pendingFocus)?.focus(); pendingFocus=''; }
  else if(params.has('section')) document.getElementById('section-evidence')?.focus();
}
function openReview() {
  if (!selected || document.querySelector('dialog')) return;
  const result = model.preview(selected.candidates[0]!.id);
  if (!result.ok) { notice = `검토할 수 없습니다: ${result.code}`; render(); return; }
  const dialog = document.createElement('dialog');
  dialog.setAttribute('aria-labelledby','review-title');
  dialog.innerHTML = `<form id="review-form"><p class="eyebrow">명시적 강사 판단 · 합성 데모</p><h2 id="review-title">${escape(selected.displayName)} 판단 검토</h2><p>이 기록은 과제 완료 상태를 변경하지 않습니다.</p><label>판단<select name="decision"><option value="accept">클리닉 후보 채택</option><option value="dismiss">클리닉 후보 제외</option></select></label><label>판단 사유<textarea name="rationale" required maxlength="2000" placeholder="확인한 내용과 판단 사유를 입력하세요."></textarea></label><p class="muted">미리보기 데이터 revision ${result.data.dataRevision} · 새로고침 시 초기화</p><p id="review-error" role="alert"></p><div class="dialog-actions"><button type="button" id="review-cancel">취소</button><button type="submit" class="primary">메모리에 판단 기록</button></div></form>`;
  document.body.append(dialog);
  const cancel = () => { model.cancel(); dialog.close(); dialog.remove(); document.querySelector<HTMLButtonElement>('#review-open')?.focus(); };
  dialog.addEventListener('cancel', e => { e.preventDefault(); cancel(); });
  dialog.querySelector('#review-cancel')!.addEventListener('click', cancel);
  dialog.querySelector('form')!.addEventListener('submit', e => {
    e.preventDefault();
    const form = new FormData(e.target as HTMLFormElement);
    const answer = model.confirm(form.get('decision') === 'accept' ? 'accept' : 'dismiss', String(form.get('rationale') ?? ''));
    if (answer?.ok) { dialog.close(); dialog.remove(); notice = '강사 판단을 이 브라우저 메모리에 기록했습니다.'; render(); }
    else dialog.querySelector('#review-error')!.textContent = answer ? `기록하지 못했습니다 (${answer.code}). 취소 후 새로 검토하세요.` : '판단 사유를 입력하세요.';
  });
  dialog.showModal();
}
window.addEventListener('hashchange', () => { document.querySelector('dialog')?.remove(); render(); if(new URLSearchParams(location.hash.slice(1)).has('section')) document.getElementById('section-evidence')?.scrollIntoView({block:'start'}); else window.scrollTo(0, 0); });
render();
