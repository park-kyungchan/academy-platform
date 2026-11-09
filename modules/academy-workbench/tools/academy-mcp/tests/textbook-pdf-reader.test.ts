import {test,expect} from 'bun:test';
import {mkdtemp,writeFile,readFile,rm,realpath} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {textbookPdfReaderConfiguration,textbookPageRequest} from '../src/textbook-pdf-reader.ts';
const sha=(b:Buffer|string)=>createHash('sha256').update(b).digest('hex');
test('pinned reader executes verified bytes in isolated Python and rejects helper/interpreter drift',async()=>{
 const root=await mkdtemp(join(tmpdir(),'pdf-reader-contract-'));
 const python=await realpath(execFileSync('python3',['-c','import sys;print(sys.executable)'],{encoding:'utf8'}).trim());
 const interpreterSha=sha(await readFile(python)),helper=join(root,'helper.py');
 const program=`import json,os,sys\nassert len(sys.argv)==4\nassert 'DC_ACADEMY_HTTP_PASSWORD' not in os.environ\nr=json.loads(sys.argv[3]);p=r.get('pages',[1]);print(json.dumps({'schemaVersion':1,'pdfSha256':sys.argv[2],'pageCount':5,'scannedPages':p,'units':[],'missingUnitCodes':r['unitCodes'],'coverage':'selected_pages_only','requiresVisualVerification':True,'reasonCodes':['no_supported_units_found'],'textCandidates':[]}))\n`;
 try{
  await writeFile(helper,program,{mode:0o600});
  const config=await textbookPdfReaderConfiguration(['--textbook-pdf-reader',python,interpreterSha,helper,sha(program),'--unrelated']);
  expect(config.remainingArgs).toEqual(['--unrelated']);
  expect(await config.reader!.read('/not-opened-by-fictional-helper','a'.repeat(64),textbookPageRequest.parse({pages:[2],unitCodes:['1-1']}))).toMatchObject({scannedPages:[2],missingUnitCodes:['1-1'],coverage:'selected_pages_only'});
  await writeFile(helper,program+'# drift');
  await expect(config.reader!.read('/unused','a'.repeat(64),textbookPageRequest.parse({}))).rejects.toThrow('label_snapshot_hash_mismatch');
  await expect(textbookPdfReaderConfiguration(['--textbook-pdf-reader',python,'0'.repeat(64),helper,sha(program+'# drift')])).rejects.toThrow('textbook_reader_interpreter_rejected');
 }finally{await rm(root,{recursive:true,force:true});}
});
