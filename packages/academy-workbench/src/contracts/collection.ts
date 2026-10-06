/** Future read-only collector contract, informed by the approved public contract. */
export type ObservationState = 'not_checked' | 'observed' | 'unresolved';
export interface AssignmentObservations {
  assignmentId: string;
  assignmentConfirmed: ObservationState;
  appScoreObserved: ObservationState;
  videoSubmitted: ObservationState;
  teacherInspectionObserved: ObservationState;
  paperWorkConfirmed: ObservationState;
}
export interface CollectionPartition {
  source: string; studentId: string; selectedPeriod: string; edition: string | null;
  asOfUtc: string; displayedSiteDate: string; siteTimezone: string | 'unknown';
}
export interface CoverageManifest {
  partition: CollectionPartition;
  appliedFilters: Readonly<Record<string, string>>;
  status: 'not_checked' | 'observed' | 'empty_observed' | 'error' | 'unresolved';
  filterVerified: boolean; rowCount: number; pageEndConfirmed: boolean;
  observedAtUtc: string; retryCount: number; unresolvedCount: number;
}
// Empty results require verified filters; errors preserve the last good partition.
// A historical score never completes a new assignment automatically.
// Video version is independent of textbook edition; unresolved mappings stay unresolved.
// No DOM adapter, replacement collector or direct HTTP implementation is enabled.
