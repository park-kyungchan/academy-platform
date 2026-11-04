"""Unchanged original reference.ts presentation() observer projection.
Ranges/values come exclusively from frozen actual-original functions; raw slices
come from the independent fictional inputs. No candidate output supplies expected data.
"""
def project(case,result):
 if result['outcome']!='returned':return result
 value=result['value'];spans=[]
 def span(kind,source,range,**extra):
  units=source.encode('utf-16-le',errors='surrogatepass');raw=units[range['start']*2:range['end']*2].decode('utf-16-le',errors='surrogatepass')
  spans.append({'kind':kind,**extra,'range':range,'raw':raw})
 for control in value['controls']:
  span('control',case['html'],control['sourceRange'])
  for option in control['options']:span('option',case['html'],option['sourceRange'])
 for function in value['functionEvidence']:
  source=case['html']if function['source']=='selector'else case['helpers']
  span('function',source,function['sourceRange'],name=function['name'])
  for literal in function['literals']:span('literal',source,literal['sourceRange'])
 for filter in value['filterDeclarations']:span('filter',case['html'],filter['sourceRange'])
 for column in value['gridColumns']:span('column',case['html'],column['sourceRange'])
 return {'outcome':'returned','value':{'value':value,'requiredOriginalSpans':spans}}
