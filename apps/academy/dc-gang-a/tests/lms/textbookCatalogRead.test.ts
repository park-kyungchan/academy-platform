import {expect,test} from 'bun:test';
import {createHash} from 'node:crypto';
import {AcademyHttpClient,sourceDomParser} from '../../src/lms/academyHttpClient';
import {parseTextbookCatalog,qualifiedTextbookPdfUrl,boundedTextbookBody} from '../../src/lms/textbookCatalogRead';
const url='https://storage.studyq.net/data/answer/mi/book/g7_fictionalplus_sample_1_1.pdf';
const ref=createHash('sha256').update(JSON.stringify(['middle',url])).digest('hex');
const row=(label:string,link=url)=>`<tr><td>${label}</td><td><button onclick="fullUrlDownloadFile('${link}')">sample</button></td></tr>`;
const catalog='<div class="tab tab-on">중등부</div><table>'+row('Invented Plus middle1 term1 version2')+row('Same shared PDF; other printed label')+'</table>';
const principal='<a>로그아웃</a><form name="userForm"><input name="pri_no" value="99001"></form><form name="form1"><input name="g_pri_no" value="99001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="teacher"></form>';
const login='<form id="loginForm" name="loginForm" method="post"><input name="p_process"><input name="p_prino"><input name="p_groupno"><input name="userpwd"><input name="userid"><input name="cf_pc"><input name="cf_user"><input name="autoLoginCheck"><input name="autoLoginKey"></form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>';
const pdf=Buffer.from('%PDF-1.7\nsynthetic transport fixture, not a real document\n%%EOF\n');
function fixture(sample:()=>Response=()=>new Response(pdf,{headers:{'content-type':'application/pdf'}}),catalogHtml=catalog,pdfUrl=url){
 const calls:{url:string;init:RequestInit}[]=[];
 const client=new AcademyHttpClient({teacherLabel:'Invented Teacher',teacherKey:'99001',collegeKey:'88001'},async(path,init)=>{
  calls.push({url:path,init});
  if(calls.length<=3)return new Response(calls.length===1?login:principal,{headers:{'set-cookie':'JSESSIONID=synthetic; Path=/'}});
  if(path===pdfUrl)return sample();
  expect(path).toBe('https://dc.gang-a.kr/servlet/controller.coursemanage.CourseManageServlet?reqCmd=GaStudyAnswer');
  expect(init.method).toBe('GET');
  return new Response(catalogHtml,{headers:{'content-type':'text/html; charset=UTF-8'}});
 });
 return {client,calls,authenticate:()=>client.authenticate({username:'fictional',password:'fictional'})};
}
test('catalog preserves actual shared links and labels without asserting edition identity or executing scripts',()=>{
 const value=parseTextbookCatalog(catalog+'<script>throw Error("must not execute")</script>',sourceDomParser.parseFromString);
 expect(value.entries).toEqual([{kind:'sample',documentRef:ref,url,filename:'g7_fictionalplus_sample_1_1.pdf',sourceLabels:['Invented Plus middle1 term1 version2 sample','Same shared PDF; other printed label sample']}]);
 expect(value.catalogSha256).toMatch(/^[a-f0-9]{64}$/);
 expect(value.coverage).toBe('all_observed_pdf_links_in_one_catalog_document');
 expect(()=>parseTextbookCatalog('<div class="tab tab-on">중등부</div><table>nothing downloaded</table>',sourceDomParser.parseFromString)).toThrow('textbook_catalog_pdf_links_unqualified');
 expect(()=>parseTextbookCatalog('<input type=password>'+catalog,sourceDomParser.parseFromString)).toThrow('textbook_catalog_authentication_required');
});
test('observed sample links reject foreign hosts, alternate paths, queries, credentials and encoded traversal',()=>{
 for(const bad of [url.replace('storage.studyq.net','evil.example'),url+'?download=1',url+'#page=1',url.replace('/book/','/other/'),url.replace('https://','https://name:secret@'),url.replace('g7_fictionalplus','..%2fg7_fictionalplus')]){
  expect(()=>qualifiedTextbookPdfUrl(bad)).toThrow('textbook_pdf_link_rejected');
  expect(()=>parseTextbookCatalog('<div class="tab tab-on">중등부</div><table>'+row('Invented',bad)+'</table>',sourceDomParser.parseFromString)).toThrow();
 }
});
test('actual rowspans retain grade labels, workbook answer kind and selected catalog identity',()=>{
 const workbook='https://storage.studyq.net/data/answer/mi/workbook/g7_fictionalplus_answer.pdf';
 const html='<div class="tab tab-on">중등부</div><table><tr><td rowspan="2">중1</td><td>Invented ordinary</td><td></td></tr>'+row('Invented Plus',workbook)+'</table>';
 const value=parseTextbookCatalog(html,sourceDomParser.parseFromString);
 expect(value.entries[0]).toMatchObject({kind:'answer',url:workbook,sourceLabels:['중1 Invented Plus sample']});
 expect(()=>parseTextbookCatalog(html,sourceDomParser.parseFromString,'elementary')).toThrow('textbook_catalog_school_level_rejected');
 expect(qualifiedTextbookPdfUrl('https://storage.studyq.net/data/answer/mi/fiveend/g_fictional_total.pdf').pathname).toContain('/fiveend/');
});
test('owner authentication is required and a forged ref never invokes public storage',async()=>{
 const f=fixture();await expect(f.client.readTextbookCatalog({teacherLabel:'Invented Teacher'})).rejects.toThrow('authentication_required');expect(f.calls).toHaveLength(0);
 await f.authenticate();await expect(f.client.readTextbookPdf({teacherLabel:'Invented Teacher',schoolLevel:'middle',documentRef:'0'.repeat(64)})).rejects.toThrow('textbook_pdf_not_in_current_catalog');
 expect(f.calls).toHaveLength(4);expect(f.calls.some(x=>x.url===url)).toBe(false);
 await expect(f.client.readTextbookCatalog({teacherLabel:'Foreign Teacher'})).rejects.toThrow('textbook_catalog_principal_rejected');
 expect(f.calls).toHaveLength(4);f.client.close();
});
test('PDF fetch requalifies the catalog and never forwards academy cookies or authorization',async()=>{
 const f=fixture();await f.authenticate();const value=await f.client.readTextbookPdf({teacherLabel:'Invented Teacher',schoolLevel:'middle',documentRef:ref});
 expect(value.bytes).toEqual(new Uint8Array(pdf));expect(value.sha256).toBe(createHash('sha256').update(pdf).digest('hex'));
 expect(value.documentRef).toBe(ref);expect(value.sourceRequests).toBe(2);expect(f.calls).toHaveLength(5);
 expect(new Headers(f.calls[3]!.init.headers).has('cookie')).toBe(true);
 const storage=f.calls[4]!.init;expect(storage.credentials).toBe('omit');expect(storage.redirect).toBe('manual');
 expect(new Headers(storage.headers).has('cookie')).toBe(false);expect(new Headers(storage.headers).has('authorization')).toBe(false);f.client.close();
});
test('PDF redirects, non-PDF bodies and size limits stop without retry or artifact success',async()=>{
 for(const response of [()=>new Response(null,{status:302,headers:{location:'https://evil.example'}}),()=>new Response('<html>login</html>',{headers:{'content-type':'text/html'}}),()=>new Response('not pdf',{headers:{'content-type':'application/pdf'}}),()=>new Response(pdf,{headers:{'content-type':'application/pdf','content-length':'999999999'}})]){
  const f=fixture(response);await f.authenticate();await expect(f.client.readTextbookPdf({teacherLabel:'Invented Teacher',schoolLevel:'middle',documentRef:ref})).rejects.toThrow('textbook_pdf_');expect(f.calls).toHaveLength(5);f.client.close();
 }
 await expect(boundedTextbookBody(new Response(pdf,{headers:{'content-type':'application/pdf'}}),8,'pdf')).rejects.toThrow('textbook_pdf_size_rejected');
 await expect(boundedTextbookBody(new Response(pdf,{headers:{'content-type':'application/pdf','content-length':'8'}}),1000,'pdf')).rejects.toThrow('textbook_pdf_size_rejected');
 // Fetch decodes compressed bodies; compressed content-length is not decoded length.
 expect(await boundedTextbookBody(new Response(pdf,{headers:{'content-type':'application/pdf','content-length':'8','content-encoding':'gzip'}}),1000,'pdf')).toEqual(new Uint8Array(pdf));
});
test('a workbook reference larger than the sample cap is preserved as an answer PDF within the separate reference bound',async()=>{
 const referenceUrl=url.replace('_sample_1_1','_answer');
 const large=new Uint8Array(33_803_723);large.set(Buffer.from('%PDF-1.7\n'));large.set(Buffer.from('\n%%EOF\n'),large.length-7);
 const html='<div class="tab tab-on">중등부</div><table>'+row('Invented Plus reference',referenceUrl)+'</table>';
 const f=fixture(()=>new Response(large,{headers:{'content-type':'application/pdf','content-length':String(large.length)}}),html,referenceUrl);
 await f.authenticate();const result=await f.client.readTextbookPdf({teacherLabel:'Invented Teacher',schoolLevel:'middle',documentRef:createHash('sha256').update(JSON.stringify(['middle',referenceUrl])).digest('hex')});
 expect(result.kind).toBe('answer');expect(result.byteLength).toBe(33_803_723);expect(result.sha256).toBe(createHash('sha256').update(large).digest('hex'));f.client.close();
});
