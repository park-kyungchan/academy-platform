//! Finite original read-failure catalog, projected through typed Rust errors.
//! Source: academy-mcp/src/read-failure.ts. No exception text is a public reason.
use crate::js::{Fault, Js, obj, sha};

fn catalog(code: &str) -> Option<(&'static str, &'static str)> {
    Some(match code {
        "upstream_tool_inventory_mismatch" => (
            "contract_drift",
            "src/endpoint-inventory.ts:verifyEndpointInventory",
        ),
        "query_hierarchy_not_found" => ("policy_denial", "src/native-pool-backend.ts:read"),
        "query_outside_declared_scope" => ("policy_denial", "src/native-pool-backend.ts:read"),
        "exact_previously_returned_question_required" => {
            ("policy_denial", "src/native-pool-backend.ts:read")
        }
        "exact_previously_returned_hierarchy_id_required" => {
            ("policy_denial", "src/native-pool-backend.ts:read")
        }
        "exact_existing_level3_or4_scope_required" => {
            ("policy_denial", "src/native-pool-backend.ts:read")
        }
        "exact_previously_returned_type_label_required" => {
            ("policy_denial", "src/native-pool-backend.ts:read")
        }
        "read_scope_not_admitted" => ("policy_denial", "src/native-pool-backend.ts:read"),
        "operator_socket_parent_not_private" => {
            ("policy_denial", "src/native-pool-backend.ts:read")
        }
        "operator_socket_not_private" => ("policy_denial", "src/native-pool-backend.ts:read"),
        "active_profile_required" => ("policy_denial", "src/native-pool-backend.ts:read"),
        "native_http_secret_required" => ("policy_denial", "src/native-pool-backend.ts:read"),
        "operator_required" => ("policy_denial", "src/native-pool-backend.ts:read"),
        "hierarchy_offset_out_of_range" => ("input_validation", "src/native-pool-backend.ts:read"),
        "label_offset_out_of_range" => ("input_validation", "src/native-pool-backend.ts:read"),
        "question_offset_out_of_range" => ("input_validation", "src/native-pool-backend.ts:read"),
        "duplicate_query_hierarchy" => ("input_validation", "src/native-pool-backend.ts:read"),
        "query_has_no_comparable_terms" => ("input_validation", "src/native-pool-backend.ts:read"),
        "live_comparison_requires_one_exact_studied_scope" => {
            ("input_validation", "src/native-pool-backend.ts:read")
        }
        "operator_request_invalid" => ("input_validation", "src/native-pool-backend.ts:read"),
        "operator_request_size_limit" => ("input_validation", "src/native-pool-backend.ts:read"),
        "operator_socket_path_invalid" => ("input_validation", "src/native-pool-backend.ts:read"),
        "operator_socket_arguments_invalid" => {
            ("input_validation", "src/native-pool-backend.ts:read")
        }
        "native_bridge_arguments_invalid" => {
            ("input_validation", "src/native-pool-backend.ts:read")
        }
        "native_pool_authentication_requires_new_input" => {
            ("authentication", "src/native-pool-backend.ts:read")
        }
        "test_pool_authentication_required" => {
            ("authentication", "src/native-pool-backend.ts:read")
        }
        "live_label_selector_source_changed" => {
            ("contract_drift", "src/native-pool-backend.ts:read")
        }
        "live_label_source_scope_conflict" => ("contract_drift", "src/native-pool-backend.ts:read"),
        "live_label_category_scope_conflict" => {
            ("contract_drift", "src/native-pool-backend.ts:read")
        }
        "live_label_hierarchy_identity_conflict" => {
            ("contract_drift", "src/native-pool-backend.ts:read")
        }
        "live_label_hierarchy_parent_conflict" => {
            ("contract_drift", "src/native-pool-backend.ts:read")
        }
        "live_label_hierarchy_cycle" => ("contract_drift", "src/native-pool-backend.ts:read"),
        "live_question_source_scope_conflict" => {
            ("contract_drift", "src/native-pool-backend.ts:read")
        }
        "source_type_id_not_qualified_for_question_read" => {
            ("contract_drift", "src/native-pool-backend.ts:read")
        }
        "live_direct_only_label_membership_not_qualified" => {
            ("contract_drift", "src/native-pool-backend.ts:read")
        }
        "operator_socket_identity_changed" => ("contract_drift", "src/native-pool-backend.ts:read"),
        "operator_reader_identity_mismatch" => {
            ("contract_drift", "src/native-pool-backend.ts:read")
        }
        "operator_hello_invalid" => ("contract_drift", "src/native-pool-backend.ts:read"),
        "native_pool_read_timeout" => ("upstream_unavailable", "src/native-pool-backend.ts:read"),
        "native_pool_read_start_failed" => {
            ("upstream_unavailable", "src/native-pool-backend.ts:read")
        }
        "native_pool_result_size_limit" => {
            ("upstream_unavailable", "src/native-pool-backend.ts:read")
        }
        "native_pool_read_unqualified" => {
            ("upstream_unavailable", "src/native-pool-backend.ts:read")
        }
        "native_pool_result_invalid" => ("upstream_unavailable", "src/native-pool-backend.ts:read"),
        "native_child_no_valid_result" => {
            ("upstream_unavailable", "src/native-pool-backend.ts:read")
        }
        "operator_transport_failed" => ("upstream_unavailable", "src/native-pool-backend.ts:read"),
        "operator_socket_unavailable" => {
            ("upstream_unavailable", "src/native-pool-backend.ts:read")
        }
        "operator_socket_closed" => ("upstream_unavailable", "src/native-pool-backend.ts:read"),
        "operator_reply_invalid" => ("upstream_unavailable", "src/native-pool-backend.ts:read"),
        "source_failure_latched_no_retry" => {
            ("upstream_unavailable", "src/native-pool-backend.ts:read")
        }
        "tool_input_invalid" => ("input_validation", "src/tool-audit.ts:inputIssues"),
        "duplicate_proposal_field" => ("input_validation", "src/tool-audit.ts:inputIssues"),
        "exact_backend_observation_handle_required" => {
            ("input_validation", "src/tool-audit.ts:inputIssues")
        }
        "tool_not_registered" => (
            "endpoint_absence",
            "src/native-dailyrecord-backend.ts:sourcePort",
        ),
        "dailyrecord_read_backend_not_configured" => (
            "endpoint_absence",
            "src/native-dailyrecord-backend.ts:sourcePort",
        ),
        "dailyrecord_save_backend_not_configured" => (
            "endpoint_absence",
            "src/native-dailyrecord-backend.ts:sourcePort",
        ),
        "textbook_catalog_source_port_unavailable" => (
            "endpoint_absence",
            "src/native-dailyrecord-backend.ts:sourcePort",
        ),
        "textbook_pdf_source_port_unavailable" => (
            "endpoint_absence",
            "src/native-dailyrecord-backend.ts:sourcePort",
        ),
        "prestudy_source_port_unavailable" => (
            "endpoint_absence",
            "src/native-dailyrecord-backend.ts:sourcePort",
        ),
        "textbook_binding_source_port_unavailable" => (
            "endpoint_absence",
            "src/native-dailyrecord-backend.ts:sourcePort",
        ),
        "textbook_index_source_port_unavailable" => (
            "endpoint_absence",
            "src/native-dailyrecord-backend.ts:sourcePort",
        ),
        "cism_source_port_unavailable" => (
            "endpoint_absence",
            "src/native-dailyrecord-backend.ts:sourcePort",
        ),
        "textbook_student_not_admitted" => {
            ("policy_denial", "src/native-dailyrecord-backend.ts:scope")
        }
        "prestudy_student_not_admitted" => {
            ("policy_denial", "src/native-dailyrecord-backend.ts:scope")
        }
        "textbook_catalog_principal_rejected" => {
            ("policy_denial", "src/native-dailyrecord-backend.ts:scope")
        }
        "cism_scope_conflict" => ("policy_denial", "src/native-dailyrecord-backend.ts:scope"),
        "cism_review_required" => ("policy_denial", "src/native-dailyrecord-backend.ts:scope"),
        "cism_store_unqualified" => ("policy_denial", "src/native-dailyrecord-backend.ts:scope"),
        "tool_audit_store_unqualified" => {
            ("policy_denial", "src/native-dailyrecord-backend.ts:scope")
        }
        "foreign_source_redirect_rejected" => {
            ("policy_denial", "src/native-dailyrecord-backend.ts:scope")
        }
        "source_principal_drift" => ("policy_denial", "src/native-dailyrecord-backend.ts:scope"),
        "textbook_binding_missing_or_ambiguous" => (
            "identity_ambiguity",
            "src/native-dailyrecord-backend.ts:textbookBinding",
        ),
        "prestudy_authentication_required" => ("authentication", "source-owner:authentication"),
        "textbook_catalog_authentication_required" => {
            ("authentication", "source-owner:authentication")
        }
        "source_login_challenge" => ("authentication", "source-owner:authentication"),
        "fresh_authentication_attempt_required" => {
            ("authentication", "source-owner:authentication")
        }
        "prestudy_scope_conflict" => ("contract_drift", "source-owner:response-contract"),
        "textbook_section_scope_conflict" => ("contract_drift", "source-owner:response-contract"),
        "textbook_binding_scope_conflict" => ("contract_drift", "source-owner:response-contract"),
        "textbook_binding_source_unqualified" => {
            ("contract_drift", "source-owner:response-contract")
        }
        "textbook_section_result_unqualified" => {
            ("contract_drift", "source-owner:response-contract")
        }
        "native_metadata_history_scope_conflict" => {
            ("contract_drift", "source-owner:response-contract")
        }
        "native_metadata_paper_conflict" => ("contract_drift", "source-owner:response-contract"),
        "native_metadata_lookup_scope_conflict" => {
            ("contract_drift", "source-owner:response-contract")
        }
        "native_metadata_operation_conflict" => {
            ("contract_drift", "source-owner:response-contract")
        }
        "native_metadata_inventory_conflict" => {
            ("contract_drift", "source-owner:response-contract")
        }
        "native_dailyrecord_previous_conflict" => {
            ("contract_drift", "source-owner:response-contract")
        }
        "native_dailyrecord_owner_admission_mismatch" => {
            ("contract_drift", "source-owner:response-contract")
        }
        "prestudy_search_form_missing" => ("contract_drift", "source-owner:response-contract"),
        "prestudy_search_form_drift" => ("contract_drift", "source-owner:response-contract"),
        "cism_wire_drift" => ("contract_drift", "source-owner:response-contract"),
        "cism_period_conflict" => ("contract_drift", "source-owner:response-contract"),
        "cism_fields_unqualified" => ("contract_drift", "source-owner:response-contract"),
        "cism_report_drift" => ("contract_drift", "source-owner:response-contract"),
        "cism_stale_before" => ("contract_drift", "source-owner:response-contract"),
        "cism_existing_value_preserved" => ("contract_drift", "source-owner:response-contract"),
        "cism_prior_review_mismatch" => ("contract_drift", "source-owner:response-contract"),
        "cism_duplicate_field" => ("contract_drift", "source-owner:response-contract"),
        "cism_request_rebound" => ("contract_drift", "source-owner:response-contract"),
        "source_redirect_contract_rejected" => ("contract_drift", "source-owner:response-contract"),
        "source_redirect_rejected" => ("contract_drift", "source-owner:response-contract"),
        "login_wire_contract_drift" => ("contract_drift", "source-owner:response-contract"),
        "teacher_user_principal_conflict" => ("contract_drift", "source-owner:response-contract"),
        "teacher_course_principal_conflict" => ("contract_drift", "source-owner:response-contract"),
        "teacher_college_conflict" => ("contract_drift", "source-owner:response-contract"),
        "teacher_member_type_missing" => ("contract_drift", "source-owner:response-contract"),
        "teacher_logout_marker_drift" => ("contract_drift", "source-owner:response-contract"),
        "native_dailyrecord_previous_unverified" => ("partial_coverage", "source-owner:coverage"),
        "prestudy_result_too_large" => ("partial_coverage", "source-owner:coverage"),
        "textbook_section_result_too_large" => ("partial_coverage", "source-owner:coverage"),
        "textbook_section_scan_limit" => ("partial_coverage", "source-owner:coverage"),
        "item_answer_result_too_large" => ("partial_coverage", "source-owner:coverage"),
        "item_section_trace_result_too_large" => ("partial_coverage", "source-owner:coverage"),
        "prestudy_source_rejected" => ("malformed_response", "source-owner:response-contract"),
        "prestudy_row_unqualified" => ("malformed_response", "source-owner:response-contract"),
        "prestudy_summary_row_unqualified" => {
            ("malformed_response", "source-owner:response-contract")
        }
        "cism_date_rejected" => ("malformed_response", "source-owner:response-contract"),
        "cism_month_rejected" => ("malformed_response", "source-owner:response-contract"),
        "cism_document_rejected" => ("malformed_response", "source-owner:response-contract"),
        "cism_body_rejected" => ("malformed_response", "source-owner:response-contract"),
        "cism_value_rejected" => ("malformed_response", "source-owner:response-contract"),
        "cism_ack_unqualified" => ("malformed_response", "source-owner:response-contract"),
        "cism_ack_rejected" => ("malformed_response", "source-owner:response-contract"),
        "cism_readback_mismatch" => ("malformed_response", "source-owner:response-contract"),
        "tool_handler_failed" => ("upstream_unavailable", "academy-mcp:execution-boundary"),
        "upstream_tool_error" => ("upstream_unavailable", "academy-mcp:execution-boundary"),
        "upstream_result_missing" => ("upstream_unavailable", "academy-mcp:execution-boundary"),
        "source_http_failed" => ("upstream_unavailable", "academy-mcp:execution-boundary"),
        "source_cause_unclassified" => ("upstream_unavailable", "academy-mcp:execution-boundary"),
        "cism_uncertain_dispatch" => ("upstream_unavailable", "academy-mcp:execution-boundary"),
        "cism_source_failed" => ("upstream_unavailable", "academy-mcp:execution-boundary"),
        "authentication_required" => ("authentication", "source-owner:authentication-boundary"),
        "daily_history_selector_drift" => {
            ("contract_drift", "source-owner:daily-history-selectors")
        }
        "student_paper_search_wire_drift" => {
            ("contract_drift", "source-owner:student-paper-search")
        }
        "student_paper_owner_drift" => ("policy_denial", "source-owner:student-paper-principal"),
        "daily_search_principal_conflict" => {
            ("policy_denial", "source-owner:daily-search-principal")
        }
        "result_list_missing_or_ambiguous" => {
            ("malformed_response", "source-owner:assessment-result-list")
        }
        "daily_search_response_rejected" => {
            ("malformed_response", "source-owner:daily-search-response")
        }
        "item_transport_failed" => ("upstream_unavailable", "source-owner:item-answer-read"),
        "item_answer_missing_or_schema_drift" => {
            ("contract_drift", "source-owner:item-answer-read")
        }
        "item_answer_read_failed" => ("upstream_unavailable", "source-owner:item-answer-read"),
        "native_name_search_unqualified_no_automatic_retry" => (
            "malformed_response",
            "src/native-dailyrecord-backend.ts:searchStudentsByName",
        ),
        "source_authentication_failed" => (
            "authentication",
            "src/native-dailyrecord-backend.ts:#authenticate",
        ),
        "native_metadata_scope_refused" => (
            "policy_denial",
            "src/native-dailyrecord-backend.ts:readStudentMetadata",
        ),
        "metadata_namespace_not_admitted" => (
            "policy_denial",
            "src/native-dailyrecord-backend.ts:readStudentMetadataByName",
        ),
        "native_dailyrecord_scope_refused" => (
            "policy_denial",
            "src/native-dailyrecord-backend.ts:readDailyRecord",
        ),
        "native_dated_name_ambiguous" => (
            "identity_ambiguity",
            "src/native-dailyrecord-backend.ts:searchStudentsByName",
        ),
        "native_dated_name_scope_conflict" => (
            "contract_drift",
            "src/native-dailyrecord-backend.ts:searchStudentsByName",
        ),
        "student_name_invalid" => (
            "input_validation",
            "src/student-resolver.ts:resolveStudentByName",
        ),
        "student_resolve_backend_not_configured" => (
            "endpoint_absence",
            "src/student-resolver.ts:resolveStudentByName",
        ),
        "student_name_not_admitted" => (
            "policy_denial",
            "src/student-resolver.ts:resolveStudentByName",
        ),
        "student_name_scope_conflict" => (
            "contract_drift",
            "src/student-resolver.ts:resolveStudentByName",
        ),
        "student_name_not_found" => (
            "partial_coverage",
            "src/student-resolver.ts:resolveStudentByName",
        ),
        "student_name_ambiguous" => (
            "identity_ambiguity",
            "src/student-resolver.ts:resolveStudentByName",
        ),
        "student_resolve_search_unqualified_no_automatic_retry" => (
            "upstream_unavailable",
            "src/student-resolver.ts:resolveStudentByName",
        ),
        "student_resolve_unqualified_no_automatic_retry" => {
            ("upstream_unavailable", "src/dailyrecord.ts:resolveStudent")
        }
        "script_digest_drift" => ("contract_drift", "source-owner:script-digest"),
        "native_dailyrecord_identity_conflict" => (
            "contract_drift",
            "src/native-dailyrecord-backend.ts:readDailyRecord",
        ),
        "native_dailyrecord_state_conflict" => (
            "contract_drift",
            "src/native-dailyrecord-backend.ts:readDailyRecord",
        ),
        "native_metadata_row_identity_conflict" => (
            "contract_drift",
            "src/native-dailyrecord-backend.ts:projectStudentMetadata",
        ),
        "native_metadata_period_conflict" => (
            "contract_drift",
            "src/native-dailyrecord-backend.ts:projectStudentMetadata",
        ),
        "native_metadata_selector_conflict" => (
            "contract_drift",
            "src/native-dailyrecord-backend.ts:projectStudentMetadata",
        ),
        "native_dailyrecord_occurrence_unverified" => (
            "partial_coverage",
            "src/native-dailyrecord-backend.ts:readDailyRecord",
        ),
        "native_metadata_method_unavailable" => (
            "endpoint_absence",
            "src/native-dailyrecord-backend.ts:readStudentMetadata",
        ),
        "student_metadata_backend_not_configured" => {
            ("endpoint_absence", "src/dailyrecord.ts:metadataByName")
        }
        "student_metadata_unqualified_no_automatic_retry" => {
            ("upstream_unavailable", "src/dailyrecord.ts:metadataByName")
        }
        "source_response_malformed" => ("malformed_response", "academy-mcp:response-contract"),
        "unknown_tool" => ("endpoint_absence", "src/neutral-server.ts:call"),
        "paper_scope_unresolved" => (
            "policy_denial",
            "src/paper-scope-routing.ts:selectPaperRoute",
        ),
        "release_pointer_missing" => ("contract_drift", "src/neutral-server.ts:activeRelease"),
        "release_upstream_admission_mismatch" => ("contract_drift", "src/neutral-server.ts:call"),
        "release_payload_unqualified" => (
            "contract_drift",
            "src/runtime-release.ts:loadRuntimeRelease",
        ),
        "release_endpoint_identity_mismatch" => (
            "contract_drift",
            "src/runtime-release.ts:verifyEndpointIdentity",
        ),
        "release_bridge_identity_mismatch" => (
            "contract_drift",
            "src/runtime-release.ts:loadRuntimeRelease",
        ),
        "release_artifact_mismatch" => (
            "contract_drift",
            "src/runtime-release.ts:verifyReleaseArtifacts",
        ),
        "release_pointer_changed" => ("contract_drift", "src/runtime-release.ts:readPointer"),
        "release_base_unqualified" => ("policy_denial", "src/runtime-release.ts:readPointer"),
        "release_pointer_unqualified" => ("policy_denial", "src/runtime-release.ts:readPointer"),
        "label_snapshot_hash_mismatch" => (
            "contract_drift",
            "src/item-label-store.ts:readPinnedLocalBytes",
        ),
        "observation_ticket_expired_or_foreign" => ("policy_denial", "src/neutral-server.ts:call"),
        "fresh_before_state_changed_reconcile" => ("contract_drift", "src/neutral-server.ts:call"),
        "dailyrecord_route_upstream_failed_no_retry" => {
            ("upstream_unavailable", "src/neutral-server.ts:call")
        }
        "dailytest_header_upstream_not_configured" => {
            ("endpoint_absence", "src/neutral-server.ts:call")
        }
        "dailytest_header_scope_conflict" => (
            "contract_drift",
            "src/dailytest-header-route.ts:readDailyTestHeaders",
        ),
        "dailytest_header_identity_conflict" => (
            "contract_drift",
            "src/dailytest-header-route.ts:readDailyTestHeaders",
        ),
        "dailytest_header_duplicate_field" => (
            "contract_drift",
            "src/dailytest-header-route.ts:readDailyTestHeaders",
        ),
        "dailytest_header_selection_changed" => (
            "contract_drift",
            "src/dailytest-header-route.ts:readDailyTestHeaders",
        ),
        "dailytest_header_offset_out_of_range" => (
            "policy_denial",
            "src/dailytest-header-route.ts:readDailyTestHeaders",
        ),
        "dailytest_header_result_too_large" => (
            "partial_coverage",
            "src/dailytest-header-route.ts:readDailyTestHeaders",
        ),
        _ => return None,
    })
}

