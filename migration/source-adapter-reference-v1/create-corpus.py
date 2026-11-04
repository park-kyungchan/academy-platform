"""Explicit synthetic bytes/recipes. No original/private data acquisition."""
import json,pathlib,hashlib
r=pathlib.Path(__file__).resolve().parent;c=r/'corpus';c.mkdir(exist_ok=True)
if any(c.iterdir()):raise SystemExit('s09_corpus_already_frozen')
def save(n,x): (c/n).write_text(json.dumps(x,ensure_ascii=False,indent=2)+'\n' if not isinstance(x,str) else x)
principal={'teacherLabel':'S09 Synthetic Teacher','teacherKey':'99001','collegeKey':'88001'}
login='<form id="loginForm" name="loginForm" method="post">'+''.join('<input name="'+n+'">' for n in ['p_process','p_prino','p_groupno','userpwd','userid','cf_pc','cf_user','autoLoginCheck','autoLoginKey'])+'</form><script>document.loginForm.action = "/servlet/controller.login.LoginServlet"; document.loginForm.p_process.value = "c_login";</script>'
save('login.html',login)
save('principal.html','<form name="userForm"><input name="pri_no" value="99001"></form><form name="form1"><input name="g_pri_no" value="99001"><input name="g_clg_no" value="88001"><input name="g_mem_type" value="teacher"></form><span>로그아웃</span>')
header='<thead><tr><th rowspan="2">이름</th><th rowspan="2">과정</th><th rowspan="2">지속사항</th><th colspan="4">지난진도</th><th colspan="6">일괄 발송 금일진도</th></tr><tr>'+''.join('<th>'+x+'</th>' for x in ['지난수업일','진도','숙제','메모','','금일출석','일일테스트','진도','숙제','메모'])+'</tr></thead>'
signatures={'onAttn':'i,course_seq,stu_pri_no,record_seq,cm_seq','onFoPrg':'i,course_seq,stu_pri_no,record_seq,cm_seq','onFoHw':'i,course_seq,stu_pri_no,record_seq,cm_seq','onFoMm':'i,course_seq,stu_pri_no,record_seq,cm_seq','onFoStuMemo':'i,stu_pri_no,course_seq','onOpenDaysMain':'student_pri_no','onDailyReport':'report_seq,stu_pri_no,record_seq,cm_seq','setDailyTest':'record_seq,cm_seq,count','openHomeWorkRate':'record_seq,cm_seq,count'}
script='<script>\n'+''.join('function '+h+'('+params+'){ return "inert"; }\n' for h,params in signatures.items())+'globalThis.s09ScriptRan=true; fetch("https://blocked.invalid/");\n</script>'
def row(rec,prev,prior=False):
 tuple=f"'0','5','77001','{rec}','8'"
 return '<tr>'+''.join('<td>'+v+'</td>' for v in ["<a onclick=\"onOpenDaysMain('77001')\">S09 Learner 가😀</a>",'S09 Course',"<textarea onfocusout=\"onFoStuMemo('0','77001','5')\">unselected synthetic note</textarea>",'2040-01-02', 'prior display only',f'<a onclick="openHomeWorkRate({prev},8,0)">prior assessment</a>','prior memo','',f'<select onchange="onAttn({tuple})"><option value="Y" selected>출석</option><option value="N">결석</option></select>',f'<a onclick="setDailyTest({rec},8,0)">DT</a><input name="h_daily_test" value="0"><input name="h_homework_rate" value="3">',f'<textarea onfocusout="onFoPrg({tuple})">'+('Previous wording' if prior else 'Unit &amp; progress 가😀')+'</textarea>',f'<textarea onfocusout="onFoHw({tuple})">'+('Previous homework' if prior else '')+'</textarea>',f'<input onfocusout="onFoMm({tuple})" value="Memo &quot;raw&quot;">'])+'</tr>'
