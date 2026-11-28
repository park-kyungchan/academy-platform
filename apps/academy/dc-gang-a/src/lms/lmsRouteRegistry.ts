/**
 * LMS Full Route Tree Map & Endpoint Schema Registry.
 * 
 * Safety & Invariants (AGENTS.md & Phase 2 Harness Optimization Compliance):
 * 1. Single Source of Truth (SSoT): Dynamically loads and validates canonical
 *    LMS operations from research/backend-map/route-registry.json.
 * 2. Cwd-Independent Path Resolution: Anchored strictly via resolveModuleDir() / import.meta.dir.
 * 3. HTTP Method Safety Fallacy Guard (TRAP 1): NEVER rely on httpMethod === 'GET' for read safety!
 *    Mutating GET endpoints (day_record_udtprg, day_record_udthw, day_record_udtmemo,
 *    day_record_udtattn, similar_paper_create) are strictly blocked by assertSafeRead().
 * 4. Fail-Closed: assertSafeRead() throws UnsafeMutatingOperationError for any endpoint
 *    where semantic_effect !== 'read' or safe_to_probe !== true.
 * 5. Dual-Runtime Parity: 1:1 structural and behavioral parity with harness/routes.py.
 * 6. Legacy route declarations are historical source candidates only. Public lookup,
 *    safety checks, and request construction use the canonical registry exclusively.
 */

import { readFileSync, existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { REPORT_VIEW_NAMES } from './reportViewRead';

export class UnsafeMutatingOperationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'UnsafeMutatingOperationError';
    Object.setPrototypeOf(this, UnsafeMutatingOperationError.prototype);
  }
}

export class RouteLookupError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'RouteLookupError';
    Object.setPrototypeOf(this, RouteLookupError.prototype);
  }
}

export type LmsEndpointEffect =
  | 'READ_ONLY'
  | 'MUTATING'
  | 'TYPESET_READ'
  | 'read'
  | 'write'
  | 'send'
  | 'open_send_screen'
  | 'unknown';

export interface LmsRouteDefinition {
  // Canonical backend-map fields
  readonly id: string;
  readonly routeTemplate: string;
  readonly operation: string;
  readonly httpMethod: 'GET' | 'POST' | null;
  readonly semanticEffect: 'read' | 'write' | 'send' | 'open_send_screen' | 'unknown';
  readonly evidenceGrade: string;
  readonly evidenceDate: string;
  readonly joinKeyNames: readonly string[];
  readonly safeToProbe: boolean;
  readonly reason: string;
  readonly sourceRefs: readonly string[];

  // Snake_case aliases matching route-registry.json schema directly
  readonly route_template: string;
  readonly http_method: 'GET' | 'POST' | null;
  readonly semantic_effect: 'read' | 'write' | 'send' | 'open_send_screen' | 'unknown';
  readonly evidence_grade: string;
  readonly evidence_date: string;
  readonly join_key_names: readonly string[];
  readonly safe_to_probe: boolean;
  readonly source_refs: readonly string[];

  // Legacy helper and compatibility fields
  readonly routeId: string;
  readonly category: string;
  readonly servletPath: string;
  readonly processName: string;
  readonly effect: LmsEndpointEffect;
  readonly description: string;
  readonly requiredParams: readonly string[];
  readonly optionalParams: readonly string[];
  readonly responseFormat: 'HTML' | 'JSON' | 'EXTERN_DIALOG_HTML';
}

interface RawRegistryEntry {
  id: string;
  route_template: string;
  operation: string;
  http_method: string | null;
  semantic_effect: string;
  evidence_grade: string;
  evidence_date: string;
  join_key_names: string[];
  safe_to_probe: boolean;
  reason: string;
  source_refs: string[];
}

interface RawRegistryData {
  schema_version: string;
  as_of: string;
  scope: string;
  evidence_legend?: Record<string, string>;
  entries: RawRegistryEntry[];
}

/** Validate the entire catalog before populating lookup state. Cardinality is not a schema. */
export function validateCanonicalRegistry(raw: unknown): RawRegistryData {
  const value = raw as RawRegistryData | null;
  if (!value || !Array.isArray(value.entries) || value.entries.length === 0) {
    throw new Error('invalid_route_registry_entries');
  }
  const ids = new Set<string>();
  const text = (x: unknown): x is string => typeof x === 'string' && x.trim().length > 0;
  const strings = (x: unknown): x is string[] => Array.isArray(x) && x.every(text);
  for (const entry of value.entries) {
    if (!entry || !text(entry.id) || ids.has(entry.id)
      || !text(entry.operation) || !text(entry.route_template)
      || !['GET', 'POST', null].includes(entry.http_method)
      || !['read', 'write', 'send', 'open_send_screen', 'unknown'].includes(entry.semantic_effect)
      || typeof entry.safe_to_probe !== 'boolean'
      || (entry.safe_to_probe && entry.semantic_effect !== 'read')
      || !strings(entry.join_key_names) || !strings(entry.source_refs)
      || !text(entry.evidence_grade) || !text(entry.evidence_date) || !text(entry.reason)) {
      // Do not echo raw entries: future source metadata may contain sensitive values.
      throw new Error('invalid_or_duplicate_route_registry_entry');
    }
    ids.add(entry.id);
  }
  return structuredClone(value);
}

/**
 * Robust cross-runtime directory resolution.
 * Anchors paths to the physical file location, immune to process.cwd() drift.
 */
function resolveModuleDir(): string {
  if (typeof import.meta !== 'undefined' && import.meta.dir) {
    return import.meta.dir;
  }
  if (typeof import.meta !== 'undefined' && import.meta.url) {
    return dirname(fileURLToPath(import.meta.url));
  }
  return __dirname;
}

export const ROUTE_REGISTRY_PATH = resolve(
  resolveModuleDir(),
  '../../research/backend-map/route-registry.json'
);

/**
 * The six read-only report views of ReportViewServlet (specs/academy-mcp/direct-key-lookup.md R8):
 * each is a read only as `POST /servlet/controller.common.ReportViewServlet` with `p_process=<View>`
 * in the form body (StudyLink.exe 0x4692e0); any other view name, or another method, is not classed
 * read. Their form bodies are pinned in reportViewClient.ts.
 */
function reportViewLegacyRoute(view: string): [string, LmsRouteDefinition] {
  const id = `REPORT_VIEW_${view.replace(/([a-z])([A-Z])/g, '$1_$2').toUpperCase()}`;
  const path = '/servlet/controller.common.ReportViewServlet';
  const reason = `Official report view ${view} of one exact record (POST form p_process=${view}; read; changes no data)`;
  const keys = view === 'NewStudentReportView' ? ['p_p_no', 'p_apply_code', 'clg_no']
    : view === 'DiagReportView' || view === 'FaView' ? ['p_testing_no', 'p_pri_no', 'p_cls_no', 'p_limit_date'] : [];
  return [id, {
    id, routeId: id, category: 'ReportView', servletPath: path, processName: view,
    httpMethod: 'POST', http_method: 'POST',
    routeTemplate: `${path}?p_process=${view}`, route_template: `${path}?p_process=${view}`,
    operation: view, semanticEffect: 'read', semantic_effect: 'read', effect: 'READ_ONLY',
    description: reason, reason,
    evidenceGrade: 'decompiled-client', evidence_grade: 'decompiled-client',
    evidenceDate: '2026-11-28', evidence_date: '2026-11-28',
    joinKeyNames: Object.freeze([...keys]), join_key_names: Object.freeze([...keys]),
    sourceRefs: Object.freeze(['specs/academy-mcp/direct-key-lookup.md#R8']), source_refs: Object.freeze(['specs/academy-mcp/direct-key-lookup.md#R8']),
    safeToProbe: true, safe_to_probe: true,
    requiredParams: ['p_process', ...keys.filter(k => k !== 'p_limit_date')], optionalParams: keys.includes('p_limit_date') ? ['p_limit_date'] : [],
    responseFormat: 'HTML',
  }];
}

