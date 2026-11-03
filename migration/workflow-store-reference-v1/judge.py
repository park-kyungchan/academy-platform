#!/usr/bin/env python3
"""Strict transcript + process/effect-channel judge. No candidate execution by default."""
import argparse,json,os,pathlib,signal,subprocess,time
from process_io import capture
HERE=pathlib.Path(__file__).resolve().parent
def unique(pairs):
 d={}
 for k,v in pairs:
  if k in d:raise ValueError('duplicate_json_key')
  d[k]=v
 return d
def strict(s):return json.loads(s,object_pairs_hook=unique,parse_constant=lambda x:(_ for _ in ()).throw(ValueError('nonfinite_json')))
def canonical(v):return json.dumps(v,ensure_ascii=False,sort_keys=True,separators=(',',':'),allow_nan=False)
def judge(command,evidence,expected=HERE/'EXPECTED-v2.json',deadline=30):
 evidence.mkdir(parents=True,mode=0o700,exist_ok=False)
 env={**os.environ,'S10_MUTANT_EFFECT_ROOT':str(evidence)}
 p=subprocess.Popen(command,env=env,cwd=HERE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
 start=time.monotonic();out,err,timed,overflow=capture(p,deadline)
 try:os.killpg(p.pid,0);gone=False;os.killpg(p.pid,signal.SIGKILL)
 except ProcessLookupError:gone=True
 (evidence/'stdout').write_bytes(out);(evidence/'stderr').write_bytes(err)
 reasons=[]
 if timed:reasons.append('deadline')
 if not gone:reasons.append('descendants')
 if p.returncode!=0:reasons.append('nonzero_exit')
 if err:reasons.append('stderr')
 if overflow:reasons.append('output_bound')
 if not out.endswith(b'\n') or len(out.splitlines())!=1:reasons.append('stdout_frame')
 try:
  actual=strict(out)
  if canonical(actual)!=canonical(strict(expected.read_bytes())):reasons.append('transcript_mismatch')
 except (ValueError,UnicodeError):reasons.append('invalid_json')
 for name in ['denied.log','fake-effects.jsonl']:
  path=evidence/name
  if path.exists() and path.read_bytes():reasons.append(name)
 result={'accepted':not reasons,'reasons':reasons,'pid':p.pid,'returncode':p.returncode,'signal':-p.returncode if p.returncode<0 else None,'timedOut':timed,'outputBoundExceeded':overflow,'groupGone':gone,'elapsedSeconds':time.monotonic()-start,'stdoutBytes':len(out),'stderrBytes':len(err)}
 (evidence/'result.json').write_text(json.dumps(result,indent=2)+'\n');return result
if __name__=='__main__':
 a=argparse.ArgumentParser();a.add_argument('--command-json',required=True);a.add_argument('--evidence',required=True);a.add_argument('--expected',type=pathlib.Path,default=HERE/'EXPECTED-v2.json');args=a.parse_args()
 result=judge(json.loads(args.command_json),pathlib.Path(args.evidence).absolute(),args.expected);print(json.dumps(result));raise SystemExit(0 if result['accepted'] else 1)
