//! Borrowed core workflow. The caller holds account, then draft, before entry.
use crate::{record_effect::SyntheticCoreSource, record_store::AccountGuard};
use edu_academy_core::{
    js::{Js, Result},
    workflow::{
        Runtime,
        review::Policy,
        service::{self, Backend, Service, Source},
        store::Store,
    },
};

/// Delegates approval, journal publication and settlement to the existing core.
/// It acquires no draft lock and retains no ports beyond the supplied borrows.
pub struct CoreAdapter<'a> {
    service: Service<'a>,
}
impl<'a> CoreAdapter<'a> {
    pub fn new(
        store: &'a Store,
        source: &'a mut dyn Source,
        backend: &'a mut dyn Backend,
        policy: &'a dyn Policy,
        runtime: &'a dyn Runtime,
    ) -> Self {
        Self {
            service: Service {
                store,
                source,
                backend,
                policy,
                runtime,
            },
        }
    }
    pub fn intake(&mut self, input: &Js) -> Result<Js> {
        self.service.intake(input)
    }
    pub fn review(&mut self, input: &Js, persist: bool) -> Result<Js> {
        self.service.review(input, persist)
    }
    pub fn save(&mut self, input: &Js) -> Result<Js> {
        self.service.save(input)
    }
    pub fn reconcile(&mut self, input: &Js) -> Result<Js> {
        self.service.reconcile(input)
    }
    pub fn status(&self, input: &Js) -> Result<Js> {
        service::status(self.service.store, input)
    }
}

/// Each core read samples the caller's clock instead of reusing capture time.
pub struct ClockedHeldSource<'a> {
    pub inner: &'a mut SyntheticCoreSource,
    pub account: &'a AccountGuard<'a>,
    pub runtime: &'a dyn Runtime,
}
impl Source for ClockedHeldSource<'_> {
    fn read(&mut self, scope: &Js, _: &Js, subject: &Js) -> Result<Js> {
        self.inner
            .read_with_account(self.account, scope, subject, &self.runtime.now())
    }
}