/**
 * The thirteen read routes of specs/academy-mcp/lms-reads.md: the schedule pages of `academy_lms_schedule_get`
 * (AttendanceServlet Main, CourseManageServlet StudySchedule, its UserSearch search POST and the CourseStudyManager
 * search GET), the prestudy pages
 * of `academy_prestudy_get` (WebUnPreStudy, WebCtPreStudy, TeacherPrestudySummary, WebPreStudy and the AI
 * report page) and the four test-result pages of view `test_results` (the search POST of TestPageListServlet
 * `UserBySearchTestResult` for ass_no 1000, 1001 and 1002, and the DiagnostManageServlet `TestResultList` GET).
 * Each is strictly read: a GET, or the POST of the list's own search form, which changes no
 * Academy data. The class is by the named route and operation, never by the HTTP method.
 */
const LMS_READ_ROUTES: readonly {id: string; path: string; param: 'p_process' | 'reqCmd' | null; operation: string; method: 'GET' | 'POST'; query?: string; reason: string; keys: readonly string[]}[] = [
  {id: 'LMS_ATTENDANCE_MAIN', path: '/servlet/controller.cct.tutor.AttendanceServlet', param: 'p_process', operation: 'Main', method: 'GET',
    reason: 'Attendance page of the teacher, one month per GET (read; changes no data)', keys: ['ym', 'sdate', 'grp_seq', 'stu_obj']},
  {id: 'LMS_STUDY_SCHEDULE', path: '/servlet/controller.coursemanage.CourseManageServlet', param: 'reqCmd', operation: 'StudySchedule', method: 'GET',
    reason: 'Study schedule page (read; changes no data)', keys: ['grade_no', 'cls_no', 'student_no', 'course']},
  {id: 'LMS_STUDY_SCHEDULE_SEARCH', path: '/servlet/controller.coursemanage.CourseManageServlet', param: 'reqCmd', operation: 'UserSearch', method: 'POST',
    reason: 'Study schedule search: the POST of the page\'s own onSearch form; it changes no data (read)', keys: ['grade_no', 'cls_no', 'student_no', 'course']},
  {id: 'LMS_COURSE_STUDY_MANAGER', path: '/servlet/controller.coursemanage.CourseManageServlet', param: 'reqCmd', operation: 'CourseStudyManager', method: 'GET', query: 'search=1',
    reason: 'Course study manager search: the GET of the page\'s own search=1 (read; changes no data)', keys: ['grade_no', 'semester_no', 'level_no', 'status', 'search']},
  {id: 'LMS_PRESTUDY_WAITING', path: '/servlet/controller.coursemanage.CourseManageServlet', param: 'reqCmd', operation: 'WebUnPreStudy', method: 'GET',
    reason: 'Prestudy waiting list; its form1 POST is the list search and changes no data (read)', keys: ['g_clg_no', 'g_mem_type', 'g_pri_no', 'student_name']},
  {id: 'LMS_PRESTUDY_COMPLETED', path: '/servlet/controller.coursemanage.CourseManageServlet', param: 'reqCmd', operation: 'WebCtPreStudy', method: 'GET',
    reason: 'Prestudy completed list; its form1 POST is the list search and changes no data (read)', keys: ['g_clg_no', 'g_mem_type', 'g_pri_no', 'student_name']},
  {id: 'LMS_PRESTUDY_SUMMARY', path: '/servlet/controller.coursemanage.CourseManageServlet', param: 'reqCmd', operation: 'TeacherPrestudySummary', method: 'GET',
    reason: 'Teacher prestudy summary page (read; changes no data)', keys: []},
  {id: 'LMS_PRESTUDY_DETAIL', path: '/servlet/controller.coursemanage.CourseManageServlet', param: 'reqCmd', operation: 'WebPreStudy', method: 'GET',
    reason: 'One uploaded prestudy video record (read; changes no data)', keys: ['prestudy_key']},
  {id: 'LMS_PRESTUDY_AI_REPORT', path: '/app-ui/solution_common/prestudy/prestudy_ai_report.jsp', param: null, operation: 'prestudy_ai_report', method: 'GET',
    reason: 'Server-rendered AI report of one prestudy video (read; changes no data)', keys: ['analysis_key']},
  {id: 'LMS_TEST_RESULTS_DA', path: '/servlet/controller.tutor.base.TestPageListServlet', param: 'p_process', operation: 'UserBySearchTestResult', method: 'POST', query: 'ass_no=1000',
    reason: 'Unit-test (DA) results of one student: the POST of the page\'s own search form; it changes no data (read)', keys: ['clg_no', 'cls_no', 'p_pageno', 'check_da_test', 'stu_name', 'chioce_cls', 'sort_date1', 'sort_date2']},
  {id: 'LMS_TEST_RESULTS_FA', path: '/servlet/controller.tutor.base.TestPageListServlet', param: 'p_process', operation: 'UserBySearchTestResult', method: 'POST', query: 'ass_no=1001',
    reason: 'Formative (FA) results of one student: the POST of the page\'s own search form; it changes no data (read)', keys: ['clg_no', 'cls_no', 'p_pageno', 'check_fa_test', 'stu_name', 'chioce_cls', 'sort_date1', 'sort_date2']},
  {id: 'LMS_TEST_RESULTS_NA', path: '/servlet/controller.tutor.base.TestPageListServlet', param: 'p_process', operation: 'UserBySearchTestResult', method: 'POST', query: 'ass_no=1002',
    reason: 'NA results of one student: the POST of the page\'s own search form; it changes no data (read)', keys: ['clg_no', 'cls_no', 'p_pageno', 'check_na_test', 'stu_name', 'chioce_cls', 'sort_date1', 'sort_date2']},
  {id: 'LMS_TEST_RESULTS_NEW_DIAG', path: '/servlet/controller.tutor.diag.DiagnostManageServlet', param: 'reqCmd', operation: 'TestResultList', method: 'GET',
    reason: 'New-student diagnosis results list (read; changes no data); its sortDate select is the list filter', keys: ['sortDate']},
];
function lmsReadLegacyRoute(def: typeof LMS_READ_ROUTES[number]): [string, LmsRouteDefinition] {
  const template = def.param ? `${def.path}?${def.param}=${def.operation}${def.query ? '&' + def.query : ''}` : def.path;
  return [def.id, {
    id: def.id, routeId: def.id, category: def.path.includes('coursemanage') || def.path.includes('prestudy') ? 'CourseManageIndex' : 'BaseManageIndex',
    servletPath: def.path, processName: def.operation,
    httpMethod: def.method, http_method: def.method,
    routeTemplate: template, route_template: template,
    operation: def.operation, semanticEffect: 'read', semantic_effect: 'read', effect: 'READ_ONLY',
    description: def.reason, reason: def.reason,
    evidenceGrade: 'live-route', evidence_grade: 'live-route',
    evidenceDate: '2026-11-28', evidence_date: '2026-11-28',
    joinKeyNames: Object.freeze([...def.keys]), join_key_names: Object.freeze([...def.keys]),
    sourceRefs: Object.freeze(['specs/academy-mcp/lms-reads.md']), source_refs: Object.freeze(['specs/academy-mcp/lms-reads.md']),
    safeToProbe: true, safe_to_probe: true,
    requiredParams: [], optionalParams: [...def.keys],
    responseFormat: 'HTML',
  }];
}

export class LmsRouteRegistry {
  private static _initialized = false;
  private static readonly canonicalRoutes: LmsRouteDefinition[] = [];
  private static readonly routesById: Map<string, LmsRouteDefinition> = new Map();
  private static readonly routesByOperation: Map<string, LmsRouteDefinition> = new Map();

