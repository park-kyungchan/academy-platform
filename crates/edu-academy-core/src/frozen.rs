//! Native immutable data boundary. JS descriptor-observing callers retain their TS port.
use crate::{js::*, knowledge};
/// No mutable reference or shared mutable child is exposed by this type.
#[derive(Debug)]
pub struct FrozenValue {
    value: Js,
}
impl FrozenValue {
    pub fn copy(value: &Js) -> Result<Self> {
        Ok(Self {
            value: knowledge::immutable_copy(value)?,
        })
    }
    pub(crate) fn from_validated(value: Js) -> Self {
        Self { value }
    }
    pub fn value(&self) -> &Js {
        &self.value
    }
    pub fn detached_copy(&self) -> Js {
        self.value.clone()
    }
    pub fn all_frozen(&self) -> bool {
        true
    }
    pub fn try_define(&self, _key: &str, _value: Js) -> Result<()> {
        Err(Fault::type_error(
            "Attempting to define property on object that is not extensible.",
        ))
    }
}