pub fn safe_reason(value: &Js) -> Js {
    let code = value.code_units();
    if value.string() && catalog(&code).is_some() {
        value.clone()
    } else {
        "source_read_unavailable".into()
    }
}

/// Finite original publicReadFailure projection. Native errors have no observed
/// TypeScript stack; retained properties cannot supply origin or class authority.
pub fn public_read_failure(error: &Fault, fallback: &str) -> Js {
    project(error, fallback, 0)
}
fn project(error: &Fault, fallback: &str, depth: usize) -> Js {
    use crate::js::TrustedContext;
    let authentication = matches!(
        error.trusted_context,
        Some(TrustedContext::Authentication { .. })
    );
    let malformed = error.name == "ZodError" || error.name == "SyntaxError";
    let known = catalog(&error.message);
    let (code, category, locator) = if authentication {
        (
            "source_authentication_failed",
            "authentication",
            "src/native-dailyrecord-backend.ts:#authenticate",
        )
    } else if malformed {
        (
            "source_response_malformed",
            "malformed_response",
            "academy-mcp:response-contract",
        )
    } else if let Some((category, locator)) = known {
        (error.message.as_str(), category, locator)
    } else {
        (
            fallback,
            "upstream_unavailable",
            "academy-mcp:owner-read-boundary",
        )
    };
    let kind = match error.name.as_str() {
        "TypeError" | "RangeError" | "SyntaxError" => error.name.as_str(),
        _ => "Error",
    };
    let mut failure = obj(&[
        ("category", category.into()),
        ("code", code.into()),
        ("evidenceLocator", locator.into()),
        ("automaticRetryAllowed", false.into()),
        ("errorKind", kind.into()),
        (
            "errorSha256",
            sha(&format!("{}\0{}", error.name, error.message)).into(),
        ),
    ]);
    if authentication {
        if depth < 2
            && let Some(TrustedContext::Authentication { cause: Some(cause) }) =
                &error.trusted_context
        {
            failure.set(
                "cause",
                project(cause, "source_cause_unclassified", depth + 1),
            );
        }
    } else if !malformed && known.is_some() {
        if let Js::Num(status) = error.properties.get("httpStatus")
            && status.is_finite()
            && status.fract() == 0.0
            && (400.0..=599.0).contains(status)
        {
            failure.set("sourceHttpStatus", Js::Num(*status));
        }
        let operation = error.properties.get("sourceOperation");
        if operation.string()
            && [
                "login_document",
                "login_submission",
                "owned_source_request",
                "cism_weekly_read",
                "cism_field_save",
                "textbook_answer_catalog",
                "getTextBookProgress",
                "day_record_group_selector",
            ]
            .iter()
            .any(|allowed| operation.is(allowed))
        {
            failure.set("sourceOperation", operation.clone());
        }
    }
    failure
}
/// Existing read boundary keeps its original default fallback and envelope.
pub fn unavailable(error: &Fault) -> Js {
    let failure = public_read_failure(error, "dailyrecord_source_unqualified_no_automatic_retry");
    obj(&[
        ("status", "unavailable".into()),
        ("reasonCode", failure.get("code").clone()),
        ("failure", failure),
    ])
}