  // Historical source candidates. Never expose these through public route APIs.
  private static readonly legacyRoutes: Map<string, LmsRouteDefinition> = new Map([
    [
      'FA_USER_SEARCH_RESULT',
      {
        id: 'FA_USER_SEARCH_RESULT',
        routeId: 'FA_USER_SEARCH_RESULT',
        category: 'FAIndex',
        servletPath: '/servlet/controller.tutor.base.TestPageListServlet',
        processName: 'UserBySearchTestResult',
        httpMethod: 'POST',
        http_method: 'POST',
        routeTemplate: '/servlet/controller.tutor.base.TestPageListServlet?p_process=UserBySearchTestResult',
        route_template: '/servlet/controller.tutor.base.TestPageListServlet?p_process=UserBySearchTestResult',
        operation: 'UserBySearchTestResult',
        semanticEffect: 'read',
        semantic_effect: 'read',
        effect: 'READ_ONLY',
        description: '학생별 기간/단원 총괄평가 응시 결과 목록 조회 (EXTERN_DIALOG 반환)',
        reason: '학생별 기간/단원 총괄평가 응시 결과 목록 조회 (EXTERN_DIALOG 반환)',
        evidenceGrade: 'live-route',
        evidence_grade: 'live-route',
        evidenceDate: '2026-09-27',
        evidence_date: '2026-09-27',
        joinKeyNames: Object.freeze(['clg_no', 'cls_no', 'stu_name', 'sort_date1', 'sort_date2']),
        join_key_names: Object.freeze(['clg_no', 'cls_no', 'stu_name', 'sort_date1', 'sort_date2']),
        sourceRefs: Object.freeze([]),
        source_refs: Object.freeze([]),
        safeToProbe: true,
        safe_to_probe: true,
        requiredParams: ['clg_no', 'cls_no', 'stu_name', 'sort_date1', 'sort_date2'],
        optionalParams: ['check_fa_test', 'p_pageno', 'checkAllPage'],
        responseFormat: 'EXTERN_DIALOG_HTML'
      }
    ],
    [
      'FA_PUPIL_SEARCH',
      {
        id: 'FA_PUPIL_SEARCH',
        routeId: 'FA_PUPIL_SEARCH',
        category: 'FAIndex',
        servletPath: '/servlet/controller.tutor.fa.TestPageListServlet',
        processName: 'PupilSearch',
        httpMethod: 'GET',
        http_method: 'GET',
        routeTemplate: '/servlet/controller.tutor.fa.TestPageListServlet?p_process=PupilSearch',
        route_template: '/servlet/controller.tutor.fa.TestPageListServlet?p_process=PupilSearch',
        operation: 'PupilSearch',
        semanticEffect: 'read',
        semantic_effect: 'read',
        effect: 'READ_ONLY',
        description: '시험지 문항별 채점 결과 및 학생 답안 상세 조회',
        reason: '시험지 문항별 채점 결과 및 학생 답안 상세 조회',
        evidenceGrade: 'live-route',
        evidence_grade: 'live-route',
        evidenceDate: '2026-09-27',
        evidence_date: '2026-09-27',
        joinKeyNames: Object.freeze(['testing_no', 'p_no', 'o_questions']),
        join_key_names: Object.freeze(['testing_no', 'p_no', 'o_questions']),
        sourceRefs: Object.freeze([]),
        source_refs: Object.freeze([]),
        safeToProbe: true,
        safe_to_probe: true,
        requiredParams: ['testing_no', 'p_no', 'o_questions'],
        optionalParams: ['apply_flg', 'no_apply_flg', 'paper_type', 'testing_name', 'grade_name', 'subject_name', 'hang_date', 'use_term'],
        responseFormat: 'HTML'
      }
    ],
    [
      'TYPESET_STUDY_RESULT_SINGLE',
      {
        id: 'TYPESET_STUDY_RESULT_SINGLE',
        routeId: 'TYPESET_STUDY_RESULT_SINGLE',
        category: 'FAIndex',
        servletPath: '/servlet/controller.common.TestpageSelectExServlet',
        processName: 'getStudyResultSingleTestingSingleUser',
        httpMethod: 'POST',
        http_method: 'POST',
        routeTemplate: '/servlet/controller.common.TestpageSelectExServlet?p_process=getStudyResultSingleTestingSingleUser',
        route_template: '/servlet/controller.common.TestpageSelectExServlet?p_process=getStudyResultSingleTestingSingleUser',
        operation: 'getStudyResultSingleTestingSingleUser',
        semanticEffect: 'read',
        semantic_effect: 'read',
        effect: 'TYPESET_READ',
        description: '오답 클리닉지 및 유형별 유사문제 클리닉지 조판/인쇄 데이터 생성',
        reason: '오답 클리닉지 및 유형별 유사문제 클리닉지 조판/인쇄 데이터 생성',
        evidenceGrade: 'live-route',
        evidence_grade: 'live-route',
        evidenceDate: '2026-09-27',
        evidence_date: '2026-09-27',
        joinKeyNames: Object.freeze(['condition']),
        join_key_names: Object.freeze(['condition']),
        sourceRefs: Object.freeze([]),
        source_refs: Object.freeze([]),
        safeToProbe: true,
        safe_to_probe: true,
        requiredParams: ['condition'],
        optionalParams: [],
        responseFormat: 'JSON'
      }
    ],
    [
      'TYPESET_STUDY_RESULT_MULTI',
      {
        id: 'TYPESET_STUDY_RESULT_MULTI',
        routeId: 'TYPESET_STUDY_RESULT_MULTI',
        category: 'FAIndex',
        servletPath: '/servlet/controller.common.TestpageSelectExServlet',
        processName: 'getStudyResultSingleTestingMultiUser',
        httpMethod: 'POST',
        http_method: 'POST',
        routeTemplate: '/servlet/controller.common.TestpageSelectExServlet?p_process=getStudyResultSingleTestingMultiUser',
        route_template: '/servlet/controller.common.TestpageSelectExServlet?p_process=getStudyResultSingleTestingMultiUser',
        operation: 'getStudyResultSingleTestingMultiUser',
        semanticEffect: 'read',
        semantic_effect: 'read',
        effect: 'TYPESET_READ',
        description: '다중 학생 일괄 오답/유사문제 클리닉지 조판 데이터 생성',
        reason: '다중 학생 일괄 오답/유사문제 클리닉지 조판 데이터 생성',
        evidenceGrade: 'live-route',
        evidence_grade: 'live-route',
        evidenceDate: '2026-09-27',
        evidence_date: '2026-09-27',
        joinKeyNames: Object.freeze(['condition']),
        join_key_names: Object.freeze(['condition']),
        sourceRefs: Object.freeze([]),
        source_refs: Object.freeze([]),
        safeToProbe: true,
        safe_to_probe: true,
        requiredParams: ['condition'],
        optionalParams: [],
        responseFormat: 'JSON'
      }
    ],
    [
      'TEXTBOOK_WRONG_ANSWER_NOTE_RESULT',
      {
        id: 'TEXTBOOK_WRONG_ANSWER_NOTE_RESULT',
        routeId: 'TEXTBOOK_WRONG_ANSWER_NOTE_RESULT',
        category: 'TextbookHistory',
        servletPath: '/servlet/controller.common.TestpageSelectExServlet',
        processName: 'getWrongAnswerNoteResult',
        httpMethod: 'POST',
        http_method: 'POST',
        routeTemplate: '/servlet/controller.common.TestpageSelectExServlet?p_process=getWrongAnswerNoteResult',
        route_template: '/servlet/controller.common.TestpageSelectExServlet?p_process=getWrongAnswerNoteResult',
        operation: 'getWrongAnswerNoteResult',
        semanticEffect: 'read',
        semantic_effect: 'read',
        effect: 'READ_ONLY',
        description: 'Wrong-answer note rows of one exact course, book and student (direct-key historical index read; changes no data)',
        reason: 'Wrong-answer note rows of one exact course, book and student (direct-key historical index read; changes no data)',
        evidenceGrade: 'live-route',
        evidence_grade: 'live-route',
        evidenceDate: '2026-11-27',
        evidence_date: '2026-11-27',
        joinKeyNames: Object.freeze(['answerNoteResult']),
        join_key_names: Object.freeze(['answerNoteResult']),
        sourceRefs: Object.freeze(['specs/academy-mcp/direct-key-lookup.md#R3']),
        source_refs: Object.freeze(['specs/academy-mcp/direct-key-lookup.md#R3']),
        safeToProbe: true,
        safe_to_probe: true,
        requiredParams: ['answerNoteResult'],
        optionalParams: [],
        responseFormat: 'JSON'
      }
    ],
    [
      'TEXTBOOK_COURSE_BOOK_REPORT',
      {
        id: 'TEXTBOOK_COURSE_BOOK_REPORT',
        routeId: 'TEXTBOOK_COURSE_BOOK_REPORT',
        category: 'TextbookHistory',
        servletPath: '/servlet/controller.coursemanage.TextBookManageServlet',
        processName: 'getCourseBookReport',
        httpMethod: 'POST',
        http_method: 'POST',
        routeTemplate: '/servlet/controller.coursemanage.TextBookManageServlet?reqCmd=getCourseBookReport',
        route_template: '/servlet/controller.coursemanage.TextBookManageServlet?reqCmd=getCourseBookReport',
        operation: 'getCourseBookReport',
        semanticEffect: 'read',
        semantic_effect: 'read',
        effect: 'READ_ONLY',
        description: 'Official course-book report of one exact course, book and student (read; changes no data)',
        reason: 'Official course-book report of one exact course, book and student (read; changes no data)',
        evidenceGrade: 'live-route',
        evidence_grade: 'live-route',
        evidenceDate: '2026-11-27',
        evidence_date: '2026-11-27',
        joinKeyNames: Object.freeze(['course_key', 'book_no', 'student_pri_no']),
        join_key_names: Object.freeze(['course_key', 'book_no', 'student_pri_no']),
        sourceRefs: Object.freeze(['specs/academy-mcp/direct-key-lookup.md#R3']),
        source_refs: Object.freeze(['specs/academy-mcp/direct-key-lookup.md#R3']),
        safeToProbe: true,
        safe_to_probe: true,
        requiredParams: ['course_key', 'book_no', 'student_pri_no'],
        optionalParams: [],
        responseFormat: 'JSON'
      }
    ],
    [
      'TESTPAGE_EXAM_LIST',
      {
        id: 'TESTPAGE_EXAM_LIST',
        routeId: 'TESTPAGE_EXAM_LIST',
        category: 'PaperPrint',
        servletPath: '/servlet/controller.common.TestpageSelectExServlet',
        processName: 'getTestExamList',
        httpMethod: 'POST',
        http_method: 'POST',
        routeTemplate: '/servlet/controller.common.TestpageSelectExServlet?p_process=getTestExamList',
        route_template: '/servlet/controller.common.TestpageSelectExServlet?p_process=getTestExamList',
        operation: 'getTestExamList',
        semanticEffect: 'read',
        semantic_effect: 'read',
        effect: 'READ_ONLY',
        description: 'Exam list of one exact paper for the print PDFs (read; changes no data)',
        reason: 'Exam list of one exact paper for the print PDFs (read; changes no data)',
        evidenceGrade: 'live-route',
        evidence_grade: 'live-route',
        evidenceDate: '2026-11-28',
        evidence_date: '2026-11-28',
        joinKeyNames: Object.freeze(['p_no', 'p_option_type']),
        join_key_names: Object.freeze(['p_no', 'p_option_type']),
        sourceRefs: Object.freeze(['specs/studyq-linux/core.md#R20']),
        source_refs: Object.freeze(['specs/studyq-linux/core.md#R20']),
        safeToProbe: true,
        safe_to_probe: true,
        requiredParams: ['p_no', 'p_option_type'],
        optionalParams: [],
        responseFormat: 'JSON'
      }
    ],
    [
      'FA_TEST_PAPER_LIST_GRP',
      {
        id: 'FA_TEST_PAPER_LIST_GRP',
        routeId: 'FA_TEST_PAPER_LIST_GRP',
        category: 'FAIndex',
        servletPath: '/servlet/controller.tutor.fa.TestPageListGrpServlet',
        processName: 'Main',
        httpMethod: 'GET',
        http_method: 'GET',
        routeTemplate: '/servlet/controller.tutor.fa.TestPageListGrpServlet?p_process=Main',
        route_template: '/servlet/controller.tutor.fa.TestPageListGrpServlet?p_process=Main',
        operation: 'Main',
        semanticEffect: 'read',
        semantic_effect: 'read',
        effect: 'READ_ONLY',
        description: '형성평가 시험지 목록 및 출제 현황 조회',
        reason: '형성평가 시험지 목록 및 출제 현황 조회',
        evidenceGrade: 'live-route',
        evidence_grade: 'live-route',
        evidenceDate: '2026-09-27',
        evidence_date: '2026-09-27',
        joinKeyNames: Object.freeze([]),
        join_key_names: Object.freeze([]),
        sourceRefs: Object.freeze([]),
        source_refs: Object.freeze([]),
        safeToProbe: true,
        safe_to_probe: true,
        requiredParams: [],
        optionalParams: ['p_pageno', 'grade_no', 'term_no'],
        responseFormat: 'HTML'
      }
    ],
    [
      'NA_USER_SEARCH_RESULT',
      {
        id: 'NA_USER_SEARCH_RESULT',
        routeId: 'NA_USER_SEARCH_RESULT',
        category: 'NAIndex',
        servletPath: '/servlet/controller.tutor.base.TestPageListServlet',
        processName: 'UserBySearchTestResult&ass_no=1002',
        httpMethod: 'POST',
        http_method: 'POST',
        routeTemplate: '/servlet/controller.tutor.base.TestPageListServlet?p_process=UserBySearchTestResult&ass_no=1002',
        route_template: '/servlet/controller.tutor.base.TestPageListServlet?p_process=UserBySearchTestResult&ass_no=1002',
        operation: 'UserBySearchTestResult',
        semanticEffect: 'read',
        semantic_effect: 'read',
        effect: 'READ_ONLY',
        description: '수시평가(Daily Test / Zero Test) 학생별 응시 결과 목록 조회',
        reason: '수시평가(Daily Test / Zero Test) 학생별 응시 결과 목록 조회',
        evidenceGrade: 'live-route',
        evidence_grade: 'live-route',
        evidenceDate: '2026-09-27',
        evidence_date: '2026-09-27',
        joinKeyNames: Object.freeze(['clg_no', 'stu_name', 'sort_date1', 'sort_date2']),
        join_key_names: Object.freeze(['clg_no', 'stu_name', 'sort_date1', 'sort_date2']),
        sourceRefs: Object.freeze([]),
        source_refs: Object.freeze([]),
        safeToProbe: true,
        safe_to_probe: true,
        requiredParams: ['clg_no', 'stu_name', 'sort_date1', 'sort_date2'],
        optionalParams: ['p_pageno', 'checkAllPage'],
        responseFormat: 'EXTERN_DIALOG_HTML'
      }
    ],
    [
      'NA_PUPIL_SEARCH',
      {
        id: 'NA_PUPIL_SEARCH',
        routeId: 'NA_PUPIL_SEARCH',
        category: 'NAIndex',
        servletPath: '/servlet/controller.tutor.na.TestPageListServlet',
        processName: 'PupilSearch',
        httpMethod: 'GET',
        http_method: 'GET',
        routeTemplate: '/servlet/controller.tutor.na.TestPageListServlet?p_process=PupilSearch',
        route_template: '/servlet/controller.tutor.na.TestPageListServlet?p_process=PupilSearch',
        operation: 'PupilSearch',
        semanticEffect: 'read',
        semantic_effect: 'read',
        effect: 'READ_ONLY',
        description: '수시평가 문항별 상세 채점 결과 조회',
        reason: '수시평가 문항별 상세 채점 결과 조회',
        evidenceGrade: 'live-route',
        evidence_grade: 'live-route',
        evidenceDate: '2026-09-27',
        evidence_date: '2026-09-27',
        joinKeyNames: Object.freeze(['testing_no', 'p_no', 'test_no']),
        join_key_names: Object.freeze(['testing_no', 'p_no', 'test_no']),
        sourceRefs: Object.freeze([]),
        source_refs: Object.freeze([]),
        safeToProbe: true,
        safe_to_probe: true,
        requiredParams: ['testing_no', 'p_no', 'test_no'],
        optionalParams: ['paper_type', 'testing_name'],
        responseFormat: 'HTML'
      }
    ],
    [
      'DA_DIAGNOST_LIST',
      {
        id: 'DA_DIAGNOST_LIST',
        routeId: 'DA_DIAGNOST_LIST',
        category: 'DAIndex',
        servletPath: '/servlet/controller.tutor.diag.DiagnostManageServlet',
        processName: 'TestPaperList',
        httpMethod: 'GET',
        http_method: 'GET',
        routeTemplate: '/servlet/controller.tutor.diag.DiagnostManageServlet?reqCmd=TestPaperList',
        route_template: '/servlet/controller.tutor.diag.DiagnostManageServlet?reqCmd=TestPaperList',
        operation: 'TestPaperList',
        semanticEffect: 'read',
        semantic_effect: 'read',
        effect: 'READ_ONLY',
        description: '진단평가 시험지 목록 조회',
        reason: '진단평가 시험지 목록 조회',
        evidenceGrade: 'live-route',
        evidence_grade: 'live-route',
        evidenceDate: '2026-09-27',
        evidence_date: '2026-09-27',
        joinKeyNames: Object.freeze(['reqCmd']),
        join_key_names: Object.freeze(['reqCmd']),
        sourceRefs: Object.freeze([]),
        source_refs: Object.freeze([]),
        safeToProbe: true,
        safe_to_probe: true,
        requiredParams: ['reqCmd'],
        optionalParams: ['grade_no'],
        responseFormat: 'HTML'
      }
    ],
    [
      'DA_DIAGNOST_RESULT_LIST',
      {
        id: 'DA_DIAGNOST_RESULT_LIST',
        routeId: 'DA_DIAGNOST_RESULT_LIST',
        category: 'DAIndex',
        servletPath: '/servlet/controller.tutor.diag.DiagnostManageServlet',
        processName: 'TestResultList',
        httpMethod: 'GET',
        http_method: 'GET',
        routeTemplate: '/servlet/controller.tutor.diag.DiagnostManageServlet?reqCmd=TestResultList',
        route_template: '/servlet/controller.tutor.diag.DiagnostManageServlet?reqCmd=TestResultList',
        operation: 'TestResultList',
        semanticEffect: 'read',
        semantic_effect: 'read',
        effect: 'READ_ONLY',
        description: '진단평가 응시 결과 목록 조회',
        reason: '진단평가 응시 결과 목록 조회',
        evidenceGrade: 'live-route',
        evidence_grade: 'live-route',
        evidenceDate: '2026-09-27',
        evidence_date: '2026-09-27',
        joinKeyNames: Object.freeze(['reqCmd']),
        join_key_names: Object.freeze(['reqCmd']),
        sourceRefs: Object.freeze([]),
        source_refs: Object.freeze([]),
        safeToProbe: true,
        safe_to_probe: true,
        requiredParams: ['reqCmd'],
        optionalParams: ['student_name', 'date'],
        responseFormat: 'HTML'
      }
    ],
    [
      'BASE_USER_SEARCH',
      {
        id: 'BASE_USER_SEARCH',
        routeId: 'BASE_USER_SEARCH',
        category: 'BaseManageIndex',
        servletPath: '/servlet/controller.tutor.base.UserSearchServlet',
        processName: 'Main',
        httpMethod: 'GET',
        http_method: 'GET',
        routeTemplate: '/servlet/controller.tutor.base.UserSearchServlet?p_process=Main',
        route_template: '/servlet/controller.tutor.base.UserSearchServlet?p_process=Main',
        operation: 'Main',
        semanticEffect: 'read',
        semantic_effect: 'read',
        effect: 'READ_ONLY',
        description: '원생 목록 및 상세 정보 검색',
        reason: '원생 목록 및 상세 정보 검색',
        evidenceGrade: 'live-route',
        evidence_grade: 'live-route',
        evidenceDate: '2026-09-27',
        evidence_date: '2026-09-27',
        joinKeyNames: Object.freeze([]),
        join_key_names: Object.freeze([]),
        sourceRefs: Object.freeze([]),
        source_refs: Object.freeze([]),
        safeToProbe: true,
        safe_to_probe: true,
        requiredParams: [],
        optionalParams: ['stu_name', 'grade_no', 'cls_no'],
        responseFormat: 'HTML'
      }
    ],
    [
      'BASE_USER_CLS_MANAGE',
      {
        id: 'BASE_USER_CLS_MANAGE',
        routeId: 'BASE_USER_CLS_MANAGE',
        category: 'BaseManageIndex',
        servletPath: '/servlet/controller.tutor.base.UserClsManageServlet',
        processName: 'Main',
        httpMethod: 'GET',
        http_method: 'GET',
        routeTemplate: '/servlet/controller.tutor.base.UserClsManageServlet?p_process=Main',
        route_template: '/servlet/controller.tutor.base.UserClsManageServlet?p_process=Main',
        operation: 'Main',
        semanticEffect: 'read',
        semantic_effect: 'read',
        effect: 'READ_ONLY',
        description: '학생 승인 및 반편성 현황 조회',
        reason: '학생 승인 및 반편성 현황 조회',
        evidenceGrade: 'live-route',
        evidence_grade: 'live-route',
        evidenceDate: '2026-09-27',
        evidence_date: '2026-09-27',
        joinKeyNames: Object.freeze([]),
        join_key_names: Object.freeze([]),
        sourceRefs: Object.freeze([]),
        source_refs: Object.freeze([]),
        safeToProbe: true,
        safe_to_probe: true,
        requiredParams: [],
        optionalParams: ['cls_no'],
        responseFormat: 'HTML'
      }
    ],
    [
      'BASE_PAD_CERTIFICATE',
      {
        id: 'BASE_PAD_CERTIFICATE',
        routeId: 'BASE_PAD_CERTIFICATE',
        category: 'BaseManageIndex',
        servletPath: '/servlet/controller.tutor.base.UserSearchServlet',
        processName: 'GetCertificateMain',
        httpMethod: 'GET',
        http_method: 'GET',
        routeTemplate: '/servlet/controller.tutor.base.UserSearchServlet?p_process=GetCertificateMain',
        route_template: '/servlet/controller.tutor.base.UserSearchServlet?p_process=GetCertificateMain',
        operation: 'GetCertificateMain',
        semanticEffect: 'read',
        semantic_effect: 'read',
        effect: 'READ_ONLY',
        description: '학원 학습패드(태블릿) 등록 및 인증 기기 현황 조회',
        reason: '학원 학습패드(태블릿) 등록 및 인증 기기 현황 조회',
        evidenceGrade: 'live-route',
        evidence_grade: 'live-route',
        evidenceDate: '2026-09-27',
        evidence_date: '2026-09-27',
        joinKeyNames: Object.freeze([]),
        join_key_names: Object.freeze([]),
        sourceRefs: Object.freeze([]),
        source_refs: Object.freeze([]),
        safeToProbe: true,
        safe_to_probe: true,
        requiredParams: [],
        optionalParams: [],
        responseFormat: 'HTML'
      }
    ],
    [
      'COURSE_MAIN_INDEX',
      {
        id: 'COURSE_MAIN_INDEX',
        routeId: 'COURSE_MAIN_INDEX',
        category: 'CourseMainIndex',
        servletPath: '/servlet/controller.tutor.TutorMenuIndexServlet',
        processName: 'CourseMainIndex',
        httpMethod: 'GET',
        http_method: 'GET',
        routeTemplate: '/servlet/controller.tutor.TutorMenuIndexServlet?p_process=CourseMainIndex',
        route_template: '/servlet/controller.tutor.TutorMenuIndexServlet?p_process=CourseMainIndex',
        operation: 'CourseMainIndex',
        semanticEffect: 'read',
        semantic_effect: 'read',
        effect: 'READ_ONLY',
        description: '학습관리 메인 대시보드 및 학습일지 진입 라우트',
        reason: '학습관리 메인 대시보드 및 학습일지 진입 라우트',
        evidenceGrade: 'live-route',
        evidence_grade: 'live-route',
        evidenceDate: '2026-09-27',
        evidence_date: '2026-09-27',
        joinKeyNames: Object.freeze([]),
        join_key_names: Object.freeze([]),
        sourceRefs: Object.freeze([]),
        source_refs: Object.freeze([]),
        safeToProbe: true,
        safe_to_probe: true,
        requiredParams: [],
        optionalParams: [],
        responseFormat: 'HTML'
      }
    ],
    ...REPORT_VIEW_NAMES.map(reportViewLegacyRoute),
    ...LMS_READ_ROUTES.map(lmsReadLegacyRoute)
  ]);

