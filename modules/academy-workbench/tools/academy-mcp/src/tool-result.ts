/** Existing MCP dual representation, measured as the actual serialized CallToolResult. */
export function toolResult(value: unknown) {
  const text = JSON.stringify(value);
  return { content: [{ type: 'text' as const, text }], structuredContent: JSON.parse(text) as Record<string, unknown> };
}
export const toolResultBytes = (value: unknown) => Buffer.byteLength(JSON.stringify(toolResult(value)));
/** A record appears once as an object and once inside JSON-escaped content.text. */
export function recordTransportBytes(value: unknown) {
  const json = JSON.stringify(value);
  return Buffer.byteLength(json) + Buffer.byteLength(JSON.stringify(json)) - 2;
}
/** Metadata must not push a source-owned bounded reply over its existing byte cap. Audit stays in owner receipts. */
export function withToolMeta<T extends {_meta?:Record<string,unknown>}>(result:T,metadata:Record<string,unknown>,maxBytes=512*1024):T{
  const audited={...result,_meta:{...result._meta,...metadata}};
  return Buffer.byteLength(JSON.stringify(audited))<=maxBytes?audited:result;
}
