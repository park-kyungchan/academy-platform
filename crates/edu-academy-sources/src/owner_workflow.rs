//! One owner-local actual Client, one retained authentication result, two fallible views.
//! No default transport, credentials, environment, owner root or raw HttpPort writes.
use crate::{
    data::*,
    http::{Client, Transport},
    native::{self, StageResult},
};
use edu_academy_core::{
    js::{Fault, Result as CoreResult},
    workflow::{Runtime, approval::Approval, native::SourcePort as WritePort, service::Source},
};
use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

/// Deliberately neither Debug nor Clone. Values stay inside the injected provider/client call.
pub struct Credentials {
    username: String,
    password: String,
}
impl Credentials {
    pub fn new(username: String, password: String) -> Self {
        Self { username, password }
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