  /**
   * Initializes the registry after validating every canonical operation.
   */
  private static ensureInitialized(): void {
    if (this._initialized) {
      return;
    }

    if (!existsSync(ROUTE_REGISTRY_PATH)) {
      throw new Error(`[LmsRouteRegistry] Canonical route registry not found at: '${ROUTE_REGISTRY_PATH}'`);
    }

    const rawContent = readFileSync(ROUTE_REGISTRY_PATH, 'utf-8');
    let parsed: RawRegistryData;
    try {
      parsed = validateCanonicalRegistry(JSON.parse(rawContent));
    } catch (err) {
      throw new Error(
        `[LmsRouteRegistry] Failed to parse route-registry.json at '${ROUTE_REGISTRY_PATH}': ${
          err instanceof Error ? err.message : String(err)
        }`
      );
    }

    if (!parsed || !Array.isArray(parsed.entries)) {
      throw new Error(`[LmsRouteRegistry] Invalid route-registry.json format: 'entries' array missing`);
    }

    // The dated baseline contains 54 operations; reviewed additions need no count override.

    // Count operations to resolve non-unique names safely
    const opCounts = new Map<string, number>();
    for (const entry of parsed.entries) {
      const op = entry.operation || '';
      opCounts.set(op, (opCounts.get(op) || 0) + 1);
    }

    for (const entry of parsed.entries) {
      // Validate entry schema
      if (!entry.id || typeof entry.id !== 'string') {
        throw new Error(`[LmsRouteRegistry] Route entry missing valid 'id': ${JSON.stringify(entry)}`);
      }
      if (!entry.operation || typeof entry.operation !== 'string') {
        throw new Error(`[LmsRouteRegistry] Route entry '${entry.id}' missing valid 'operation'`);
      }
      if (typeof entry.route_template !== 'string') {
        throw new Error(`[LmsRouteRegistry] Route entry '${entry.id}' missing valid 'route_template'`);
      }
      if (typeof entry.semantic_effect !== 'string') {
        throw new Error(`[LmsRouteRegistry] Route entry '${entry.id}' missing valid 'semantic_effect'`);
      }
      if (typeof entry.safe_to_probe !== 'boolean') {
        throw new Error(`[LmsRouteRegistry] Route entry '${entry.id}' missing valid boolean 'safe_to_probe'`);
      }
      if (!Array.isArray(entry.join_key_names)) {
        throw new Error(`[LmsRouteRegistry] Route entry '${entry.id}' missing valid array 'join_key_names'`);
      }
      if (!Array.isArray(entry.source_refs)) {
        throw new Error(`[LmsRouteRegistry] Route entry '${entry.id}' missing valid array 'source_refs'`);
      }

      // Extract servlet path and process name from route template
      let servletPath = entry.route_template;
      let processName = entry.operation;
      if (entry.route_template.startsWith('native:')) {
        servletPath = entry.route_template;
      } else if (entry.route_template.includes('?')) {
        const [pathPart, queryPart] = entry.route_template.split('?');
        servletPath = pathPart;
        const searchParams = new URLSearchParams(queryPart);
        processName = searchParams.get('p_process') || searchParams.get('reqCmd') || entry.operation;
      }

      // Determine category from servlet path or entry properties
      let category = 'BackendMap';
      if (servletPath.includes('tutor.fa') || entry.id.startsWith('fa_')) {
        category = 'FAIndex';
      } else if (servletPath.includes('tutor.na') || entry.id.startsWith('dtzt_')) {
        category = 'NAIndex';
      } else if (servletPath.includes('tutor.diag')) {
        category = 'DAIndex';
      } else if (servletPath.includes('tutor.base') || entry.id.startsWith('base_') || entry.id.startsWith('attendance_')) {
        category = 'BaseManageIndex';
      } else if (servletPath.includes('tutor.wb') || entry.id.startsWith('smartbook_') || entry.id.startsWith('workbook_')) {
        category = 'SmartBookIndex';
      } else if (servletPath.includes('coursemanage') || entry.id.startsWith('study_') || entry.id.startsWith('prestudy_')) {
        category = 'CourseManageIndex';
      } else if (servletPath.includes('DayRecordServlet') || entry.id.startsWith('day_record_') || entry.id.startsWith('course_')) {
        category = 'CourseMainIndex';
      }

      // Semantic effect to legacy LmsEndpointEffect
      const isPureRead = entry.semantic_effect === 'read' && entry.safe_to_probe === true;
      const effect: LmsEndpointEffect = isPureRead ? 'READ_ONLY' : 'MUTATING';

      const method = entry.http_method === 'GET' || entry.http_method === 'POST' ? entry.http_method : null;

      const def: LmsRouteDefinition = Object.freeze({
        id: entry.id,
        routeId: entry.id,
        routeTemplate: entry.route_template,
        route_template: entry.route_template,
        operation: entry.operation,
        httpMethod: method,
        http_method: method,
        semanticEffect: entry.semantic_effect as any,
        semantic_effect: entry.semantic_effect as any,
        evidenceGrade: entry.evidence_grade,
        evidence_grade: entry.evidence_grade,
        evidenceDate: entry.evidence_date,
        evidence_date: entry.evidence_date,
        joinKeyNames: Object.freeze([...entry.join_key_names]),
        join_key_names: Object.freeze([...entry.join_key_names]),
        safeToProbe: entry.safe_to_probe,
        safe_to_probe: entry.safe_to_probe,
        reason: entry.reason,
        description: entry.reason,
        sourceRefs: Object.freeze([...entry.source_refs]),
        source_refs: Object.freeze([...entry.source_refs]),
        category,
        servletPath,
        processName,
        effect,
        // The registry's join keys describe response binding, not request wire inputs.
        // Only the reviewed DayRecord read has a verified bounded selector here.
        requiredParams: Object.freeze(entry.id === 'day_record_read' ? ['std_ymd', 'grp_seq'] : []),
        optionalParams: Object.freeze([]),
        responseFormat: 'HTML'
      });

      this.canonicalRoutes.push(def);
      this.routesById.set(entry.id, def);

      // Only register operation name if it is unique across all entries (avoids collisions like 'Main')
      if (opCounts.get(entry.operation) === 1) {
        this.routesByOperation.set(entry.operation, def);
      }
    }

    this._initialized = true;
  }

