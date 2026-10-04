import { assertExactKeys, assertKnowledge, immutableCopy, requireText, stableId, type Knowledge } from '../learning/model';

/** Matches the existing student-evidence SourceKey definition; namespaces are never aliases. */
export interface SourceKey { readonly namespace: string; readonly literalKey: string }
export type CatalogKind = 'book_edition' | 'major_unit' | 'middle_unit' | 'minor_unit' | 'printed_page' | 'problem';
export interface CatalogNode {
  readonly id: string;
  readonly kind: CatalogKind;
  readonly sourceKey: SourceKey;
  readonly label: string;
  readonly parent: Knowledge<string>;
}
export const catalogLevels: readonly CatalogKind[] = ['book_edition', 'major_unit', 'middle_unit', 'minor_unit', 'printed_page', 'problem'];
export function validateSourceKey(key: SourceKey): void {
  assertExactKeys(key, ['namespace', 'literalKey']);
  requireText(key.namespace, 'source namespace');
  requireText(key.literalKey, 'literal source key');
  if (key.namespace.length > 160 || key.literalKey.length > 200 || /[\u0000-\u001f\u007f]/.test(key.namespace + key.literalKey)) throw Error('invalid_source_key');
}
export function sourceId(key: SourceKey): string {
  validateSourceKey(key);
  return stableId('source', key.namespace, key.literalKey);
}
export function validateHierarchy(nodes: readonly CatalogNode[]): void {
  if (!Array.isArray(nodes) || nodes.length > 100000) throw Error('invalid_catalog');
  const index = new Map<string, CatalogNode>();
  for (const node of nodes) {
    assertExactKeys(node, ['id', 'kind', 'sourceKey', 'label', 'parent']);
    requireText(node.id, 'catalog id'); requireText(node.label, 'catalog label');
    if (node.label.length > 16000 || !catalogLevels.includes(node.kind)) throw Error('invalid_catalog_node');
    validateSourceKey(node.sourceKey); assertKnowledge<string>(node.parent, value => requireText(value, 'catalog parent'));
    if (index.has(node.id)) throw Error('duplicate_catalog_id');
    index.set(node.id, node);
  }
  for (const node of nodes) {
    if (node.kind === 'book_edition') {
      if (node.parent.state === 'known') throw Error('catalog_root_parent');
    } else if (node.parent.state === 'known') {
      const parent = index.get(node.parent.value);
      if (!parent) throw Error('catalog_parent_missing');
      if (catalogLevels.indexOf(parent.kind) !== catalogLevels.indexOf(node.kind) - 1) throw Error('catalog_parent_level');
    }
  }
}
export function traceHierarchy(nodes: readonly CatalogNode[], id: string) {
  validateHierarchy(nodes);
  const index = new Map(nodes.map(node => [node.id, node]));
  const path: CatalogNode[] = [];
  let node = index.get(id);
  if (!node) throw Error('unknown_catalog_node');
  while (node) {
    path.unshift(node);
    if (node.kind === 'book_edition') return immutableCopy({ state: 'known' as const, path, unknowns: [] as string[] });
    if (node.parent.state === 'unknown') return immutableCopy({ state: 'partial' as const, path, unknowns: [node.parent.reason] });
    node = index.get(node.parent.value);
  }
  throw Error('catalog_parent_missing');
}
