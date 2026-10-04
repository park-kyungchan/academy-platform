import { describe, expect, test } from 'bun:test';
import { validateHierarchy, sourceId, traceHierarchy, type CatalogNode } from '../../src/academy/domain';
import { known, unknown } from '../../src/learning/model';

describe('grounded shared curriculum hierarchy', () => {
  test('preserves namespaced source labels and requires each known parent level', () => {
    const book: CatalogNode = { id: 'catalog-book', kind: 'book_edition', sourceKey: { namespace: 'academy_book', literalKey: '8101' }, label: 'Invented edition', parent: unknown('root_book') };
    const major: CatalogNode = { id: 'catalog-major', kind: 'major_unit', sourceKey: { namespace: 'academy_major_unit', literalKey: 'unit-1' }, label: 'Invented major', parent: known(book.id, ['fixture']) };
    const nodes = [book, major];
    validateHierarchy(nodes);
    expect(traceHierarchy(nodes, major.id).path.map(row => row.id)).toEqual([book.id, major.id]);
    expect(sourceId({ namespace: 'academy_video_label', literalKey: 'H2031' })).not.toBe(sourceId({ namespace: 'academy_board_lecture', literalKey: 'H2031' }));
    expect(() => validateHierarchy([book, { ...major, kind: 'minor_unit' }])).toThrow('catalog_parent_level');
  });
});