  /**
   * Retrieves a canonical route by ID or unique operation name.
   */
  public static getRoute(routeIdOrOperation: string): LmsRouteDefinition {
    this.ensureInitialized();
    const route = this.routesById.get(routeIdOrOperation)
      || this.routesByOperation.get(routeIdOrOperation);

    if (!route) {
      throw new RouteLookupError(`[LmsRouteRegistry] Unknown routeId: ${routeIdOrOperation}`);
    }
    return { ...route };
  }

  /**
   * Checks whether a canonical route exists by ID or unique operation.
   */
  public static hasRoute(routeIdOrOperation: string): boolean {
    this.ensureInitialized();
    return this.routesById.has(routeIdOrOperation)
      || this.routesByOperation.has(routeIdOrOperation);
  }

  /**
   * Returns every validated canonical operation from route-registry.json.
   * The original 54-entry evidence remains unchanged; additions require valid metadata.
   */
  public static getAllRoutes(): LmsRouteDefinition[] {
    this.ensureInitialized();
    return this.canonicalRoutes.map(r => ({ ...r }));
  }

  /**
   * Lists canonical routes, optionally filtered by category or effect.
   */
  public static listRoutes(filter?: { category?: string; effect?: LmsEndpointEffect }): LmsRouteDefinition[] {
    this.ensureInitialized();
    let filtered = this.canonicalRoutes;
    if (filter?.category) {
      filtered = filtered.filter(r => r.category === filter.category);
    }
    if (filter?.effect) {
      filtered = filtered.filter(r => r.effect === filter.effect);
    }
    return filtered.map(r => ({ ...r }));
  }