for n,date,rec,prev,prior in [('current','2040-01-04','13','12',False),('prior','2040-01-02','12','11',True)]:
 page=f'<input name="std_date" value="{date}"><input name="teacher_pri_no" value="99001"><select name="grp_seq"><option value="0">All</option><option value="3" selected>Class 가</option></select><table>'+header+'<tbody>'+row(rec,prev,prior)+'</tbody></table>'+script
 save(n+'.html',page+'<script>\nvar g_param = \"\";\nvar daily_test_radio=\"synthetic\";\n</script>')
 values=['',date,'5','','S09 Learner 가😀','','','','','3',rec,'77001','8'];save(n+'.xml','<root><record>'+'!'.join(values)+'</record></root>')
save('dom.html','<!doctype html>가😀é\r\n<table id="repair"><tr><td>A&amp;B<td>C</table><select id="duplicate"><option value="01"> zero </option><option selected>가&amp;😀</option></select><input id="duplicate" value="other"><textarea id="text">\n가&amp;😀\r\n&lt;x&gt;</textarea><script id="handler">function onFoPrg(i){ return "가😀}"; }\n globalThis.s09ScriptRan=true;fetch("https://blocked.invalid/");</script><script src="https://blocked.invalid/external.js"></script><img src="https://blocked.invalid/pixel" onerror="globalThis.s09ScriptRan=true">')
save('presentation.html','<!doctype html>앞😀\r\n<label for="difficulty">난이도 &amp; 값</label><select id="difficulty" onchange="choose(this.value)"><option value="01">하&amp;중</option><option selected>상😀 </option></select><textarea>unused</textarea><script>\nvar g_examtype_filter="all";var searchConditionDifficulty=0;var searchConditionBehAreaNo=null;\n$("#dbExamListGrid").jqGrid({colNames:["난이도"],colModel:[{name:"difficulty",formatter:dbFormatterDifficulty}]});\nfunction dbFormatterDifficulty(value){return value===0?"미실시":"가😀";}\nfunction previewDBExam(key){return "inert";}\nglobalThis.s09ScriptRan=true; fetch("https://blocked.invalid/");\n</script>')
save('helpers.js','// 가😀\nfunction getExamPath(key){return "/synthetic/";}\nfunction getExamCardImageFileName(key,type){return key+"-"+type;}\nglobalThis.s09ScriptRan=true;')
sid='s09://account-A/student/0001'; edition='edition/A:01'
audit={'schemaVersion':'academy.audit.normalized/1.0.0','collectedAt':'2040-01-04T08:00:00Z','students':[]}
for i,id in enumerate([sid,'s09://account-B/student/0001','0001','0001 ','0001:Ab']):
 audit['students'].append({'studentId':id,'name':'S09 Duplicate Name' if i>2 else 'S09 Saved '+str(i),'sourceAccount':'account-A' if i==0 else 'account-B','books':[],'sections':[]})
