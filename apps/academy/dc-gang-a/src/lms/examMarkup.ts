/**
 * A small, fail-closed reader of the two markup shapes the exam write answers in (specs/academy-mcp/gated-writes.md): the `form1` of the
 * creation and AutoModify pages (HTML), and the flat XML answers of `SaveMunhang`, `CheckMunhang` and `Validate`. It runs no script and builds
 * no DOM; it reads tags, attributes and text only, skips comments, `<script>` and `<style>` bodies, and decodes character references. It
 * serves these pages only, not HTML in general.
 */

const NAMED: Record<string, string> = {amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: ' '};
/** Character references of an attribute value or text: the five XML names, `&nbsp;`, and numeric references; anything else is kept as written. */
export function decodeEntities(value: string): string {
  return value.replace(/&(#x[0-9a-fA-F]{1,6}|#[0-9]{1,7}|[a-zA-Z]{2,6});/g, (whole, body: string) => {
    if (body[0] === '#') {
      const code = body[1] === 'x' || body[1] === 'X' ? parseInt(body.slice(2), 16) : parseInt(body.slice(1), 10);
      return Number.isInteger(code) && code > 0 && code <= 0x10ffff && !(code >= 0xd800 && code <= 0xdfff) ? String.fromCodePoint(code) : whole;
    }
    return NAMED[body] ?? whole;
  });
}

export interface Tag {name: string; attrs: Map<string, string>; closing: boolean; selfClosing: boolean; start: number; end: number}
const RAW_TEXT = new Set(['script', 'style']);