  /**
   * The generic request builder is deliberately disabled. Each operation requires
   * a reviewed effect, selector, and response-binding adapter.
   */
  public static buildRequest(routeId: string, params: Record<string, string | number>, baseUrl: string = 'https://dc.gang-a.kr'): {
    url: string;
    method: 'GET' | 'POST';
    headers: Record<string, string>;
    body?: string;
  } {
    const route = this.getRoute(routeId);
    void params;
    void baseUrl;
    if (route.id === 'textbook_sample_pdf') {
      throw new Error('[LmsRouteRegistry] Use resolveGaussSamplePdfUrl for catalog-scoped PDF URLs');
    }
    throw new Error('[LmsRouteRegistry] Canonical routes require a reviewed operation-specific adapter');
  }

  /**
   * Fail-Closed Safe Read Check.
   * Returns true ONLY if the operation is strictly a read operation AND safe to probe.
   * NEVER uses http_method === 'GET' to determine safety!
   */
  public static isSafeRead(routeIdOrOperation: string): boolean {
    this.ensureInitialized();
    if (!this.hasRoute(routeIdOrOperation)) {
      return false;
    }
    try {
      const route = this.getRoute(routeIdOrOperation);
      return route.semanticEffect === 'read' && route.safeToProbe === true;
    } catch {
      return false;
    }
  }

