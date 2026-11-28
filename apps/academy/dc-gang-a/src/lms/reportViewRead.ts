/** The read-only report views of `ReportViewServlet` (specs/academy-mcp/direct-key-lookup.md R8).
 * A tool may read exactly these six views, each by GET; any other name refuses before any
 * request. The HTTP route of each view is pinned by the unit that reads them. */
export const REPORT_VIEW_NAMES=['SmartBookReportView','DiagReportView','NewStudentReportView','FaView','GaReportView','NaScoreResultView'] as const;
export type ReportViewName=typeof REPORT_VIEW_NAMES[number];
export function validateReportViewName(view:unknown):ReportViewName {
  if(typeof view!=='string'||!(REPORT_VIEW_NAMES as readonly string[]).includes(view))throw Error('report_view_name_invalid');
  return view as ReportViewName;
}
