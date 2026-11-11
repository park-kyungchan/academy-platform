//! E10a-1 synthetic textbook catalogue/PDF tests. Invented pages, URLs and
//! PDF bytes only; no source or student originals, no network.
use edu_academy_sources::{
    data::{Error, Result, hash},
    http::{self, Response},
    owner_admission::{TransportAdmission, TransportScope},
    student_course::{ReadPort, read_contracts},
    textbook::*,
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    path::PathBuf,
    rc::Rc,
};

const NOW: &str = "2040-01-01T00:00:00Z";
const PRINCIPAL: &str = "Fictional Teacher";
const URL: &str = "https://storage.studyq.net/data/answer/mi/book/g7_fictionalplus_sample_1_1.pdf";
const CATALOG: &str = include_str!("fixtures/textbook/catalog-middle.html");
const ROWSPAN: &str = include_str!("fixtures/textbook/catalog-rowspan.html");
const PDF: &[u8] = include_bytes!("fixtures/textbook/sample.pdf.txt");

fn document_ref(level: &str, url: &str) -> String {
    hash(serde_json::json!([level, url]).to_string().as_bytes())
}
fn code(error: Error) -> String {
    error.message
}
fn response(status: u16, content_type: &str, bytes: Vec<u8>) -> TextbookResponse {
    TextbookResponse {
        status,
        headers: vec![("content-type".into(), content_type.into())],
        bytes,
        final_url: None,
        redirected: false,
        credentials_sent: false,
        redirect_followed: false,
    }
}
fn catalog_response() -> TextbookResponse {
    response(200, "text/html; charset=UTF-8", CATALOG.as_bytes().to_vec())
}