s=audit['students'][0];s.update(books=[{'sourceBookEditionId':edition,'displayName':'Synthetic Book 가','nativeBookKey':'001','provenance':'synthetic_link_fixture_only'},{'sourceBookEditionId':'edition/B','displayName':'Separate Synthetic Book'}],sections=[{'id':'section/1','bookEditionId':edition,'rowKind':'section','progress':{'state':'observed','completed':False,'score':0}},{'id':'placeholder','bookEditionId':edition,'rowKind':'placeholder','progress':None},{'id':'section-b','bookEditionId':'edition/B','rowKind':'section'}],attempts=[{'id':'assessment-a','bookEditionId':edition,'sectionId':'section/1','score':0,'teacherFinal':'unknown','relationEvidence':{'basis':'synthetic_explicit_edition_section_link','bookEditionId':edition,'sectionId':'section/1'}},{'id':'missing'},{'id':'null','score':None},{'id':'blank','score':''},{'id':'unknown','score':'unknown'},{'id':'not-performed','state':'not_performed'}],effectiveAssignments=[{'id':'assignment-a','bookEditionId':edition,'sectionId':'section/1','state':'assigned_not_completed'}],prepVideosHistorical=[{'id':'video-1','state':'waiting_teacher_review'},{'id':'video-2','state':'completed_source_not_teacher_final'}],DTZT={'date/DT~1':{'score':0,'bookEditionId':edition},'not-performed':{'state':'not_performed'}},liveDailyRecords=[{'date':'2040-01-04','recordKey':'13','progress':'Unit 1','homework':'','account':'account-A'},{'date':'2040-01-02','recordKey':'12','progress':None,'account':'account-A'}])
audit['students'][1]['prepVideosHistorical']=None
audit['students'][2]['prepVideosHistorical']=[]
lesson={'schemaVersion':'academy.lesson-overlay/1','lessonDate':'2040-01-04','preparedAt':'2040-01-04T09:00:00Z','students':[{'studentId':sid,'teacherEvents':[{'id':'event-a','studentId':sid,'sourceKind':'teacher_report','recordedAt':'2040-01-04T08:00:00Z','facts':[{'state':'teacher_reported_only','bookEditionId':edition}]},{'id':'event-b','studentId':sid,'sourceKind':'user_supplied_message_context','recordedAt':'2040-01-04T08:01:00Z','facts':[{'state':'unknown'}]}],'observations':[{'kind':'dailyRecord','observedAt':'2040-01-04T08:02:00Z','progress':'','homework':None}],'dailyRecordDraft':{'student':{'studentId':sid},'serverSaved':False,'parentSent':False,'status':'unsaved_draft'}}]}
save('audit.json',audit);save('lesson.json',lesson)
save('provenance.json',{'kind':'entirely_synthetic_recorded_local_bytes','source':'selected_original_TS_schemas_and_test_structure','nativePrincipal':principal,'accounts':['account-A','account-B'],'savedFixtureStudent':sid,'nativeFixtureStudent':'77001','sameStudentClaim':False,'realRecordedS05AndP8Gap':'unchanged; no real textbook/progress/assessment-edition evidence manufactured','teacherLabelInSavedOutput':'source_constant_not_authenticated_by_saved_loader'})
cases=[]
def add(id,kind,**args):cases.append({'id':id,'kind':kind,**args})
add('dom-values-and-exact-spans','dom');add('presentation-values-and-spans','presentation')
for v in ['duplicate-control','duplicate-label','malformed-script','duplicate-function','invalid-range','oversize','too-many-options']:
 add('presentation-'+v,'presentation',variant=v)
for v in ['normal','duplicate-rows','inconsistent-key','duplicate-date-id','broken-table','leading-zero-key','case-name','space-name','prior','malformed-xml','wrong-principal','oversize','utf8-invalid','login-challenge','redirect','http-error','foreign-response-url','transport-throw','slow-abort','stalled-body','journal-unqualified','wire-inspection']:
 add('http-'+v,'http',variant=v)
for v in ['normal','retry-login','invalid-principal','foreign-login-redirect','login-redirect','second-redirect','missing-location','foreign-cookie','cookie-path','cookie-delete','login-failure','login-wire','principal-key','principal-college','source-encoding','constructor-leading-zero','constructor-delimiter','constructor-whitespace','constructor-case-key']:
 add('authentication-'+v,'auth',variant=v)
for field in ['progress','homework','lessonMemo','attendance','dailyTestAssessment','previousHomeworkAssessment']:
 add('deny-mutation-'+field,'operation',field=field)
for variant in ['unknown-selector','wrong-method','foreign-origin','duplicate-selector','valid-read']:
 add('read-operation-'+variant,'operation',variant=variant)
for v in ['normal','other-account','duplicate-current','wrong-student','wrong-teacher','wrong-date','state-target','previous-missing','previous-record','previous-student','previous-course','previous-member','previous-date','null-field','zero-field','missing-field','blank-field','scope-namespace','scope-teacher','scope-student','scope-date','auth-failure-twice']:
 add('native-'+v,'native',variant=v)
