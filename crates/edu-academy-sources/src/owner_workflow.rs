//! Owner-local Client with one retained authentication result and fallible views.
//! Native sessions require pinned owner inputs, account exclusion and readback fences.
//! No environment discovery, default credentials or raw HttpPort writes.
use crate::{
    data::*,
    http::{Client, Transport},
    native::{self, StageResult},
};
use edu_academy_core::{
    js::{Fault, Result as CoreResult},
    workflow::{
        Runtime,
        approval::{Approval, AttributionOwner, AttributionStore},
        native::SourcePort as WritePort,
        service::Source,
    },
};
use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

/// Deliberately neither Debug nor Clone. Values stay inside the injected provider/client call.
pub struct Credentials {
    username: String,
    password: String,
    expires_at_ms: Option<u64>,
}
impl Credentials {
    pub fn new(username: String, password: String) -> Self {
        Self {
            username,
            password,
            expires_at_ms: None,
        }
    }
}
pub trait CredentialProvider {
    fn credential(&mut self) -> Result<Credentials>;
}
impl<F: FnMut() -> Result<Credentials>> CredentialProvider for F {
    fn credential(&mut self) -> Result<Credentials> {
        self()
    }
}
struct OwnerClient<'a, T: Transport, C: CredentialProvider> {
    client: Client<T>,
    credential: C,
    runtime: &'a dyn Runtime,
}
impl<T: Transport, C: CredentialProvider> native::SourcePort for OwnerClient<'_, T, C> {
    fn authenticate(&mut self) -> Result<Js> {
        let credential = self.credential.credential()?;
        self.client
            .authenticate(&credential.username, &credential.password)
    }
    fn discover(&mut self, input: &Js) -> Result<Js> {
        self.client.discover(input, &self.runtime.now())
    }
    fn execute(&mut self, input: &Js) -> Result<Js> {
        self.client.execute(input, &self.runtime.now())
    }
    fn journal_state(&mut self, target: &Js) -> Result<Js> {
        self.client.journal_state(target, &self.runtime.now())
    }
    fn has_name_search(&self) -> bool {
        true
    }
    fn search_students_by_name(&mut self, input: &Js) -> Option<Result<Js>> {
        // F owns this actual method. Root must integrate F before compiling E.
        Some(self.client.search_students_by_name(input))
    }
}
type State<'a, T, C> = native::Backend<OwnerClient<'a, T, C>>;
type Shared<'a, T, C> = Rc<RefCell<State<'a, T, C>>>;

pub struct Owner<'a, T: Transport, C: CredentialProvider> {
    state: Shared<'a, T, C>,
}
pub struct SourceView<'a, T: Transport, C: CredentialProvider> {
    state: Shared<'a, T, C>,
}
pub struct WriteView<'a, T: Transport, C: CredentialProvider> {
    state: Shared<'a, T, C>,
}