#[derive(Default)]
struct Fake {
    catalog: VecDeque<Result<TextbookResponse>>,
    storage: VecDeque<Result<TextbookResponse>>,
    catalog_calls: Rc<Cell<usize>>,
    storage_urls: Rc<RefCell<Vec<String>>>,
    catalog_applied: Option<(bool, bool)>,
    storage_applied: Option<(bool, bool)>,
    storage_max_seen: Rc<RefCell<Vec<usize>>>,
}
impl TextbookPort for Fake {
    fn catalog(
        &mut self,
        _level: SchoolLevel,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse> {
        // The port contract carries the real bound and the session policy.
        assert_eq!(max_bytes, catalog_body_limit());
        assert_eq!(policy, CATALOG_FETCH);
        self.catalog_calls.set(self.catalog_calls.get() + 1);
        let mut response = self.catalog.pop_front().expect("unexpected catalog call")?;
        let (credentials, redirect) = self.catalog_applied.unwrap_or((true, false));
        response.credentials_sent = credentials;
        response.redirect_followed = redirect;
        Ok(response)
    }
    fn storage(
        &mut self,
        url: &str,
        max_bytes: usize,
        policy: FetchPolicy,
    ) -> Result<TextbookResponse> {
        assert_eq!(policy, STORAGE_FETCH);
        assert!(
            max_bytes == pdf_body_limit(DocumentKind::Sample)
                || max_bytes == pdf_body_limit(DocumentKind::Answer)
        );
        self.storage_max_seen.borrow_mut().push(max_bytes);
        self.storage_urls.borrow_mut().push(url.into());
        let mut response = self.storage.pop_front().expect("unexpected storage call")?;
        let (credentials, redirect) = self.storage_applied.unwrap_or((false, false));
        response.credentials_sent = credentials;
        response.redirect_followed = redirect;
        Ok(response)
    }
}
fn make_source(fake: Fake) -> TextbookSource<Fake> {
    TextbookSource::new(fake, PRINCIPAL)
}

/// Ports `textbookCatalogRead.test.ts:24` (shared links/labels, no edition claim).
#[test]
fn catalog_preserves_shared_links_and_labels() {
    let value = parse_catalog(CATALOG, SchoolLevel::Middle, NOW).unwrap();
    assert_eq!(value.entries.len(), 1);
    let entry = &value.entries[0];
    assert_eq!(entry.kind, DocumentKind::Sample);
    assert_eq!(entry.document_ref, document_ref("middle", URL));
    assert_eq!(entry.url, URL);
    assert_eq!(entry.filename, "g7_fictionalplus_sample_1_1.pdf");
    assert_eq!(
        entry.source_labels,
        [
            "Invented Plus middle1 term1 version2 sample",
            "Same shared PDF; other printed label sample"
        ]
    );
    assert_eq!(
        value.coverage,
        "all_observed_pdf_links_in_one_catalog_document"
    );
    assert_eq!(value.catalog_sha256.len(), 64);
    assert_eq!(value.source_requests, 1);
}

/// Ports `textbookCatalogRead.test.ts:29-30` and the session-page refusal.
#[test]
fn catalog_parse_refusals_fail_closed() {
    assert_eq!(
        code(
            parse_catalog(
                "<div class=\"tab tab-on\">중등부</div><table>nothing downloaded</table>",
                SchoolLevel::Middle,
                NOW
            )
            .unwrap_err()
        ),
        "textbook_catalog_pdf_links_unqualified"
    );
    let password = format!("<input type=password>{CATALOG}");
    assert_eq!(
        code(parse_catalog(&password, SchoolLevel::Middle, NOW).unwrap_err()),
        "textbook_catalog_authentication_required"
    );
    let expired = format!("normal_session_error.jsp{CATALOG}");
    assert_eq!(
        code(parse_catalog(&expired, SchoolLevel::Middle, NOW).unwrap_err()),
        "textbook_catalog_authentication_required"
    );
    // A row label over 2000 UTF-16 units is never guessed or truncated.
    let long_label = "x".repeat(2001);
    let unlabeled = format!(
        "<div class=\"tab tab-on\">중등부</div><table><tr><td>{long_label}</td><td><button onclick=\"fullUrlDownloadFile('https://storage.studyq.net/data/answer/mi/book/g7_fictionalplus_sample_1_1.pdf')\">sample</button></td></tr></table>"
    );
    assert_eq!(
        code(parse_catalog(&unlabeled, SchoolLevel::Middle, NOW).unwrap_err()),
        "textbook_catalog_row_unqualified"
    );
}

/// Ports `textbookCatalogRead.test.ts:38` (rowspans, workbook answer, level).
#[test]
fn catalog_rowspans_retain_labels_and_answer_kind() {
    let value = parse_catalog(ROWSPAN, SchoolLevel::Middle, NOW).unwrap();
    assert_eq!(value.entries.len(), 1);
    assert_eq!(value.entries[0].kind, DocumentKind::Answer);
    assert_eq!(value.entries[0].source_labels, ["중1 Invented Plus sample"]);
    assert_eq!(
        code(parse_catalog(ROWSPAN, SchoolLevel::Elementary, NOW).unwrap_err()),
        "textbook_catalog_school_level_rejected"
    );
}

/// Ports `textbookCatalogRead.test.ts:32` (foreign host/path/query/credentials/traversal).
#[test]
fn qualified_url_refusals() {
    for bad in [
        URL.replace("storage.studyq.net", "evil.example"),
        format!("{URL}?download=1"),
        format!("{URL}#page=1"),
        URL.replace("/book/", "/other/"),
        URL.replace("https://", "https://name:secret@"),
        URL.replace("g7_fictionalplus", "..%2fg7_fictionalplus"),
        URL.replace("https://", "http://"),
    ] {
        assert_eq!(
            code(qualified_pdf_url(&bad).unwrap_err()),
            "textbook_pdf_link_rejected",
            "{bad}"
        );
    }
    assert!(
        qualified_pdf_url(
            "https://storage.studyq.net/data/answer/mi/fiveend/g_fictional_total.pdf"
        )
        .unwrap()
        .contains("/fiveend/")
    );
    assert_eq!(qualified_pdf_url(URL).unwrap(), URL);
}

/// Ports `textbookCatalogRead.test.ts:61-68` at explicit small bounds:
/// at the bound passes, one over and declared-length mismatches refuse,
/// and a compressed declared length is not compared to decoded bytes.
#[test]
fn bounded_body_bounds_and_signatures() {
    let ok = bounded_body(
        response(200, "application/pdf", PDF.to_vec()),
        PDF.len(),
        BodyKind::Pdf,
    )
    .unwrap();
    assert_eq!(ok, PDF);
    let mut over = PDF.to_vec();
    over.push(b'x');
    assert_eq!(
        code(
            bounded_body(
                response(200, "application/pdf", over),
                PDF.len(),
                BodyKind::Pdf
            )
            .unwrap_err()
        ),
        "textbook_pdf_size_rejected"
    );
    let mut declared = response(200, "application/pdf", PDF.to_vec());
    declared.headers.push(("content-length".into(), "8".into()));
    assert_eq!(
        code(bounded_body(declared.clone(), 1000, BodyKind::Pdf).unwrap_err()),
        "textbook_pdf_size_rejected"
    );
    declared
        .headers
        .push(("content-encoding".into(), "gzip".into()));
    assert_eq!(bounded_body(declared, 1000, BodyKind::Pdf).unwrap(), PDF);
    assert_eq!(
        code(
            bounded_body(
                response(200, "application/pdf", b"not pdf".to_vec()),
                1000,
                BodyKind::Pdf
            )
            .unwrap_err()
        ),
        "textbook_pdf_signature_rejected"
    );
    assert_eq!(
        code(
            bounded_body(
                response(302, "application/pdf", PDF.to_vec()),
                1000,
                BodyKind::Pdf
            )
            .unwrap_err()
        ),
        "textbook_pdf_response_rejected"
    );
    assert_eq!(
        code(
            bounded_body(
                response(200, "text/html", PDF.to_vec()),
                1000,
                BodyKind::Pdf
            )
            .unwrap_err()
        ),
        "textbook_pdf_content_type_rejected"
    );
    assert_eq!(
        code(
            bounded_body(
                response(200, "application/pdf", PDF.to_vec()),
                1000,
                BodyKind::Catalog
            )
            .unwrap_err()
        ),
        "textbook_catalog_content_type_rejected"
    );
}

/// Ports `textbookCatalogRead.test.ts:46,50` (principal binding, zero calls)
/// and the tool filter/paging of `textbook-source-tools.ts:78-80`.
#[test]
fn catalog_principal_filter_and_paging() {
    let mut fake = Fake::default();
    fake.catalog.push_back(Ok(catalog_response()));
    let catalog_calls = fake.catalog_calls.clone();
    let mut textbook_source = make_source(fake);
    assert_eq!(
        code(
            textbook_source
                .read_catalog("Foreign Teacher", SchoolLevel::Middle, NOW)
                .unwrap_err()
        ),
        "textbook_catalog_principal_rejected"
    );
    assert_eq!(catalog_calls.get(), 0);
    let value = textbook_source
        .read_catalog(PRINCIPAL, SchoolLevel::Middle, NOW)
        .unwrap();
    let page = page_catalog(
        &value,
        CatalogPageRequest {
            document_kind: Some(DocumentKind::Sample),
            query: Some("plus".into()),
        },
        0,
        10,
    )
    .unwrap();
    assert_eq!(page.total_observed_pdf_links, 1);
    assert_eq!(page.total_matched, 1);
    assert_eq!(page.returned, 1);
    assert_eq!(page.next_offset, None);
    let none = page_catalog(
        &value,
        CatalogPageRequest {
            document_kind: Some(DocumentKind::Answer),
            query: None,
        },
        0,
        10,
    )
    .unwrap();
    assert_eq!(none.total_matched, 0);
    assert_eq!(
        code(page_catalog(&value, CatalogPageRequest::default(), 1001, 10).unwrap_err()),
        "tool_input_invalid"
    );
}

/// Ports `textbookCatalogRead.test.ts:46-49`: a forged reference never
/// reaches storage; an invalid reference never reaches the catalogue.
#[test]
fn pdf_forged_or_invalid_reference_makes_no_storage_call() {
    let mut fake = Fake::default();
    fake.catalog.push_back(Ok(catalog_response()));
    let storage_urls = fake.storage_urls.clone();
    let catalog_calls = fake.catalog_calls.clone();
    let mut textbook_source = make_source(fake);
    assert_eq!(
        code(
            textbook_source
                .fetch_pdf(PRINCIPAL, SchoolLevel::Middle, &"0".repeat(64), NOW)
                .unwrap_err()
        ),
        "textbook_pdf_not_in_current_catalog"
    );
    assert!(storage_urls.borrow().is_empty());
    assert_eq!(
        code(
            textbook_source
                .fetch_pdf(PRINCIPAL, SchoolLevel::Middle, "bad", NOW)
                .unwrap_err()
        ),
        "textbook_pdf_input_rejected"
    );
    assert_eq!(catalog_calls.get(), 1);
}

/// Ports `textbookCatalogRead.test.ts:53` at the port boundary: the storage
/// fetch is addressed only to the qualified URL the catalogue observed.
#[test]
fn pdf_fetch_binds_catalog_entry_and_digest() {
    let mut fake = Fake::default();
    fake.catalog.push_back(Ok(catalog_response()));
    fake.storage
        .push_back(Ok(response(200, "application/pdf", PDF.to_vec())));
    let storage_urls = fake.storage_urls.clone();
    let mut textbook_source = make_source(fake);
    let pdf = textbook_source
        .fetch_pdf(
            PRINCIPAL,
            SchoolLevel::Middle,
            &document_ref("middle", URL),
            NOW,
        )
        .unwrap();
    assert_eq!(storage_urls.borrow().as_slice(), [URL]);
    assert_eq!(pdf.meta.sha256, hash(PDF));
    assert_eq!(pdf.meta.byte_length, PDF.len());
    assert_eq!(pdf.meta.source_requests, 2);
    assert_eq!(pdf.meta.kind, DocumentKind::Sample);
    assert_eq!(textbook_source.requests(), 2);
}

/// Ports `textbookCatalogRead.test.ts:61` response refusals through fetch.
#[test]
fn pdf_fetch_response_refusals() {
    for (storage_response, expected) in [
        (
            response(302, "application/pdf", vec![]),
            "textbook_pdf_response_rejected",
        ),
        (
            response(200, "text/html", b"<html>login</html>".to_vec()),
            "textbook_pdf_content_type_rejected",
        ),
        (
            response(200, "application/pdf", b"not pdf".to_vec()),
            "textbook_pdf_signature_rejected",
        ),
    ] {
        let mut fake = Fake::default();
        fake.catalog.push_back(Ok(catalog_response()));
        fake.storage.push_back(Ok(storage_response));
        let mut textbook_source = make_source(fake);
        assert_eq!(
            code(
                textbook_source
                    .fetch_pdf(
                        PRINCIPAL,
                        SchoolLevel::Middle,
                        &document_ref("middle", URL),
                        NOW
                    )
                    .unwrap_err()
            ),
            expected
        );
    }
    // A final URL different from the qualified link is refused by name.
    let mut fake = Fake::default();
    fake.catalog.push_back(Ok(catalog_response()));
    let mut redirected = response(200, "application/pdf", PDF.to_vec());
    redirected.final_url = Some("https://storage.studyq.net/other.pdf".into());
    fake.storage.push_back(Ok(redirected));
    let mut textbook_source = make_source(fake);
    assert_eq!(
        code(
            textbook_source
                .fetch_pdf(
                    PRINCIPAL,
                    SchoolLevel::Middle,
                    &document_ref("middle", URL),
                    NOW
                )
                .unwrap_err()
        ),
        "textbook_pdf_response_url_rejected"
    );
    // A transport failure is reported as transport, never as a result.
    let mut fake = Fake::default();
    fake.catalog.push_back(Ok(catalog_response()));
    fake.storage
        .push_back(Err(Error::refusal("socket exploded")));
    let mut textbook_source = make_source(fake);
    assert_eq!(
        code(
            textbook_source
                .fetch_pdf(
                    PRINCIPAL,
                    SchoolLevel::Middle,
                    &document_ref("middle", URL),
                    NOW
                )
                .unwrap_err()
        ),
        "textbook_pdf_transport_failed"
    );
}

/// Ports `textbookCatalogRead.test.ts:70`: an answer PDF above the sample
/// cap is preserved under the separate reference bound.
#[test]
fn pdf_reference_kind_uses_reference_bound() {
    let reference_url =
        "https://storage.studyq.net/data/answer/mi/book/g7_fictionalplus_answer.pdf";
    let html = format!(
        "<div class=\"tab tab-on\">중등부</div><table><tr><td>Invented Plus reference</td><td><button onclick=\"fullUrlDownloadFile('{reference_url}')\">sample</button></td></tr></table>"
    );
    let mut large = vec![b'x'; MAX_SAMPLE_BYTES + 1];
    large[..9].copy_from_slice(b"%PDF-1.7\n");
    let tail = b"\n%%EOF\n";
    let start = large.len() - tail.len();
    large[start..].copy_from_slice(tail);
    let mut fake = Fake::default();
    fake.catalog
        .push_back(Ok(response(200, "text/html", html.as_bytes().to_vec())));
    fake.storage
        .push_back(Ok(response(200, "application/pdf", large.clone())));
    let seen = fake.storage_max_seen.clone();
    let mut textbook_source = make_source(fake);
    let pdf = textbook_source
        .fetch_pdf(
            PRINCIPAL,
            SchoolLevel::Middle,
            &document_ref("middle", reference_url),
            NOW,
        )
        .unwrap();
    assert_eq!(pdf.meta.kind, DocumentKind::Answer);
    assert_eq!(
        seen.borrow().as_slice(),
        [pdf_body_limit(DocumentKind::Answer)]
    );
    assert_eq!(pdf.meta.byte_length, MAX_SAMPLE_BYTES + 1);
    assert_eq!(pdf.meta.sha256, hash(&large));
}

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("textbook-e10a-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    root
}
fn pdf_observation() -> PdfObservation {
    let catalog = parse_catalog(CATALOG, SchoolLevel::Middle, NOW).unwrap();
    let entry = &catalog.entries[0];
    PdfObservation {
        meta: PdfMeta {
            school_level: SchoolLevel::Middle,
            kind: entry.kind,
            document_ref: entry.document_ref.clone(),
            filename: entry.filename.clone(),
            source_url: entry.url.clone(),
            source_labels: entry.source_labels.clone(),
            catalog_sha256: catalog.catalog_sha256.clone(),
            observed_at: NOW.into(),
            sha256: hash(PDF),
            byte_length: PDF.len(),
            source_requests: 2,
        },
        bytes: PDF.to_vec(),
    }
}

/// Ports `textbook-source-tools.test.ts:11`: custody round trip, tamper and
/// target-symlink refusal, digest refusal.
#[test]
fn custody_round_trip_tamper_and_symlink() {
    let root = temp_root("custody");
    let pdf = pdf_observation();
    let first = preserve_pdf(&root, &pdf).unwrap();
    let second = preserve_pdf(&root, &pdf).unwrap();
    assert_eq!(first, second);
    assert_eq!(std::fs::read(&first.path).unwrap(), PDF);
    std::fs::write(&first.path, "tampered").unwrap();
    assert_eq!(
        code(preserve_pdf(&root, &pdf).unwrap_err()),
        "textbook_artifact_readback_rejected"
    );
    std::fs::remove_file(&first.path).unwrap();
    let unrelated = root.join("unrelated");
    std::fs::write(&unrelated, PDF).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&unrelated, &first.path).unwrap();
    assert_eq!(
        code(preserve_pdf(&root, &pdf).unwrap_err()),
        "textbook_artifact_readback_rejected"
    );
    let mut bad = pdf_observation();
    bad.meta.sha256 = "0".repeat(64);
    assert_eq!(
        code(preserve_pdf(&root, &bad).unwrap_err()),
        "textbook_pdf_digest_rejected"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// A symlinked custody root is not the verified owner root.
#[test]
fn custody_symlink_root_refused() {
    let real = temp_root("real-root");
    std::fs::create_dir_all(&real).unwrap();
    let link = temp_root("link-root");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert_eq!(
        code(preserve_pdf(&link, &pdf_observation()).unwrap_err()),
        "textbook_artifact_directory_rejected"
    );
    let _ = std::fs::remove_file(&link);
    let _ = std::fs::remove_dir_all(&real);
}

/// `download_pdf` returns identity, size, digest and the artifact only.
#[test]
fn download_pdf_preserves_and_reports_meta_only() {
    let root = temp_root("download");
    let mut fake = Fake::default();
    fake.catalog.push_back(Ok(catalog_response()));
    fake.storage
        .push_back(Ok(response(200, "application/pdf", PDF.to_vec())));
    let mut textbook_source = make_source(fake);
    let preserved = textbook_source
        .download_pdf(
            PRINCIPAL,
            SchoolLevel::Middle,
            &document_ref("middle", URL),
            &root,
            NOW,
        )
        .unwrap();
    assert_eq!(preserved.meta.sha256, hash(PDF));
    assert_eq!(preserved.artifact.bytes, PDF.len() as u64);
    assert_eq!(std::fs::read(&preserved.artifact.path).unwrap(), PDF);
    let _ = std::fs::remove_dir_all(&root);
}

/// The admitted contracts and the Academy operation list carry the new
/// catalogue entries, and the session port drives the middle contract.
#[test]
fn contracts_and_session_port() {
    let contracts = read_contracts();
    for (id, grade) in [
        ("textbook.catalog", None),
        ("textbook.catalog.elementary", Some("elementary")),
        ("textbook.catalog.high", Some("high")),
    ] {
        let contract = contracts.iter().find(|c| c.id == id).expect(id);
        assert_eq!(contract.method, "GET");
        assert_eq!(contract.path, CATALOG_PATH);
        assert!(!contract.effect);
        match grade {
            None => assert_eq!(contract.query.len(), 1),
            Some(value) => assert!(
                contract
                    .query
                    .iter()
                    .any(|p| p.name == "grade"
                        && matches!(&p.value, edu_academy_sources::owner_admission::ParameterValue::Literal { value: v } if v == value))
            ),
        }
    }
    let operations = http::operations();
    assert!(
        operations
            .array()
            .iter()
            .any(|op| { op.get("selector").is("GaStudyAnswer") && op.get("method").is("GET") })
    );

    struct SessionFake {
        admission: TransportAdmission,
        calls: Rc<Cell<usize>>,
    }
    impl ReadPort for SessionFake {
        fn transport_admission(&self) -> &TransportAdmission {
            &self.admission
        }
        fn read(
            &mut self,
            _scope: &TransportScope,
            operation: &str,
            _args: &[(String, String)],
        ) -> Result<Response> {
            self.calls.set(self.calls.get() + 1);
            assert_eq!(operation, "textbook.catalog");
            Ok(Response {
                status: 200,
                headers: vec![("content-type".into(), "text/html".into())],
                bytes: CATALOG.as_bytes().to_vec(),
                url: http::ORIGIN.into(),
                stalled: false,
            })
        }
    }
    let admission = TransportAdmission {
        schema_version: "academy.transport-admission/1".into(),
        owner_id: "fictional-owner".into(),
        teacher_id: "99001".into(),
        teacher_label: PRINCIPAL.into(),
        college_id: "88001".into(),
        allowed_from: "2040-01-01".into(),
        allowed_through: "2040-01-31".into(),
        student_ids: vec!["77001".into()],
        source_namespaces: vec![edu_academy_sources::student_course::COURSE_NS.into()],
        operations: read_contracts(),
    };
    let scope = TransportScope {
        owner_id: "fictional-owner".into(),
        source_namespace: edu_academy_sources::student_course::COURSE_NS.into(),
        teacher_id: "99001".into(),
        student_id: "77001".into(),
        date: "2040-01-04".into(),
    };
    let calls = Rc::new(Cell::new(0));
    let mut session = SessionFake {
        admission,
        calls: calls.clone(),
    };
    let port = SessionTextbookPort::new(&mut session, scope);
    let mut textbook_source = TextbookSource::new(port, PRINCIPAL);
    let catalog = textbook_source
        .read_catalog(PRINCIPAL, SchoolLevel::Middle, NOW)
        .unwrap();
    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(calls.get(), 1);
    // Storage is never available through the Academy session port.
    let mut session2 = SessionFake {
        admission: TransportAdmission {
            schema_version: "academy.transport-admission/1".into(),
            owner_id: "fictional-owner".into(),
            teacher_id: "99001".into(),
            teacher_label: PRINCIPAL.into(),
            college_id: "88001".into(),
            allowed_from: "2040-01-01".into(),
            allowed_through: "2040-01-31".into(),
            student_ids: vec!["77001".into()],
            source_namespaces: vec![edu_academy_sources::student_course::COURSE_NS.into()],
            operations: read_contracts(),
        },
        calls: Rc::new(Cell::new(0)),
    };
    let scope2 = TransportScope {
        owner_id: "fictional-owner".into(),
        source_namespace: edu_academy_sources::student_course::COURSE_NS.into(),
        teacher_id: "99001".into(),
        student_id: "77001".into(),
        date: "2040-01-04".into(),
    };
    let mut port2 = SessionTextbookPort::new(&mut session2, scope2);
    assert_eq!(
        code(
            port2
                .storage(URL, pdf_body_limit(DocumentKind::Sample), STORAGE_FETCH)
                .unwrap_err()
        ),
        "textbook_pdf_source_port_unavailable"
    );
}

#[test]
fn qualified_url_port_trailing_dot_and_encoded_dot_refused() {
    for bad in [
        "https://storage.studyq.net:8443/data/answer/mi/book/g7_fictionalplus_sample_1_1.pdf",
        "https://storage.studyq.net./data/answer/mi/book/g7_fictionalplus_sample_1_1.pdf",
        "https://storage.studyq.net/data/answer/mi/book/%2e%2e_sample.pdf",
    ] {
        assert_eq!(
            code(qualified_pdf_url(bad).unwrap_err()),
            "textbook_pdf_link_rejected",
            "{bad}"
        );
    }
}

/// P3: the real constants are the bound, proven without allocating them.
#[test]
fn body_limits_are_the_real_constants() {
    assert_eq!(catalog_body_limit(), 2_000_000);
    assert_eq!(pdf_body_limit(DocumentKind::Sample), 32 * 1024 * 1024);
    assert_eq!(pdf_body_limit(DocumentKind::Answer), 64 * 1024 * 1024);
    assert_eq!(pdf_body_limit(DocumentKind::Other), 64 * 1024 * 1024);
}

/// P3/P9: a declared length above the maximum refuses at the real bound
/// (TS textbookCatalogRead.test.ts:62 uses 999999999).
#[test]
fn declared_oversize_refuses_without_large_allocation() {
    let mut fake = Fake::default();
    let mut catalog = catalog_response();
    catalog
        .headers
        .push(("content-length".into(), "999999999".into()));
    fake.catalog.push_back(Ok(catalog));
    let mut textbook_source = make_source(fake);
    assert_eq!(
        code(
            textbook_source
                .read_catalog(PRINCIPAL, SchoolLevel::Middle, NOW)
                .unwrap_err()
        ),
        "textbook_catalog_size_rejected"
    );
    let mut fake = Fake::default();
    fake.catalog.push_back(Ok(catalog_response()));
    let mut storage = response(200, "application/pdf", PDF.to_vec());
    storage.headers.push((
        "content-length".into(),
        (pdf_body_limit(DocumentKind::Sample) + 1).to_string(),
    ));
    fake.storage.push_back(Ok(storage));
    let mut textbook_source = make_source(fake);
    assert_eq!(
        code(
            textbook_source
                .fetch_pdf(
                    PRINCIPAL,
                    SchoolLevel::Middle,
                    &document_ref("middle", URL),
                    NOW
                )
                .unwrap_err()
        ),
        "textbook_pdf_size_rejected"
    );
}

/// P9: every bad link is refused inside a catalogue, not only standalone.
#[test]
fn parse_catalog_refuses_each_bad_link() {
    for bad in [
        URL.replace("storage.studyq.net", "evil.example"),
        format!("{URL}?download=1"),
        format!("{URL}#page=1"),
        URL.replace("/book/", "/other/"),
        URL.replace("https://", "https://name:secret@"),
        URL.replace("g7_fictionalplus", "..%2fg7_fictionalplus"),
        "https://storage.studyq.net:8443/data/answer/mi/book/g7_fictionalplus_sample_1_1.pdf"
            .to_string(),
    ] {
        let html = format!(
            "<div class=\"tab tab-on\">중등부</div><table><tr><td>Invented</td><td><button onclick=\"fullUrlDownloadFile('{bad}')\">sample</button></td></tr></table>"
        );
        assert_eq!(
            code(parse_catalog(&html, SchoolLevel::Middle, NOW).unwrap_err()),
            "textbook_pdf_link_rejected",
            "{bad}"
        );
    }
}

/// P9: session-expired pages, tab count refusals, entry cap, redirected body.
#[test]
fn catalog_session_and_shape_refusals() {
    for marker in ["popup_session_error.jsp", "세션이 만료"] {
        let html = format!("{marker}{CATALOG}");
        assert_eq!(
            code(parse_catalog(&html, SchoolLevel::Middle, NOW).unwrap_err()),
            "textbook_catalog_authentication_required"
        );
    }
    let two_tabs = format!("<div class=\"tab tab-on\">고등부</div>{CATALOG}");
    assert_eq!(
        code(parse_catalog(&two_tabs, SchoolLevel::Middle, NOW).unwrap_err()),
        "textbook_catalog_school_level_rejected"
    );
    let no_tab = "<table><tr><td>x</td></tr></table>";
    assert_eq!(
        code(parse_catalog(no_tab, SchoolLevel::Middle, NOW).unwrap_err()),
        "textbook_catalog_school_level_rejected"
    );
    let mut rows = String::new();
    for i in 0..1001 {
        rows.push_str(&format!(
            "<tr><td>Invented {i}</td><td><button onclick=\"fullUrlDownloadFile('https://storage.studyq.net/data/answer/mi/book/g7_fictionalplus_sample_1_{i}.pdf')\">sample</button></td></tr>"
        ));
    }
    let many = format!("<div class=\"tab tab-on\">중등부</div><table>{rows}</table>");
    assert_eq!(
        code(parse_catalog(&many, SchoolLevel::Middle, NOW).unwrap_err()),
        "textbook_catalog_pdf_links_unqualified"
    );
    let mut redirected = catalog_response();
    redirected.redirected = true;
    assert_eq!(
        code(bounded_body(redirected, catalog_body_limit(), BodyKind::Catalog).unwrap_err()),
        "textbook_catalog_response_rejected"
    );
}

/// P9/P10: paging limits and the trimmed 1-200 query rule.
#[test]
fn page_catalog_query_and_limit_rules() {
    let value = parse_catalog(CATALOG, SchoolLevel::Middle, NOW).unwrap();
    assert_eq!(
        code(page_catalog(&value, CatalogPageRequest::default(), 0, 101).unwrap_err()),
        "tool_input_invalid"
    );
    for query in ["   ".to_string(), "x".repeat(201)] {
        assert_eq!(
            code(
                page_catalog(
                    &value,
                    CatalogPageRequest {
                        document_kind: None,
                        query: Some(query),
                    },
                    0,
                    10
                )
                .unwrap_err()
            ),
            "tool_input_invalid"
        );
    }
    let trimmed = page_catalog(
        &value,
        CatalogPageRequest {
            document_kind: None,
            query: Some("  Plus  ".into()),
        },
        0,
        10,
    )
    .unwrap();
    assert_eq!(trimmed.total_matched, 1);
}

/// P4: an onclick on the row element itself keeps its rowspan label.
#[test]
fn catalog_onclick_on_row_keeps_label() {
    let html = "<div class=\"tab tab-on\">중등부</div><table><tr><td rowspan=\"2\">중1</td><td>Invented ordinary</td></tr><tr onclick=\"fullUrlDownloadFile('https://storage.studyq.net/data/answer/mi/workbook/g7_fictionalplus_answer.pdf')\"><td>Invented Plus</td></tr></table>";
    let value = parse_catalog(html, SchoolLevel::Middle, NOW).unwrap();
    assert_eq!(value.entries[0].source_labels, ["중1 Invented Plus"]);
}

/// P5: the HTML integer rule (`" 2"` and `"2x"` are 2) carries the label.
#[test]
fn catalog_span_attributes_use_html_integer_rule() {
    for span in [" 2", "2x"] {
        let html = format!(
            "<div class=\"tab tab-on\">중등부</div><table><tr><td rowspan=\"{span}\">중1</td><td>Invented ordinary</td></tr><tr><td>Invented Plus</td><td><button onclick=\"fullUrlDownloadFile('https://storage.studyq.net/data/answer/mi/workbook/g7_fictionalplus_answer.pdf')\">sample</button></td></tr></table>"
        );
        let value = parse_catalog(&html, SchoolLevel::Middle, NOW).unwrap();
        assert_eq!(
            value.entries[0].source_labels,
            ["중1 Invented Plus sample"],
            "rowspan={span:?}"
        );
    }
}

/// Declared difference (finding 6): Rust `split_whitespace` does not split
/// on U+FEFF where JS `\s` does, so the character stays inside the label.
#[test]
fn declared_difference_feff_stays_in_label() {
    let html = "<div class=\"tab tab-on\">중등부</div><table><tr><td>Invented\u{feff}Plus</td><td><button onclick=\"fullUrlDownloadFile('https://storage.studyq.net/data/answer/mi/book/g7_fictionalplus_sample_1_1.pdf')\">sample</button></td></tr></table>";
    let value = parse_catalog(html, SchoolLevel::Middle, NOW).unwrap();
    assert!(value.entries[0].source_labels[0].contains('\u{feff}'));
}

/// P6: strict UTF-8 refuses invalid bytes; a BOM is preserved in the hash
/// (the decoded text keeps U+FEFF, unlike TS lossy TextDecoder).
#[test]
fn catalog_utf8_and_bom_results() {
    let mut fake = Fake::default();
    fake.catalog
        .push_back(Ok(response(200, "text/html", vec![0xff, 0xfe, 0x41])));
    let mut textbook_source = make_source(fake);
    assert_eq!(
        code(
            textbook_source
                .read_catalog(PRINCIPAL, SchoolLevel::Middle, NOW)
                .unwrap_err()
        ),
        "textbook_catalog_response_rejected"
    );
    let mut bom_bytes = vec![0xef, 0xbb, 0xbf];
    bom_bytes.extend_from_slice(CATALOG.as_bytes());
    let expected_sha = hash(&bom_bytes);
    let mut fake = Fake::default();
    fake.catalog
        .push_back(Ok(response(200, "text/html", bom_bytes)));
    let mut textbook_source = make_source(fake);
    let value = textbook_source
        .read_catalog(PRINCIPAL, SchoolLevel::Middle, NOW)
        .unwrap();
    assert_eq!(value.catalog_sha256, expected_sha);
    assert_eq!(value.entries.len(), 1);
}

/// P7: same-length tamper, interrupted write, FIFO, signature recheck.
#[test]
fn custody_hardened_cases() {
    let root = temp_root("hardened");
    let pdf = pdf_observation();
    let first = preserve_pdf(&root, &pdf).unwrap();
    // Same-length tamper: digest mismatch at the same size.
    std::fs::write(&first.path, vec![b'X'; PDF.len()]).unwrap();
    assert_eq!(
        code(preserve_pdf(&root, &pdf).unwrap_err()),
        "textbook_artifact_readback_rejected"
    );
    std::fs::remove_file(&first.path).unwrap();
    // A leftover temporary file from an interrupted write is not an obstacle.
    std::fs::write(
        root.join(format!(".{}.pdf.tmp-999999", pdf.meta.sha256)),
        "partial",
    )
    .unwrap();
    let again = preserve_pdf(&root, &pdf).unwrap();
    assert_eq!(std::fs::read(&again.path).unwrap(), PDF);
    // A FIFO at the target name refuses without being opened.
    std::fs::remove_file(&again.path).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&again.path)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(
        code(preserve_pdf(&root, &pdf).unwrap_err()),
        "textbook_artifact_readback_rejected"
    );
    std::fs::remove_file(&again.path).unwrap();
    // Non-PDF bytes never enter custody even with a matching digest.
    let mut not_pdf = pdf_observation();
    not_pdf.bytes = b"not a pdf at all %%EOF".to_vec();
    not_pdf.meta.byte_length = not_pdf.bytes.len();
    not_pdf.meta.sha256 = hash(&not_pdf.bytes);
    assert_eq!(
        code(preserve_pdf(&root, &not_pdf).unwrap_err()),
        "textbook_pdf_signature_rejected"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// P7: a symlinked intermediate directory refuses before creation, and a
/// relative root returns an absolute artifact path.
#[test]
fn custody_intermediate_symlink_and_relative_root() {
    let real = temp_root("intermediate-real");
    std::fs::create_dir_all(&real).unwrap();
    let link_parent = temp_root("intermediate-link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real, &link_parent).unwrap();
    let root = link_parent.join("sub");
    assert_eq!(
        code(preserve_pdf(&root, &pdf_observation()).unwrap_err()),
        "textbook_artifact_directory_rejected"
    );
    assert!(!real.join("sub").exists());
    let _ = std::fs::remove_file(&link_parent);
    let _ = std::fs::remove_dir_all(&real);

    let relative_name = format!("textbook-e10a-relative-{}", std::process::id());
    let artifact = preserve_pdf(std::path::Path::new(&relative_name), &pdf_observation()).unwrap();
    assert!(artifact.path.is_absolute());
    let _ = std::fs::remove_dir_all(&relative_name);
}

/// P8: Debug prints lengths and digests, never bytes or header values.
#[test]
fn debug_output_never_prints_bytes_or_header_values() {
    let pdf_debug = format!("{:?}", pdf_observation());
    assert!(!pdf_debug.contains("synthetic transport fixture"));
    assert!(pdf_debug.contains("bytes_len"));
    let mut response = response(200, "application/pdf", b"secret body bytes".to_vec());
    response
        .headers
        .push(("set-cookie".into(), "secret-cookie-value".into()));
    let response_debug = format!("{:?}", response);
    assert!(!response_debug.contains("secret body bytes"));
    assert!(!response_debug.contains("secret-cookie-value"));
    assert!(response_debug.contains("set-cookie"));
}

/// Q1: a transport that silently sent credentials or followed a redirect
/// is refused with the transport-failure code of that read.
#[test]
fn applied_policy_mismatch_refused() {
    for applied in [(true, false), (false, true)] {
        let mut fake = Fake::default();
        fake.catalog.push_back(Ok(catalog_response()));
        fake.storage
            .push_back(Ok(response(200, "application/pdf", PDF.to_vec())));
        fake.storage_applied = Some(applied);
        let mut textbook_source = make_source(fake);
        assert_eq!(
            code(
                textbook_source
                    .fetch_pdf(
                        PRINCIPAL,
                        SchoolLevel::Middle,
                        &document_ref("middle", URL),
                        NOW
                    )
                    .unwrap_err()
            ),
            "textbook_pdf_transport_failed",
            "applied={applied:?}"
        );
    }
    let mut fake = Fake::default();
    fake.catalog.push_back(Ok(catalog_response()));
    fake.catalog_applied = Some((false, false));
    let mut textbook_source = make_source(fake);
    assert_eq!(
        code(
            textbook_source
                .read_catalog(PRINCIPAL, SchoolLevel::Middle, NOW)
                .unwrap_err()
        ),
        "textbook_catalog_transport_failed"
    );
}

/// Q2: the bound at the storage call site is exact for the entry kind,
/// with a declared length equal to the body.
#[test]
fn storage_bound_is_exact_per_kind() {
    let mut fake = Fake::default();
    fake.catalog.push_back(Ok(catalog_response()));
    let mut storage = response(200, "application/pdf", PDF.to_vec());
    storage
        .headers
        .push(("content-length".into(), PDF.len().to_string()));
    fake.storage.push_back(Ok(storage));
    let seen = fake.storage_max_seen.clone();
    let mut textbook_source = make_source(fake);
    textbook_source
        .fetch_pdf(
            PRINCIPAL,
            SchoolLevel::Middle,
            &document_ref("middle", URL),
            NOW,
        )
        .unwrap();
    assert_eq!(
        seen.borrow().as_slice(),
        [pdf_body_limit(DocumentKind::Sample)]
    );
}

/// Q3: rows follow `table.rows` order (thead, tbody sections, tfoot), and a
/// rowspan started in one body section carries into the next, not into the
/// footer row that sits between them in document order.
#[test]
fn catalog_rows_follow_table_rows_order() {
    let sample = "https://storage.studyq.net/data/answer/mi/book/g7_fictionalplus_sample_1_1.pdf";
    let answer = "https://storage.studyq.net/data/answer/mi/workbook/g7_fictionalplus_answer.pdf";
    let html = format!(
        "<div class=\"tab tab-on\">중등부</div><table><tbody><tr><td rowspan=\"2\">중1</td><td>Invented first body</td><td></td></tr></tbody><tfoot><tr><td>Foot label</td><td><button onclick=\"fullUrlDownloadFile('{answer}')\">sample</button></td></tr></tfoot><tbody><tr><td>Invented Plus</td><td><button onclick=\"fullUrlDownloadFile('{sample}')\">sample</button></td></tr></tbody></table>"
    );
    let value = parse_catalog(&html, SchoolLevel::Middle, NOW).unwrap();
    let sample_entry = value
        .entries
        .iter()
        .find(|e| e.kind == DocumentKind::Sample)
        .unwrap();
    assert_eq!(sample_entry.source_labels, ["중1 Invented Plus sample"]);
    let answer_entry = value
        .entries
        .iter()
        .find(|e| e.kind == DocumentKind::Answer)
        .unwrap();
    assert_eq!(answer_entry.source_labels, ["Foot label sample"]);
}

/// Q5: an empty body with status 200 gives the size code for both kinds.
#[test]
fn empty_body_gives_size_code() {
    assert_eq!(
        code(
            bounded_body(
                response(200, "text/html", vec![]),
                catalog_body_limit(),
                BodyKind::Catalog
            )
            .unwrap_err()
        ),
        "textbook_catalog_size_rejected"
    );
    assert_eq!(
        code(
            bounded_body(
                response(200, "application/pdf", vec![]),
                pdf_body_limit(DocumentKind::Sample),
                BodyKind::Pdf
            )
            .unwrap_err()
        ),
        "textbook_pdf_size_rejected"
    );
}
