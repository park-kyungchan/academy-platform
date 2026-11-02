import {expect,test} from 'bun:test';
import {sourceDomParser} from '../../src/lms/academyHttpClient';
import {parsePrestudySearchForm,parsePrestudyList,parsePrestudySummary,prestudyRowsForStudent,parseTextbookBindings} from '../../src/lms/prestudyRead';

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

test('unqualified progress responses cannot establish an empty registered-book result',()=>{
  for(const html of ['<html><body>Temporarily unavailable</body></html>','<table></table>',
    '<table><tr><td><button onclick="createBookIncorrectPapers(672,2198,77001)">wrong note</button></td></tr></table>'])
    expect(()=>parseTextbookBindings(html,sourceDomParser.parseFromString,'77001')).toThrow('textbook_binding_source_unqualified');
});