  /**
   * Fail-Closed Safe Read Assertion (RUB-03 Compliance).
   *
   * TRAP 1: HTTP Method Safety Fallacy!
   * day_record_udtprg, day_record_udthw, day_record_udtmemo, day_record_udtattn,
   * similar_paper_create use http_method: "GET" (or native) but have semantic_effect: "write"
   * or safe_to_probe: false.
   *
   * Strictly throws UnsafeMutatingOperationError if semantic_effect !== "read" or safe_to_probe !== true.
   */
  public static assertSafeRead(routeIdOrOperation: string): void {
    this.ensureInitialized();
    const route = this.getRoute(routeIdOrOperation);

    if (!this.isSafeRead(routeIdOrOperation)) {
      throw new UnsafeMutatingOperationError(
        `[LmsRouteRegistry: SAFETY VIOLATION] Attempted to invoke unsafe/mutating route "${routeIdOrOperation}" ` +
        `(id: ${route.id}, op: ${route.operation}, method: ${route.httpMethod}, semantic_effect: ${route.semanticEffect}, safe_to_probe: ${route.safeToProbe}) in read-only harness.`
      );
    }
  }
}

// Standalone functional exports for ergonomic parity
export function getRoute(routeIdOrOperation: string): LmsRouteDefinition {
  return LmsRouteRegistry.getRoute(routeIdOrOperation);
}

export function getAllRoutes(): LmsRouteDefinition[] {
  return LmsRouteRegistry.getAllRoutes();
}

export function hasRoute(routeIdOrOperation: string): boolean {
  return LmsRouteRegistry.hasRoute(routeIdOrOperation);
}

export function isSafeRead(routeIdOrOperation: string): boolean {
  return LmsRouteRegistry.isSafeRead(routeIdOrOperation);
}

export function assertSafeRead(routeIdOrOperation: string): void {
  LmsRouteRegistry.assertSafeRead(routeIdOrOperation);
}

/** The class of one ReportViewServlet request (specs/academy-mcp/direct-key-lookup.md R8): `read` only for
 * a POST whose `p_process` is one of the six report views; any other view name or method is `write`. */
export function reportViewRequestEffect(method: string | undefined, view: unknown): 'read' | 'write' {
  return (method ?? 'GET').toUpperCase() === 'POST' && typeof view === 'string'
    && (REPORT_VIEW_NAMES as readonly string[]).includes(view) ? 'read' : 'write';
}

/** The six report-view routes, as classed above (POST only). */
export function reportViewRoutes(): LmsRouteDefinition[] {
  return REPORT_VIEW_NAMES.map(view => ({ ...reportViewLegacyRoute(view)[1] }));
}

/** The nine read routes of specs/academy-mcp/lms-reads.md, each `semanticEffect: 'read'` and `safeToProbe: true`. */
export function lmsReadServletRoutes(): LmsRouteDefinition[] {
  return LMS_READ_ROUTES.map(def => ({ ...lmsReadLegacyRoute(def)[1] }));
}

/**
 * The two pages whose inline handlers the gated clinic write pins by `wireSha256`
 * (specs/academy-mcp/gated-writes.md "Wire hash"): both are plain GETs that change no data, classed read by their
 * named route. Standalone, like `LMS_READ_ROUTES`: nothing here loads the canonical route registry file.
 */
export const CLINIC_READ_ROUTES: readonly {id: string; path: string; param: 'p_process' | 'reqCmd'; operation: string; method: 'GET'}[] = [
  {id: 'CLINIC_TEXTBOOK_MAIN', path: '/servlet/controller.coursemanage.TextBookManageServlet', param: 'reqCmd', operation: 'getTextBookMain', method: 'GET'},
  {id: 'CLINIC_FA_PAGE_MAIN', path: '/servlet/controller.tutor.fa.TestPageListGrpServlet', param: 'p_process', operation: 'Main', method: 'GET'},
];

export const TESTPAGE_SELECT_EX_PATH = '/servlet/controller.common.TestpageSelectExServlet';
const TESTPAGE_ALWAYS_READ = ['getWrongAnswerNoteResult', 'getTestExamList'] as const;
const TESTPAGE_CONDITION_OPERATIONS = ['getStudyResultSingleTestingSingleUser', 'getStudyResultSingleTestingMultiUser', 'getStudyResultMultiTestingMultiUser'] as const;
const TESTPAGE_CREATE_FLAGS = ['incorrect', 'similar', 'advance'] as const;

