/** The read-only report views of `ReportViewServlet` (specs/academy-mcp/direct-key-lookup.md R8).
 * A tool may read exactly these six views, each by `POST /servlet/controller.common.ReportViewServlet`
 * with the form body `p_process=<View>&...` (reportViewClient.ts); any other name refuses before any
 * request. */
export const REPORT_VIEW_NAMES=['SmartBookReportView','DiagReportView','NewStudentReportView','FaView','GaReportView','NaScoreResultView'] as const;
export type ReportViewName=typeof REPORT_VIEW_NAMES[number];
export function validateReportViewName(view:unknown):ReportViewName {
  if(typeof view!=='string'||!(REPORT_VIEW_NAMES as readonly string[]).includes(view))throw Error('report_view_name_invalid');
  return view as ReportViewName;
}
