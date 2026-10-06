/** Offline validator for precisely the keywords used by the bundled r2 schema.
 * Unsupported keywords and non-local references fail closed; not a general JSON Schema implementation. */
type Schema = { [key: string]: unknown };
const supported = new Set(['$schema','$id','$defs','$ref','title','description','type','properties','required','additionalProperties','items','enum','const','minimum','minLength','minItems','maxItems','uniqueItems','oneOf','anyOf','allOf','if','then','format']);
const obj = (v: unknown): v is Schema => !!v && typeof v === 'object' && !Array.isArray(v);
export function validateSchema(value: unknown, schema: Schema, root: Schema = schema, path = '$'): string[] {
  for (const key of Object.keys(schema)) if (!supported.has(key)) throw new Error(`Unsupported schema keyword: ${key}`);
  if (schema.$ref) {
    const ref = String(schema.$ref);
    if (!ref.startsWith('#/')) throw new Error('Only local schema references are supported');
    let target: unknown = root;
    for (const part of ref.slice(2).split('/')) target = obj(target) ? target[part.replace(/~1/g,'/').replace(/~0/g,'~')] : undefined;
    if (!obj(target)) throw new Error(`Unresolved schema reference: ${ref}`);
    return validateSchema(value, target, root, path);
  }
  const errors: string[] = [];
  const fail = (message: string) => errors.push(`${path}: ${message}`);
  for (const key of ['oneOf','anyOf','allOf'] as const) if (Array.isArray(schema[key])) {
    const results = (schema[key] as Schema[]).map(s => validateSchema(value,s,root,path));
    const count = results.filter(r=>!r.length).length;
    if (key==='oneOf' ? count!==1 : key==='anyOf' ? count===0 : count!==results.length) fail(`${key} failed (${count}/${results.length} branches)`);
  }
  if (obj(schema.if) && obj(schema.then) && !validateSchema(value,schema.if,root,path).length) errors.push(...validateSchema(value,schema.then,root,path));
  if (schema.type) {
    const types = Array.isArray(schema.type) ? schema.type : [schema.type];
    const matches = types.some(t => t==='null' ? value===null : t==='array' ? Array.isArray(value) : t==='object' ? obj(value) : t==='integer' ? typeof value==='number' && Number.isInteger(value) : typeof value===t);
    if (!matches) { fail(`expected ${types.join('|')}`); return errors; }
  }
  if ('const' in schema && JSON.stringify(schema.const)!==JSON.stringify(value)) fail('const mismatch');
  if (Array.isArray(schema.enum) && !schema.enum.some(v=>JSON.stringify(v)===JSON.stringify(value))) fail('enum mismatch');
  if (typeof value==='number' && typeof schema.minimum==='number' && value<schema.minimum) fail('below minimum');
  if (typeof value==='string') {
    if (typeof schema.minLength==='number' && [...value].length<schema.minLength) fail('below minLength');
    if (schema.format==='date' && (!/^\d{4}-\d{2}-\d{2}$/.test(value) || !Number.isFinite(Date.parse(value)) || new Date(value).toISOString().slice(0,10)!==value)) fail('invalid date');
    if (schema.format==='date-time' && (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/.test(value) || !Number.isFinite(Date.parse(value)))) fail('invalid offset timestamp');
    if (schema.format && !['date','date-time'].includes(String(schema.format))) throw new Error('Unsupported format');
  }
  if (Array.isArray(value)) {
    if (typeof schema.minItems==='number' && value.length<schema.minItems) fail('below minItems');
    if (typeof schema.maxItems==='number' && value.length>schema.maxItems) fail('above maxItems');
    if (schema.uniqueItems && new Set(value.map(v=>JSON.stringify(v))).size!==value.length) fail('duplicate items');
    if (obj(schema.items)) value.forEach((v,i)=>errors.push(...validateSchema(v,schema.items as Schema,root,`${path}[${i}]`)));
  }
  if (obj(value)) {
    if (Array.isArray(schema.required)) for (const key of schema.required) if (typeof key==='string' && !Object.hasOwn(value,key)) fail(`missing ${key}`);
    const properties = obj(schema.properties) ? schema.properties : {};
    for (const [key,v] of Object.entries(value)) {
      if (obj(properties[key])) errors.push(...validateSchema(v,properties[key],root,`${path}.${key}`));
      else if (schema.additionalProperties===false) fail(`unexpected ${key}`);
      else if (obj(schema.additionalProperties)) errors.push(...validateSchema(v,schema.additionalProperties,root,`${path}.${key}`));
    }
  }
  return errors;
}
