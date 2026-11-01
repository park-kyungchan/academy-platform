/** Parent text only: exact edition metadata remains valid in teacher/internal evidence. */
export const parentInternalLanguage=/강사\s*확인|교사\s*확인|교사\s*검토|교사\s*(?:최종\s*)?완료\s*판정|채점|미실시|미완료|시간\s*부족|확인\s*전|검사.{0,8}미완료|풀이와\s*채점|teacher\s*(?:inspection|review)|ungraded|not\s*completed|insufficient\s*time|(?:시리즈|버전|version|v)\s*(?:[:._-]\s*)?\d+|(?<![A-Za-z0-9])[1-9][A-Z][1-9][A-Z](?![A-Za-z0-9])/i;
/** Permanent parent-field line grammar; historical source records remain unchanged. */
export const parentDailyRecordLines=/^- [^\r\n]+(?:\n- [^\r\n]+)*$/;