fn borrow<'s, 'a, T: Transport, C: CredentialProvider>(
    state: &'s Shared<'a, T, C>,
) -> CoreResult<RefMut<'s, State<'a, T, C>>> {
    state
        .try_borrow_mut()
        .map_err(|e| Fault::error(e.to_string()))
}
impl<'a, T: Transport, C: CredentialProvider> Owner<'a, T, C> {
    /// Validate owner schema, teacher and launch date before credentials or transport.
    /// None preserves the selected launch date, not the standalone historical default.
    pub fn new(
        client: Client<T>,
        admission: Js,
        owner: Option<&Js>,
        managed_names: Vec<Js>,
        credential: C,
        runtime: &'a dyn Runtime,
    ) -> Result<Self> {
        let state = native::Backend::with_owner(
            OwnerClient {
                client,
                credential,
                runtime,
            },
            admission,
            owner,
            managed_names,
        )?;
        Ok(Self {
            state: Rc::new(RefCell::new(state)),
        })
    }
    pub fn source(&self) -> SourceView<'a, T, C> {
        SourceView {
            state: Rc::clone(&self.state),
        }
    }
    pub fn writer(&self) -> WriteView<'a, T, C> {
        WriteView {
            state: Rc::clone(&self.state),
        }
    }
    /// These are copies of already-validated data, suitable for NativeBackend::new.
    pub fn write_configuration(&self) -> CoreResult<(Js, Js, Vec<Js>)> {
        let state = borrow(&self.state)?;
        Ok((
            state.admission.clone(),
            state.owner_admission(),
            state.managed_names().to_vec(),
        ))
    }
}
impl<T: Transport, C: CredentialProvider> SourceView<'_, T, C> {
    pub fn read_routing_admission(&self) -> CoreResult<Js> {
        Ok(borrow(&self.state)?.read_routing_admission())
    }
    /// Typed not-admitted is intentionally distinct from arbitrary parsed error parts.
    pub fn search_students_by_name(
        &mut self,
        exact_name: &Js,
        request_ref: Option<&Js>,
        lesson_date: Option<&Js>,
    ) -> StageResult<Js> {
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|e| native::StageError::Source(Error::error(e.to_string())))?;
        let now_ms = state.port.runtime.epoch_ms();
        state.search_students_by_name(exact_name, request_ref, lesson_date, now_ms)
    }
}
impl<T: Transport, C: CredentialProvider> Source for SourceView<'_, T, C> {
    fn read(&mut self, scope: &Js, request: &Js, subject: &Js) -> CoreResult<Js> {
        let mut state = borrow(&self.state)?;
        let runtime = state.port.runtime;
        let now = runtime.now();
        let uuid = runtime.uuid()?;
        state
            .read_owner(
                scope,
                Some(request),
                Some(subject),
                runtime.epoch_ms(),
                &now,
                &uuid,
            )
            .map_err(native::StageError::into_fault)
    }
}
impl<T: Transport, C: CredentialProvider> WritePort for WriteView<'_, T, C> {
    fn authenticate(&mut self) -> CoreResult<()> {
        borrow(&self.state)?
            .ensure_authenticated()
            .map(|_| ())
            .map_err(native::StageError::into_fault)
    }
    fn discover_groups(&mut self, input: &Js) -> CoreResult<Js> {
        let mut state = borrow(&self.state)?;
        state
            .ensure_authenticated()
            .map_err(native::StageError::into_fault)?;
        native::SourcePort::discover(&mut state.port, input).map_err(fault_from_error)
    }
    fn execute(&mut self, input: &Js) -> CoreResult<Js> {
        let mut state = borrow(&self.state)?;
        state
            .ensure_authenticated()
            .map_err(native::StageError::into_fault)?;
        native::SourcePort::execute(&mut state.port, input).map_err(fault_from_error)
    }
    fn journal_state(&mut self, target: &Js) -> CoreResult<Js> {
        let mut state = borrow(&self.state)?;
        state
            .ensure_authenticated()
            .map_err(native::StageError::into_fault)?;
        native::SourcePort::journal_state(&mut state.port, target).map_err(fault_from_error)
    }
    fn apply(&mut self, action: &Js, approval: &mut dyn Approval) -> CoreResult<Js> {
        let mut state = borrow(&self.state)?;
        state
            .ensure_authenticated()
            .map_err(native::StageError::into_fault)?;
        let now = state.port.runtime.now();
        state
            .port
            .client
            .apply_journal_action(action, approval, &now)
            .map_err(fault_from_error)
    }
}