/**
 * The class of one POST of `TestpageSelectExServlet` (specs/academy-mcp/gated-writes.md "Request classes").
 * The score view and the paper-creation request share an operation name and differ in the `create` flags of
 * the `condition` form member: the request is `read` only when `condition` parses to a JSON object and none
 * of `incorrect.create`, `similar.create`, `advance.create` equals 1; any such flag equal to 1, or an absent
 * or malformed `condition`, is `write`. `getWrongAnswerNoteResult` and `getTestExamList` are always `read`;
 * an operation this function does not name is `write`. `body` is the form body as a string or a
 * URLSearchParams.
 */
export function testpageSelectExRequestEffect(operation: string, body?: unknown): 'read' | 'write' {
  if ((TESTPAGE_ALWAYS_READ as readonly string[]).includes(operation)) return 'read';
  if (!(TESTPAGE_CONDITION_OPERATIONS as readonly string[]).includes(operation)) return 'write';
  const form = typeof body === 'string' ? new URLSearchParams(body) : body instanceof URLSearchParams ? body : null;
  const raw = form?.get('condition');
  if (typeof raw !== 'string') return 'write';
  let condition: unknown;
  try { condition = JSON.parse(raw); } catch { return 'write'; }
  if (condition === null || typeof condition !== 'object' || Array.isArray(condition)) return 'write';
  for (const flag of TESTPAGE_CREATE_FLAGS) {
    const member = (condition as Record<string, unknown>)[flag];
    if (member !== undefined && (member === null || typeof member !== 'object' || Array.isArray(member))) return 'write';
    const create = (member as Record<string, unknown> | undefined)?.create;
    if (create !== undefined && create !== 0 && create !== 1) return 'write';
    if (create === 1) return 'write';
  }
  return 'read';
}

/** The class of one clinic-related request by route (path and operation), for the standalone classification. */
export function clinicRequestEffect(method: string | undefined, path: string, operation: string, body?: unknown): 'read' | 'write' {
  const verb = (method ?? 'GET').toUpperCase();
  if (path === TESTPAGE_SELECT_EX_PATH) return testpageSelectExRequestEffect(operation, body);
  return verb === 'GET' && CLINIC_READ_ROUTES.some(r => r.path === path && r.operation === operation) ? 'read' : 'write';
}

/**
 * The routes of the gated FA and unit-exam write (specs/academy-mcp/gated-writes.md "Request classes", `academy_exam_paper_review` and
 * `_save`), in two standalone lists like `CLINIC_READ_ROUTES`: nothing here loads the canonical route registry file, and
 * `endpointCandidates.ts`, the catalog and the function-relation map are not edited. A route is classed by its path, operation and
 * HTTP method, never by the method alone: a GET can write (the allocating `p_process=Auto` page, `SaveMunhang`, the assignment save).
 * `<family>` is `fa` or `da` (the servlet package `controller.tutor.<family>`).
 */
export type ExamFamily = 'fa' | 'da';
export const EXAM_FAMILIES: readonly ExamFamily[] = ['fa', 'da'];
export const examRegPath = (family: ExamFamily): string => `/servlet/controller.tutor.${family}.TestPageRegServlet`;
export const examHangPath = (family: ExamFamily): string => `/servlet/controller.tutor.${family}.TestHangServlet`;
export const TESTPAGE_SELECT_PATH = '/servlet/controller.common.TestpageSelectServlet';
export const OBJECT_SELECT_PATH = '/servlet/controller.common.ObjectSelectServlet';
export const DIAGNOST_MANAGE_PATH = '/servlet/controller.tutor.diag.DiagnostManageServlet';
export const TEST_PAGE_LIST_PATH = '/servlet/controller.tutor.base.TestPageListServlet';
export interface ExamRoute {id: string; path: string; param: 'p_process' | 'reqCmd'; operation: string; method: 'GET' | 'POST' | 'ANY'}
const perFamily = (id: string, path: (f: ExamFamily) => string, param: 'p_process' | 'reqCmd', operation: string, method: 'GET' | 'POST'): ExamRoute[] =>
  EXAM_FAMILIES.map(f => ({id: `${id}_${f.toUpperCase()}`, path: path(f), param, operation, method}));
/** Read: no data changes. The existing `getTestExamList` and the `TestPageListServlet` routes of `searchStudentPaperMetadata` are already read. */
export const EXAM_READ_ROUTES: readonly ExamRoute[] = [
  {id: 'EXAM_TEST_EXAM_LIST', path: TESTPAGE_SELECT_EX_PATH, param: 'p_process', operation: 'getTestExamList', method: 'POST'},
  {id: 'EXAM_PAPER_NO', path: DIAGNOST_MANAGE_PATH, param: 'reqCmd', operation: 'GetPaperNo', method: 'POST'},
  {id: 'EXAM_STUDENT_LIST', path: OBJECT_SELECT_PATH, param: 'reqCmd', operation: 'getStudentList', method: 'POST'},
  {id: 'EXAM_USER_BY_MAIN', path: TEST_PAGE_LIST_PATH, param: 'p_process', operation: 'UserByMain', method: 'ANY'},
  {id: 'EXAM_USER_BY_SEARCH_TEST_RESULT', path: TEST_PAGE_LIST_PATH, param: 'p_process', operation: 'UserBySearchTestResult', method: 'ANY'},
  ...perFamily('EXAM_CHECK_MUNHANG', examRegPath, 'reqCmd', 'CheckMunhang', 'GET'),
  ...perFamily('EXAM_AUTO_MODIFY', examRegPath, 'p_process', 'AutoModify', 'GET'),
  ...perFamily('EXAM_HANG_MAIN', examHangPath, 'p_process', 'Main', 'GET'),
  ...perFamily('EXAM_HANG_VALIDATE', examHangPath, 'reqCmd', 'Validate', 'GET'),
];
/** Write: only inside the save, under the ledger lock. */
export const EXAM_WRITE_ROUTES: readonly ExamRoute[] = [
  ...perFamily('EXAM_AUTO_ALLOCATE', examRegPath, 'p_process', 'Auto', 'GET'),
  {id: 'EXAM_SAVE_MUNHANG', path: TESTPAGE_SELECT_PATH, param: 'reqCmd', operation: 'SaveMunhang', method: 'GET'},
  ...perFamily('EXAM_PAPER_SAVE', examRegPath, 'reqCmd', 'Save', 'POST'),
  ...perFamily('EXAM_HANG_SAVE', examHangPath, 'reqCmd', 'Save', 'GET'),
];
const examMatches = (list: readonly ExamRoute[], method: string, path: string, operation: string): boolean =>
  list.some(r => r.path === path && r.operation === operation && (r.method === 'ANY' || r.method === method.toUpperCase()));
/** The listed class of one request of the exam pair, or null when neither list names it. */
export function examRouteClass(method: string | undefined, path: string, operation: string): 'read' | 'write' | null {
  const verb = (method ?? 'GET').toUpperCase();
  if (examMatches(EXAM_WRITE_ROUTES, verb, path, operation)) return 'write';
  if (path === TESTPAGE_SELECT_EX_PATH) return operation === 'getTestExamList' ? 'read' : null;
  return examMatches(EXAM_READ_ROUTES, verb, path, operation) ? 'read' : null;
}
/** The class of one request of the exam pair: a route neither list names is write (specs/academy-mcp/gated-writes.md "Request classes"). */
export const examRequestEffect = (method: string | undefined, path: string, operation: string): 'read' | 'write' => examRouteClass(method, path, operation) ?? 'write';
/** The path (without query) and operation (`p_process`, else `reqCmd`, of the query) of a request path. */
export function examRouteOf(pathWithQuery: string): {path: string; operation: string} {
  const parsed = new URL(pathWithQuery, 'https://lms.invalid');
  return {path: parsed.pathname, operation: parsed.searchParams.get('p_process') ?? parsed.searchParams.get('reqCmd') ?? ''};
}
