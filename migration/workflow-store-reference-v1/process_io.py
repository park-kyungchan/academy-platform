"""Pipe bounds for migration-only native children; no daemon or runtime controller."""
import os,selectors,signal,subprocess,time
def kill_group(p):
 try:os.killpg(p.pid,signal.SIGKILL)
 except ProcessLookupError:pass
def capture(p,timeout,limit=4*1024*1024):
 streams=selectors.DefaultSelector();buffers={'stdout':bytearray(),'stderr':bytearray()};total=0;timed=False;overflow=False;deadline=time.monotonic()+timeout
 for name in buffers:
  pipe=getattr(p,name);os.set_blocking(pipe.fileno(),False);streams.register(pipe,selectors.EVENT_READ,name)
 try:
  while streams.get_map():
   if time.monotonic()>=deadline and not timed:timed=True;kill_group(p);deadline=time.monotonic()+3
   if timed and time.monotonic()>=deadline:raise RuntimeError('s10_pipe_cleanup_deadline')
   for key,_ in streams.select(.05):
    data=os.read(key.fileobj.fileno(),65536)
    if not data:streams.unregister(key.fileobj);continue
    total+=len(data);remaining=max(0,limit-sum(map(len,buffers.values())));buffers[key.data].extend(data[:remaining])
    if total>limit and not overflow:overflow=True;kill_group(p)
  p.wait(timeout=3)
  return bytes(buffers['stdout']),bytes(buffers['stderr']),timed,overflow
 finally:
  streams.close()
  if p.poll() is None:kill_group(p);p.wait(timeout=3)
  p.stdout.close();p.stderr.close()