for name,rows in [('exact',[{'studentKey':'77001','studentName':'S09 Name'}]),('duplicate-identical',[{'studentKey':'77001','studentName':'S09 Name'}]*2),('ambiguous',[{'studentKey':k,'studentName':'S09 Name'} for k in ['77001','77002']]),('wrong-name',[{'studentKey':'77001','studentName':'s09 Name'}]),('leading-zero',[{'studentKey':'077001','studentName':'S09 Name'}]),('extra-field',[{'studentKey':'77001','studentName':'S09 Name','extra':True}]),('empty',[])]:add('resolve-'+name,'resolve',name=' S09 Name ',rows=rows)
for key in ['001','1','1:2','A','a',' 1','1 ','9007199254740992']:add('native-key-'+repr(key),'literal-key',value=key)
queries=[('roster',{'kind':'roster'}),('summary',{'kind':'student_summary','studentId':sid}),('history-all',{'kind':'student_history','studentId':sid}),('history-ordered',{'kind':'student_history','studentId':sid,'facets':['daily_records','sections','books','attempts']}),('lesson-current',{'kind':'lesson_context','studentId':sid,'date':'2040-01-04'}),('lesson-prior',{'kind':'lesson_context','studentId':sid,'date':'2040-01-02'})]
facets=['books','sections','attempts','assignments','videos','daily_tests','daily_records','teacher_events','daily_record_drafts']
for facet in facets:
 q={'kind':'student_records','studentId':sid,'facet':facet};queries.append((facet,q));queries.append((facet+'-edition',dict(q,bookEditionId=edition)))
for id,q in queries:add('saved-'+id,'saved',input={'query':q,'limit':250})
for val in [1,250,0,251,-1,1.5,None,'2']:add('saved-limit-'+str(val),'saved',input={'query':{'kind':'roster'},'limit':val})
for id in [sid,'s09://account-B/student/0001','0001','0001 ','0001:Ab','0001:ab',' '+sid,'S09 Duplicate Name']:
 add('saved-identity-'+str(len(cases)),'saved',input={'query':{'kind':'student_summary','studentId':id}})
for id,inp in [('unknown-field',{'query':{'kind':'roster'},'extra':True}),('wrong-mode',{'query':{'kind':'roster'},'sourceMode':'live'}),('invalid-date',{'query':{'kind':'lesson_context','studentId':sid,'date':'2040-02-31'}}),('duplicate-facet',{'query':{'kind':'student_history','studentId':sid,'facets':['books','books']}}),('empty-facets',{'query':{'kind':'student_history','studentId':sid,'facets':[]}}),('unknown-facet',{'query':{'kind':'student_records','studentId':sid,'facet':'progress'}}),('unknown-edition',{'query':{'kind':'student_records','studentId':sid,'facet':'books','bookEditionId':'edition/unknown'}}),('null-cursor',{'query':{'kind':'roster'},'cursor':None}),('junk-cursor',{'query':{'kind':'roster'},'cursor':'!!!'}),('long-cursor',{'query':{'kind':'roster'},'cursor':'A'*2049})]:add('saved-'+id,'saved',input=inp)
for v in ['pagination','cursor-wrong-selection','cursor-wrong-snapshot','cursor-offset-equal','cursor-offset-over','cursor-negative','cursor-extra','cursor-limit-change','cursor-query-key-order','budget-partial','budget-single','budget-fixed','loader-pin-missing','loader-hash-mismatch','loader-symlink','loader-parent-symlink','loader-world-writable','loader-malformed-json','loader-invalid-utf8','loader-duplicate-id','loader-duplicate-name','loader-lesson-foreign','loader-draft-foreign','loader-event-duplicate']:
 add('saved-'+v,'saved-special',variant=v)
for v in ['valid','bad-hash','symlink','ancestor-symlink','world-writable','oversize','directory','relative']:
 add('pinned-local-'+v,'pinned-local',variant=v)
for v in ['limit-zero','limit-fraction','limit-max','bad-budget','empty','byte-limited','single-too-big']:
 add('pagination-direct-'+v,'paginate',variant=v)
add('module-relative-paths','paths')
add('native-configuration-owner-refusal','native-config')
add('native-configuration-empty','native-config',variant='empty')
add('native-configuration-duplicate-students','config-schema')
save('CASES.json',cases)
print(json.dumps({'cases':len(cases),'corpusFiles':len(list(c.iterdir()))}))
