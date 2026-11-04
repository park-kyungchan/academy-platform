import {expect,test} from 'bun:test';
import {sourceDomParser} from '../../src/lms/academyHttpClient';
import {parsePrestudySearchForm,parsePrestudyList,parsePrestudySummary,prestudyRowsForStudent,parseTextbookBindings,parsePrestudyDetail,parsePrestudyReport} from '../../src/lms/prestudyRead';

const form=(cmd:string)=>`<form name="form1"><input type="hidden" name="reqCmd" value="${cmd}"><input type="hidden" name="g_clg_no" value="88001"><input type="hidden" name="g_mem_type" value="1"><input type="hidden" name="g_pri_no" value="99001"><input type="text" name="student_name" value=""></form>`;
const waiting=`<html>${form('WebUnPreStudy')}<table><tr><th>학생이름</th><th>단원명</th><th>날짜</th><th>영상 상태</th><th></th></tr>
<tr><td>Invented Student</td><td>9-1 Fictional Unit (기본)(ver. 1)</td><td>40.01.02 03:04</td><td>OK</td><td><button onclick="prestudy_main(12345);">예습 검사</button></td></tr>
<tr><td>Other Student</td><td>9-2 Other Unit(ver. 1)</td><td>40.01.02 04:05</td><td>OK</td><td><button onclick="prestudy_main(12346);">예습 검사</button></td></tr></table></html>`;
const summary=`<table><tr><th>학생</th><th>단원명</th><th>날짜</th><th>Ai총점</th><th>별점</th><th>action</th></tr>
<tr><td>Invented Student(초5)</td><td>9-1 Fictional Unit(ver. 1)</td><td>40.01.02 03:04</td><td><span>77점</span> <button onclick="openReport(777);">보고서</button></td><td>-</td><td><button onclick="playVideo('https://cdn.example/prestudy/1_77001_1_prestudy.mp4')">재생</button></td></tr></table>`;

test('prestudy search form is read back from the live shell and bound to the principal',()=>{
  expect(parsePrestudySearchForm(waiting,sourceDomParser.parseFromString,'WebUnPreStudy','99001'))
    .toEqual({reqCmd:'WebUnPreStudy',collegeKey:'88001',memberType:'1',teacherKey:'99001'});
  expect(()=>parsePrestudySearchForm(waiting,sourceDomParser.parseFromString,'WebCtPreStudy','99001')).toThrow('prestudy_search_form_drift');
  expect(()=>parsePrestudySearchForm(waiting,sourceDomParser.parseFromString,'WebUnPreStudy','99002')).toThrow('prestudy_search_form_drift');
  expect(()=>parsePrestudySearchForm('<input type=password>'+waiting,sourceDomParser.parseFromString,'WebUnPreStudy','99001')).toThrow('prestudy_authentication_required');
});

test('waiting rows keep upload literal and prestudy key without deriving teacher review',()=>{
  const rows=parsePrestudyList(waiting,sourceDomParser.parseFromString,'WebUnPreStudy','99001');
  expect(rows).toHaveLength(2);
  expect(prestudyRowsForStudent(rows,'Invented Student')).toEqual([
    {studentLabel:'Invented Student',unitLabel:'9-1 Fictional Unit (기본)(ver. 1)',dateLiteral:'40.01.02 03:04',statusLiteral:'OK',prestudyKey:'12345'}]);
  expect(prestudyRowsForStudent(rows,'Invented')).toEqual([]);
});

test('summary rows retain AI/star literals and availability flags but never the video URL',()=>{
  const rows=parsePrestudySummary(summary,sourceDomParser.parseFromString);
  expect(rows).toEqual([{studentLabel:'Invented Student(초5)',unitLabel:'9-1 Fictional Unit(ver. 1)',dateLiteral:'40.01.02 03:04',
    aiScoreLiteral:'77점 보고서',ratingLiteral:'-',reportAvailable:true,videoAvailable:true}]);
  expect(JSON.stringify(rows)).not.toContain('cdn.example');
  expect(prestudyRowsForStudent(rows,'Invented Student')).toHaveLength(1);
});

test('textbook bindings come only from the exact student row handler',()=>{
  const progress=`<table><tr><td>Invented Student</td><td><p>22개정 Fictional Book</p></td><td>50.0%</td><td><button onclick="createBookIncorrectPapers(672, 2198, 77001);">오답노트</button></td></tr>
  <tr><td>Other Student</td><td><p>Other Book</p></td><td>10.0%</td><td><button onclick="createBookIncorrectPapers(673, 2199, 77002);">오답노트</button></td></tr></table>`;
  expect(parseTextbookBindings(progress,sourceDomParser.parseFromString,'77001'))
    .toEqual([{courseKey:'672',bookNo:'2198',studentKey:'77001',bookLabel:'22개정 Fictional Book'}]);
  expect(()=>parseTextbookBindings(progress,sourceDomParser.parseFromString,'77003')).toThrow('textbook_binding_source_unqualified');
});

