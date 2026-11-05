import { describe, expect, test } from 'bun:test';
import catalogJson from './catalog.json';
import { selectContext } from './select.ts';
import { validateCatalog } from './validate.ts';
import { readingPlan } from './reading-plan.ts';
import { runContextCli } from './cli.ts';
import type { SelectionPage } from './types.ts';

describe('actual academy metadata integration', () => {
  test('foundation.harness CLI returns the U3 successor and only revision metadata for the old maintained claim', async () => {
    const before = JSON.stringify(catalogJson);
    const items: SelectionPage['items'] = [];
    let cursor: string | null = null;
    do {
      const page = await runContextCli(['select', '--topic', 'foundation.harness', '--max-bytes', '24576', ...(cursor ? ['--cursor', cursor] : [])]) as SelectionPage;
      expect(page.usedBytes).toBeLessThanOrEqual(24576);
      items.push(...page.items);
      cursor = page.nextCursor;
    } while (cursor);
    const successor = items.find(item => item.kind === 'record' && item.value.id === 'CTX-U3-RETIREMENT-20261105');
    expect(successor?.kind).toBe('record');
    if (successor?.kind !== 'record') throw Error('Missing U3 successor');
    expect(successor.value).toMatchObject({status:'current',kind:'source-owner-decision',supersedes:['CTX-ED-MAINTENANCE']});
    for (const text of ['29 excluded files', 'no current or complete gang-a implementation claim', 'archive/education-platform-pre-delw1-C-20261105', 'a810066', 'Drive']) expect(successor.value.text).toContain(text);
    expect(items.find(item => item.value.id === 'CTX-ED-MAINTENANCE')).toMatchObject({kind:'reference',value:{status:'historical'}});
    expect(JSON.stringify(items)).not.toContain('is the maintained gang-a code path');
    const history = selectContext(catalogJson, {topicId:'foundation.harness',includeHistory:true,maxBytes:100_000});
    const original = catalogJson.records.find(row => row.id === 'CTX-ED-MAINTENANCE')!;
    expect(history.items.find(item => item.kind === 'record' && item.value.id === original.id)).toMatchObject({kind:'record',value:{...original,status:'historical'}});
    expect(JSON.stringify(catalogJson)).toBe(before);
  });
  test('every in-scope workflow has complete, bounded and duplicate-free continuation', () => {
    const catalog = validateCatalog(catalogJson);
    for (const topic of catalog.topics.filter(row => row.kind === 'workflow' && !catalog.scope.excluded.includes(row.id))) {
      const ids: string[] = [];
      let cursor: string | undefined;
      let total = 0;
      do {
        const page = selectContext(catalog, { topicId: topic.id, maxBytes: 16384, ...(cursor ? { cursor } : {}) });
        expect(Buffer.byteLength(JSON.stringify(page))).toBe(page.usedBytes);
        expect(page.usedBytes).toBeLessThanOrEqual(16384);
        ids.push(...page.items.map(item => item.value.id));
        total = page.closure.itemCount;
        cursor = page.nextCursor ?? undefined;
      } while (cursor);
      expect(ids.length).toBe(total);
      expect(new Set(ids).size).toBe(total);
    }
  });
  test('required reference intake stays smaller than discovery and preserves actual class duties', () => {
    const plan = readingPlan(catalogJson, 'academy.class-preparation');
    expect(plan.requiredReferenceIds).toContain('SRC-DC-CLASSROOM');
    expect(plan.requiredReferenceIds).toContain('SRC-CURRENT-NAVIGATION');
    expect(plan.requiredReferenceIds).toContain('SRC-CLASS-TIMETABLE-GUIDE');
    const source = catalogJson.sources.find(row => row.id === 'SRC-CLASS-TIMETABLE-GUIDE');
    expect(source?.path).toBe('context/class-timetable/README.ko.md');
    expect(plan.requiredReferenceIds.length).toBeLessThan(plan.discoveryReferenceIds.length);
    expect(plan.sourceBodiesLoaded).toBe(false);
    expect(plan.nativePermissionGranted).toBe(false);
  });
  test('excluded workflows and malformed CLI requests fail without source access', async () => {
    expect(() => selectContext(catalogJson, { topicId: 'academy.parent-communication' })).toThrow('topic-outside-scope');
    expect(() => readingPlan(catalogJson, 'academy.counseling')).toThrow();
    await expect(runContextCli(['select', '--topic', 'academy.class-preparation', '--max-bytes', '1e9'])).rejects.toThrow();
    await expect(runContextCli(['inspect', '--topic', 'academy.class-preparation'])).rejects.toThrow();
  });
  test('entrypoint preserves unknown coverage and dates do not confer permission', async () => {
    const value = await runContextCli(['inspect']);
    expect(value).toMatchObject({ authority: 'none', nativePermissionGranted: false, defaultNavigation: ['date','class','student'], coverage: { complete: false, denominator: null }, sourceBodiesLoaded: false });
  });
});