/// Only locations and content pins cross the native launch boundary. The private
/// executable accepts these as explicit owner inputs; no inherited environment,
/// Hermes profile lookup, dynamic module import or executable path is required.
#[derive(Clone)]
pub struct PinnedInput {
    pub relative_path: String,
    pub sha256: String,
}
pub struct NativeOwnerInputs {
    pub root: std::path::PathBuf,
    pub admission: PinnedInput,
    pub credential: PinnedInput,
}
/// Credential values deliberately have no Debug/Clone/serialization implementation.
/// The provider reads the exact bytes once through OwnerRoot's descriptor checks.
pub struct FileCredentialProvider<'a> {
    root: Rc<crate::owner_root::OwnerRoot>,
    input: PinnedInput,
    runtime: &'a dyn Runtime,
}
impl<'a> FileCredentialProvider<'a> {
    pub fn new(
        root: Rc<crate::owner_root::OwnerRoot>,
        input: PinnedInput,
        runtime: &'a dyn Runtime,
    ) -> Self {
        Self {
            root,
            input,
            runtime,
        }
    }
}
impl CredentialProvider for FileCredentialProvider<'_> {
    fn credential(&mut self) -> Result<Credentials> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Document {
            schema_version: String,
            username: String,
            password: String,
            expires_at_ms: u64,
        }
        let capture = self
            .root
            .read(&self.input.relative_path, &self.input.sha256, 16384)
            .map_err(|_| Error::error("native_owner_credential_refused"))?;
        let document: Document = serde_json::from_slice(&capture.bytes)
            .map_err(|_| Error::error("native_owner_credential_refused"))?;
        if document.schema_version != "academy.owner-credential/1"
            || document.username.is_empty()
            || document.username.len() > 1024
            || document.password.is_empty()
            || document.password.len() > 8192
            || document.expires_at_ms <= self.runtime.epoch_ms()
            || document.expires_at_ms > 9_007_199_254_740_991
        {
            return Err(Error::error("native_owner_credential_refused"));
        }
        Ok(Credentials {
            username: document.username,
            password: document.password,
            expires_at_ms: Some(document.expires_at_ms),
        })
    }
}
/// Generic source session for every retained family. Family DTO parsing and
/// workflow approvals remain with their owners. This is not an MCP business
/// implementation, production qualification, or permission to call Academy.
/// A single Client and account lease span login, reads, effects and readbacks.
pub struct NativeSession<'a, T: Transport> {
    client: Client<CredentialTransport<'a, T>>,
    admission: crate::owner_admission::TransportAdmission,
    lease: crate::owner_root::AccountLease,
    credential: Option<Credentials>,
    authentication: Option<Result<Js>>,
    expires_at_ms: u64,
    cancellation: crate::http::Cancellation,
    runtime: &'a dyn Runtime,
}
/// The guard is beneath Client so every actual request, including recursive
/// login redirects and discovery, rechecks the owner clock before network I/O.
struct CredentialTransport<'a, T: Transport> {
    inner: T,
    runtime: &'a dyn Runtime,
    expires_at_ms: u64,
    expired: bool,
    cancellation: crate::http::Cancellation,
}
impl<T: Transport> Transport for CredentialTransport<'_, T> {
    fn dispatch(&mut self, request: &crate::http::Request) -> Result<crate::http::Response> {
        self.cancellation.check()?;
        if self.expired || self.runtime.epoch_ms() >= self.expires_at_ms {
            self.expired = true;
            return Err(Error::error("native_owner_credential_expired"));
        }
        self.inner.dispatch(request)
    }
}
impl<'a> NativeSession<'a, crate::http::NativeTransport> {
    pub fn installed(
        inputs: NativeOwnerInputs,
        limits: crate::http::TransportLimits,
        cancellation: crate::http::Cancellation,
        runtime: &'a dyn Runtime,
    ) -> Result<Self> {
        let root = Rc::new(crate::owner_root::OwnerRoot::open(&inputs.root)?);
        let capture = root.read(
            &inputs.admission.relative_path,
            &inputs.admission.sha256,
            256 * 1024,
        )?;
        let admission = crate::owner_admission::TransportAdmission::from_bytes(&capture.bytes)?;
        let credential = FileCredentialProvider::new(Rc::clone(&root), inputs.credential, runtime);
        let transport = crate::http::NativeTransport::new(limits, cancellation.clone())?;
        Self::open(
            root,
            admission,
            transport,
            credential,
            cancellation,
            runtime,
        )
    }
}
impl<'a, T: Transport> NativeSession<'a, T> {
    pub fn open(
        root: Rc<crate::owner_root::OwnerRoot>,
        admission: crate::owner_admission::TransportAdmission,
        transport: T,
        mut provider: impl CredentialProvider,
        cancellation: crate::http::Cancellation,
        runtime: &'a dyn Runtime,
    ) -> Result<Self> {
        admission.validate()?;
        cancellation.check()?;
        let credential = provider
            .credential()
            .map_err(|_| Error::error("native_owner_credential_refused"))?;
        if credential.username.is_empty() || credential.password.is_empty() {
            return Err(Error::error("native_owner_credential_refused"));
        }
        // Production requires an expiring owner-published credential document.
        // Synthetic injected providers may supply an explicit Credentials value.
        let expires_at_ms = credential.expires_at_ms.unwrap_or(u64::MAX);
        if expires_at_ms <= runtime.epoch_ms() {
            return Err(Error::error("native_owner_credential_refused"));
        }
        let account = hash(
            format!(
                "{origin}\0{username}",
                origin = crate::http::ORIGIN,
                username = credential.username
            )
            .as_bytes(),
        );
        let lease = root.account(&account)?;
        let principal = obj(&[
            ("teacherLabel", admission.teacher_label.as_str().into()),
            ("teacherKey", admission.teacher_id.as_str().into()),
            ("collegeKey", admission.college_id.as_str().into()),
        ]);
        Ok(Self {
            client: Client::new(
                principal,
                CredentialTransport {
                    inner: transport,
                    runtime,
                    expires_at_ms,
                    expired: false,
                    cancellation: cancellation.clone(),
                },
            )?,
            admission,
            lease,
            credential: Some(credential),
            authentication: None,
            expires_at_ms,
            cancellation,
            runtime,
        })
    }
    fn check_credential_lifetime(&mut self) -> Result<()> {
        if self.runtime.epoch_ms() >= self.expires_at_ms {
            self.client.close();
            self.credential.take();
            return Err(Error::error("native_owner_credential_expired"));
        }
        Ok(())
    }
    fn authenticate(&mut self) -> Result<()> {
        self.cancellation.check()?;
        self.check_credential_lifetime()?;
        if self.authentication.is_none() {
            let credential = self
                .credential
                .take()
                .ok_or_else(|| Error::error("native_owner_credential_refused"))?;
            self.authentication = Some(
                self.client
                    .authenticate(&credential.username, &credential.password)
                    .map_err(|error| {
                        if matches!(
                            error.message.as_str(),
                            "native_transport_timeout"
                                | "native_owner_credential_expired"
                                | "native_transport_cancelled"
                        ) {
                            error
                        } else {
                            Error::error("native_owner_authentication_failed")
                        }
                    }),
            );
        }
        self.authentication
            .as_ref()
            .expect("assigned authentication")
            .as_ref()
            .map_err(Clone::clone)?;
        // Login includes several blocking exchanges. Expiry must be rechecked
        // after them, before any new source request can be dispatched.
        self.check_credential_lifetime()?;
        if !self.client.authenticated {
            return Err(Error::error("native_owner_session_expired"));
        }
        Ok(())
    }
    fn dispatch(&mut self, request: crate::http::Request) -> Result<crate::http::Response> {
        self.authenticate()?;
        self.cancellation.check()?;
        let response = self.client.scoped_request(request)?;
        self.cancellation.check()?;
        Ok(response)
    }
    pub fn read(
        &mut self,
        scope: &crate::owner_admission::TransportScope,
        operation: &str,
        arguments: &[(String, String)],
    ) -> Result<crate::http::Response> {
        let request = self.admission.request(scope, operation, arguments, false)?;
        self.dispatch(request)
    }
    pub fn reconciliation_required(&self) -> Result<bool> {
        self.lease.reconciliation_required()
    }
    fn prepare_effect(&self, intent: &EffectIntent<'_>) -> Result<PreparedEffect> {
        let request =
            self.admission
                .request(intent.scope, intent.operation, intent.arguments, true)?;
        let readback = self.admission.request(
            intent.scope,
            intent.readback_operation,
            intent.readback_arguments,
            false,
        )?;
        if !sha_valid(&intent.expected_sha256.into()) {
            return Err(Error::error("native_readback_digest_required"));
        }
        let pending = crate::owner_root::PendingEffect {
            scope: scope_digest(intent.scope)?,
            operation: request_digest(&request),
            readback: request_digest(&readback),
            expected: intent.expected_sha256.into(),
        };
        let authorization = obj(&[
            (
                "sourceNamespace",
                intent.scope.source_namespace.as_str().into(),
            ),
            ("ownerId", intent.scope.owner_id.as_str().into()),
            ("teacherId", intent.scope.teacher_id.as_str().into()),
            ("studentId", intent.scope.student_id.as_str().into()),
            ("date", intent.scope.date.as_str().into()),
            ("operationDigest", pending.operation.as_str().into()),
            ("readbackDigest", pending.readback.as_str().into()),
            ("expectedSha256", intent.expected_sha256.into()),
        ]);
        Ok(PreparedEffect {
            request,
            pending,
            authorization,
        })
    }
    /// Exact transport action for the private owner's independent E06 review/grant.
    /// This creates no approval, consumes nothing and performs no network I/O.
    pub fn effect_action(&self, intent: &EffectIntent<'_>) -> Result<Js> {
        Ok(self.prepare_effect(intent)?.authorization)
    }
    /// Effects REQUIRE E06 verification and the actual owner's atomic
    /// compare-and-consume. An older Approval callback cannot authorize this API.
    /// expected_sha256 is an exact readback body, never an acknowledgement;
    /// dynamic documents still require a reviewed family projection/contract.
    pub fn effect<S: AttributionStore>(
        &mut self,
        intent: EffectIntent<'_>,
        attribution: &mut EffectAttribution<'_, S>,
    ) -> Result<crate::http::Response> {
        let PreparedEffect {
            request,
            pending,
            authorization,
        } = self.prepare_effect(&intent)?;
        if self.lease.reconciliation_required()? {
            return Err(Error::error("owner_account_reconciliation_required"));
        }
        self.authenticate()?;
        attribution.consume(&authorization, self.runtime.epoch_ms())?;
        self.cancellation.check()?;
        self.lease.begin(&pending)?;
        // Exactly one effect dispatch after durable fencing. Errors stay uncertain.
        self.dispatch(request)
            .map_err(|_| Error::error("native_effect_uncertain"))?;
        self.reconcile(
            intent.scope,
            intent.readback_operation,
            intent.readback_arguments,
        )
        .map_err(|_| Error::error("native_effect_uncertain"))
    }
    /// READ/reconcile deliberately use TransportAdmission + scope, not instruction
    /// attribution. R12 supplies private owner identity; no effect is granted here.
    pub fn reconcile(
        &mut self,
        scope: &crate::owner_admission::TransportScope,
        operation: &str,
        arguments: &[(String, String)],
    ) -> Result<crate::http::Response> {
        let request = self.admission.request(scope, operation, arguments, false)?;
        let pending = self
            .lease
            .pending()?
            .ok_or_else(|| Error::error("native_reconciliation_not_pending"))?;
        if pending.scope != scope_digest(scope)? || pending.readback != request_digest(&request) {
            return Err(Error::error("native_reconciliation_scope_refused"));
        }
        let response = self.dispatch(request)?;
        if hash(&response.bytes) != pending.expected {
            return Err(Error::error("native_reconciliation_readback_mismatch"));
        }
        self.lease.reconciled()?;
        Ok(response)
    }
}
impl<T: Transport> Drop for NativeSession<'_, T> {
    fn drop(&mut self) {
        self.client.close();
        self.credential.take();
    }
}
struct PreparedEffect {
    request: crate::http::Request,
    pending: crate::owner_root::PendingEffect,
    authorization: Js,
}
/// Mandatory E06 effect authorization. The installed owner supplies its existing
/// transaction store; this wrapper cannot accept a caller-generated Verified flag
/// or a legacy Approval callback. E06 rechecks fresh instruction/session/source/
/// grant/time under that store's transaction before consuming once.
pub struct EffectAttribution<'a, S: AttributionStore> {
    approval: edu_academy_core::attribution::Approval,
    source_sha256: String,
    store: &'a mut S,
}
impl<'a, S: AttributionStore> EffectAttribution<'a, S> {
    pub fn new(envelope: &Js, source_sha256: &str, store: &'a mut S) -> Result<Self> {
        if !sha_valid(&source_sha256.into()) {
            return Err(Error::error("native_effect_attribution_refused"));
        }
        let approval = edu_academy_core::attribution::Approval::new(envelope)
            .map_err(|_| Error::error("native_effect_attribution_refused"))?;
        Ok(Self {
            approval,
            source_sha256: source_sha256.into(),
            store,
        })
    }
    fn consume(&mut self, action: &Js, now_ms: u64) -> Result<()> {
        let now_ms =
            i64::try_from(now_ms).map_err(|_| Error::error("native_effect_attribution_refused"))?;
        let verdict = self
            .approval
            .consume(
                action,
                &self.source_sha256,
                now_ms,
                &mut AttributionOwner::new(&mut *self.store),
            )
            .map_err(|_| Error::error("native_effect_attribution_refused"))?;
        match verdict {
            edu_academy_core::attribution::Verdict::Verified { .. } => Ok(()),
            edu_academy_core::attribution::Verdict::Rejected(_) => {
                Err(Error::error("native_effect_attribution_refused"))
            }
        }
    }
}
pub struct EffectIntent<'a> {
    pub scope: &'a crate::owner_admission::TransportScope,
    pub operation: &'a str,
    pub arguments: &'a [(String, String)],
    pub readback_operation: &'a str,
    pub readback_arguments: &'a [(String, String)],
    pub expected_sha256: &'a str,
}
fn scope_digest(scope: &crate::owner_admission::TransportScope) -> Result<String> {
    serde_json::to_vec(scope)
        .map(|bytes| hash(&bytes))
        .map_err(|_| Error::error("native_transport_scope_refused"))
}
fn request_digest(request: &crate::http::Request) -> String {
    // Length-delimited JSON avoids concatenation collisions, and excludes cookies.
    let bytes = serde_json::to_vec(&(
        &request.method,
        &request.path,
        &request.query,
        &request.body,
    ))
    .expect("string request serialization");
    hash(&bytes)
}
