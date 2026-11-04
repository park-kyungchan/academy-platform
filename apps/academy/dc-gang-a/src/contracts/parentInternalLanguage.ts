/** Parent text only: keep exact edition metadata in teacher/internal evidence.
 * Known codes: both edition catalogs + policy 1P6A + retained textbook evidence 1U0T.
 * Codes require metadata context: parentheses (including v3(...)) or an adjacent
 * book/series label. Plain algebra has no such label and remains valid.
 * Known codes ignore ASCII case; the future shape uses uppercase letters, so
 * parenthesized lowercase algebra also remains valid. Keep one ES2022 literal
 * for the existing MCP pinned-source loader. */
export const parentInternalLanguage=/강사\s*확인|교사\s*확인|교사\s*검토|교사\s*(?:최종\s*)?완료\s*판정|채점|미실시|미완료|시간\s*부족|확인\s*전|검사.{0,8}미완료|풀이와\s*채점|[Tt][Ee][Aa][Cc][Hh][Ee][Rr]\s*(?:[Ii][Nn][Ss][Pp][Ee][Cc][Tt][Ii][Oo][Nn]|[Rr][Ee][Vv][Ii][Ee][Ww])|[Uu][Nn][Gg][Rr][Aa][Dd][Ee][Dd]|[Nn][Oo][Tt]\s*[Cc][Oo][Mm][Pp][Ll][Ee][Tt][Ee][Dd]|[Ii][Nn][Ss][Uu][Ff][Ff][Ii][Cc][Ii][Ee][Nn][Tt]\s*[Tt][Ii][Mm][Ee]|(?:시리즈|버전|[Vv][Ee][Rr][Ss][Ii][Oo][Nn]|[Vv])\s*(?:[:._-]\s*)?\d+|\(\s*(?:1[Ss][Aa][Gg]|1[Ss][Jj]2|1[Uu]0[Uu]|1[Uu][Ff][Jj]|1[Nn]82|1[Nn]87|1[Nn]88|1[Mm][Hh][Ll]|1[Pp]6[Aa]|1[Uu]0[Tt]|[1-9][A-Z][0-9][A-Z])\s*\)|(?:(?:가우스(?:\s*플러스)?|다빈치)(?:\s+(?:초|중|고)?[0-9]+(?:-[0-9]+){0,2})?|교재(?:명)?|교과서|문제집|시리즈)\s*(?:[:#-]\s*)?(?:1[Ss][Aa][Gg]|1[Ss][Jj]2|1[Uu]0[Uu]|1[Uu][Ff][Jj]|1[Nn]82|1[Nn]87|1[Nn]88|1[Mm][Hh][Ll]|1[Pp]6[Aa]|1[Uu]0[Tt]|[1-9][A-Z][0-9][A-Z])(?![A-Za-z0-9])/;
/** Permanent parent-field line grammar; historical source records remain unchanged. */
export const parentDailyRecordLines=/^- [^\r\n]+(?:\n- [^\r\n]+)*$/;