/** The attributes of one start tag body. A boolean attribute (`selected`) has the value ''. Names are lower-cased. */
function readAttributes(body: string): Map<string, string> {
  const attrs = new Map<string, string>();
  const pattern = /([^\s"'<>\/=]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+)))?/g;
  for (let m = pattern.exec(body); m; m = pattern.exec(body)) {
    const name = m[1]!.toLowerCase();
    if (!attrs.has(name)) attrs.set(name, decodeEntities(m[2] ?? m[3] ?? m[4] ?? ''));
  }
  return attrs;
}

/** Every tag of an HTML text in order, with comments and the bodies of `<script>` and `<style>` skipped. */
export function htmlTags(html: string): Tag[] {
  const tags: Tag[] = [];
  let i = 0;
  while (i < html.length) {
    const lt = html.indexOf('<', i);
    if (lt < 0) break;
    if (html.startsWith('<!--', lt)) { const end = html.indexOf('-->', lt + 4); i = end < 0 ? html.length : end + 3; continue; }
    if (html[lt + 1] === '!' || html[lt + 1] === '?') { const end = html.indexOf('>', lt + 2); i = end < 0 ? html.length : end + 1; continue; }
    const m = /^<(\/?)([a-zA-Z][a-zA-Z0-9:-]*)((?:"[^"]*"|'[^']*'|[^>"'])*)>/.exec(html.slice(lt, lt + 20000));
    if (!m) { i = lt + 1; continue; }
    const name = m[2]!.toLowerCase(), closing = m[1] === '/', selfClosing = !closing && m[3]!.trimEnd().endsWith('/');
    const end = lt + m[0].length;
    tags.push({name, attrs: closing ? new Map() : readAttributes(m[3]!), closing, selfClosing, start: lt, end});
    i = end;
    if (!closing && RAW_TEXT.has(name)) {
      const close = html.toLowerCase().indexOf('</' + name, end);
      i = close < 0 ? html.length : close;
    }
  }
  return tags;
}

export interface FormControl {
  tag: 'input' | 'select' | 'textarea';
  name: string; type: string; disabled: boolean;
  /** `input`: its `value` attribute; `textarea`: its text. For `select`, see `options`. */
  value: string; checked: boolean;
  options: {value: string; selected: boolean}[]; multiple: boolean;
}
export interface HtmlForm {controls: FormControl[]}

/** The controls of the first `<form>` whose `name` or `id` is `formName`, in page order, or null when the page has no such form. Selects outside it are in `documentSelects`. */
export function readForm(html: string, formName: string): {form: HtmlForm | null; documentSelects: FormControl[]} {
  const tags = htmlTags(html);
  const controlsIn = (from: number, to: number): FormControl[] => {
    const controls: FormControl[] = [];
    for (let t = from; t < to; t++) {
      const tag = tags[t]!;
      if (tag.closing) continue;
      if (tag.name === 'input') {
        controls.push({tag: 'input', name: tag.attrs.get('name') ?? '', type: (tag.attrs.get('type') ?? 'text').toLowerCase(), disabled: tag.attrs.has('disabled'),
          value: tag.attrs.get('value') ?? '', checked: tag.attrs.has('checked'), options: [], multiple: false});
      } else if (tag.name === 'textarea') {
        const close = tags.findIndex((x, k) => k > t && x.closing && x.name === 'textarea');
        const body = close < 0 ? '' : html.slice(tag.end, tags[close]!.start);
        controls.push({tag: 'textarea', name: tag.attrs.get('name') ?? '', type: 'textarea', disabled: tag.attrs.has('disabled'), value: decodeEntities(body.replace(/^\r?\n/, '')), checked: false, options: [], multiple: false});
        if (close > t) t = close;
      } else if (tag.name === 'select') {
        const options: {value: string; selected: boolean}[] = [];
        let k = t + 1;
        for (; k < to && !(tags[k]!.closing && tags[k]!.name === 'select'); k++) {
          const o = tags[k]!;
          if (o.closing || o.name !== 'option') continue;
          const label = html.slice(o.end, tags[k + 1] ? tags[k + 1]!.start : html.length);
          options.push({value: o.attrs.has('value') ? o.attrs.get('value')! : decodeEntities(label.replace(/<[^>]*>/g, '')).trim(), selected: o.attrs.has('selected')});
        }
        controls.push({tag: 'select', name: tag.attrs.get('name') ?? '', type: 'select', disabled: tag.attrs.has('disabled'), value: '', checked: false, options, multiple: tag.attrs.has('multiple')});
        t = k;
      }
    }
    return controls;
  };
  const documentSelects = controlsIn(0, tags.length).filter(c => c.tag === 'select');
  const open = tags.findIndex(t => !t.closing && t.name === 'form' && (t.attrs.get('name') === formName || t.attrs.get('id') === formName));
  if (open < 0) return {form: null, documentSelects};
  const close = tags.findIndex((t, k) => k > open && t.closing && t.name === 'form');
  return {form: {controls: controlsIn(open + 1, close < 0 ? tags.length : close)}, documentSelects};
}

/** The data members a browser would submit for a control: a checkbox or radio only when checked, a select by its selected option (else the first), no button or file. */
export function submittedMembers(control: FormControl): [string, string][] {
  if (!control.name || control.disabled) return [];
  if (control.tag === 'input') {
    if (['submit', 'button', 'reset', 'image', 'file'].includes(control.type)) return [];
    if ((control.type === 'checkbox' || control.type === 'radio') && !control.checked) return [];
    return [[control.name, control.value]];
  }
  if (control.tag === 'textarea') return [[control.name, control.value]];
  if (control.options.length === 0) return [];
  const chosen = control.options.filter(o => o.selected);
  if (control.multiple) return chosen.map(o => [control.name, o.value] as [string, string]);
  return [[control.name, (chosen.at(-1) ?? control.options[0]!).value]];
}

/** The leaf elements of a well-formed XML answer: tag name to the trimmed text of the first element of that name. Anything that is not one well-formed document throws. */
export function xmlLeaves(body: string): Map<string, string> {
  const text = body.replace(/^﻿/, '').trim();
  const stack: string[] = [], leaves = new Map<string, string>();
  let root = 0, i = 0, lastText = '', hasChildren: boolean[] = [];
  const token = /<!--[\s\S]*?-->|<\?[\s\S]*?\?>|<!\[CDATA\[([\s\S]*?)\]\]>|<(\/?)([A-Za-z_][\w.:-]*)((?:"[^"]*"|'[^']*'|[^>"'])*?)(\/?)>|([^<]+)|(<)/g;
  for (let m = token.exec(text); m; m = token.exec(text)) {
    i = token.lastIndex;
    if (m[7]) throw Error('xml_unparsable');
    if (m[6] !== undefined) {
      if (stack.length === 0 && m[6].trim() !== '') throw Error('xml_unparsable');
      lastText += m[6];
    } else if (m[1] !== undefined) lastText += m[1];
    else if (m[3] !== undefined) {
      const name = m[3];
      if (m[2]) {
        if (stack.pop() !== name) throw Error('xml_unparsable');
        const children = hasChildren.pop();
        if (!children && !leaves.has(name)) leaves.set(name, decodeEntities(lastText).trim());
        lastText = '';
      } else {
        if (stack.length === 0 && ++root > 1) throw Error('xml_unparsable');
        if (hasChildren.length) hasChildren[hasChildren.length - 1] = true;
        lastText = '';
        if (m[5]) { if (!leaves.has(name)) leaves.set(name, ''); } else { stack.push(name); hasChildren.push(false); }
      }
    }
  }
  if (stack.length !== 0 || root !== 1 || i === 0) throw Error('xml_unparsable');
  return leaves;
}