const detail=`<html><form name="form1"><input type="hidden" name="prestudy_key" value="12345"><input type="hidden" name="student_pri_no" value="77001"><input type="hidden" name="sunit_name" value="9-1 Fictional Unit(ver. 1)"><input type="hidden" name="create_datetime" value="2040-01-02 03:04:05"><input type="hidden" name="encoding_result" value="success"></form>
<button onclick="viewAiReport('55501');">Ai 보고서 보기</button></html>`;
const detailNoReport=`<html><form name="form1"><input type="hidden" name="prestudy_key" value="12345"><input type="hidden" name="student_pri_no" value="77001"><input type="hidden" name="encoding_result" value="success"></form></html>`;

test('prestudy detail binds exact student and record and preserves source unit/date literals',()=>{
  expect(parsePrestudyDetail(detail,sourceDomParser.parseFromString,'77001','12345')).toEqual({prestudyKey:'12345',unitLabel:'9-1 Fictional Unit(ver. 1)',dateLiteral:'2040-01-02 03:04:05',analysisKey:'55501',encodingResult:'success'});
  expect(parsePrestudyDetail(detailNoReport,sourceDomParser.parseFromString,'77001','12345')).toEqual({prestudyKey:'12345',unitLabel:null,dateLiteral:null,analysisKey:null,encodingResult:'success'});
  expect(()=>parsePrestudyDetail(detail,sourceDomParser.parseFromString,'77002','12345')).toThrow('prestudy_detail_student_mismatch');
  expect(()=>parsePrestudyDetail(detail,sourceDomParser.parseFromString,'77001','12346')).toThrow('prestudy_detail_record_mismatch');
  expect(()=>parsePrestudyDetail('<html><body>gone</body></html>',sourceDomParser.parseFromString,'77001','12345')).toThrow('prestudy_detail_unqualified');
});

test('duplicate detail identity fields and conflicting report links fail closed',()=>{
  for(const html of [detail.replace('</form>','<input name="prestudy_key" value="12346"></form>'),
    detail.replace('</form>','<input name="student_pri_no" value="77002"></form>'),
    detail+'<form name="form1"><input name="prestudy_key" value="12345"></form>',
    detail+'<button onclick="viewAiReport(\'55502\')">other report</button>',
    detail.replace('name="prestudy_key"','name="unrelated_key"')]){
    expect(()=>parsePrestudyDetail(html,sourceDomParser.parseFromString,'77001','12345')).toThrow('prestudy_detail_unqualified');
  }
});

const report=`<html><body>
<div class="w-full max-w-5xl mx-auto bg-white rounded-2xl shadow-lg"><div class="p-4"><h1>예습영상 AI 분석 종합</h1></div><div class="p-4"><p>총점 77점</p><p>한줄 요약</p></div></div>
<div class="max-w-5xl mx-auto bg-white rounded-2xl shadow-lg"><div class="p-4"><h1>개념 이해</h1></div><div class="p-4"><h2>개념을 정확히 이해하고 설명하는가?</h2>
<h3>포함된 핵심 개념</h3><div class="text-sm">첫째 줄<br>둘째 줄</div><h3>advice</h3><div class="text-sm">조언 본문</div></div></div>
<script>const scores = { m1: 77, m2: 80, m3: 90 };</script></body></html>`;

test('prestudy report parses server-rendered section cards without inventing a transcript',()=>{
  const parsed=parsePrestudyReport(report,sourceDomParser.parseFromString);
  expect(parsed.scores).toEqual([77,80,90]);
  expect(parsed.sections).toHaveLength(2);
  expect(parsed.sections[1]).toEqual({title:'개념 이해',subtitle:'개념을 정확히 이해하고 설명하는가?',blocks:[
    {heading:'포함된 핵심 개념',text:'첫째 줄\n둘째 줄'},{heading:'advice',text:'조언 본문'}]});
  expect(JSON.stringify(parsed)).not.toContain('transcript');
  expect(()=>parsePrestudyReport('<html><body><h1>리포트를 생성할 수 없습니다.</h1></body></html>',sourceDomParser.parseFromString)).toThrow('prestudy_report_not_ready');
  expect(()=>parsePrestudyReport('<html><body>empty</body></html>',sourceDomParser.parseFromString)).toThrow('prestudy_report_unqualified');
});

test('unqualified progress responses cannot establish an empty registered-book result',()=>{
  for(const html of ['<html><body>Temporarily unavailable</body></html>','<table></table>',
    '<table><tr><td><button onclick="createBookIncorrectPapers(672,2198,77001)">wrong note</button></td></tr></table>'])
    expect(()=>parseTextbookBindings(html,sourceDomParser.parseFromString,'77001')).toThrow('textbook_binding_source_unqualified');
});
