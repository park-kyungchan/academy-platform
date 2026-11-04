//! Version 3 lossless, flat adapter input. This is not a domain schema.
use crate::js::{Js, Text};
use serde::Deserialize;
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wire {
    pub root: usize,
    pub nodes: Vec<Node>,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Node {
    Null,
    Undefined,
    Hole,
    Bool {
        value: bool,
    },
    Number {
        bits: String,
    },
    String {
        units: Text,
    },
    Array {
        items: Vec<usize>,
    },
    Object {
        entries: Vec<(Text, usize)>,
    },
    Inherited {
        entries: Vec<(Text, usize)>,
    },
    NullPrototype {
        entries: Vec<(Text, usize)>,
    },
    SymbolKeys {
        entries: Vec<(Text, usize)>,
    },
    Accessor {
        entries: Vec<(Text, usize)>,
    },
    Decorated {
        items: Vec<usize>,
        entries: Vec<(Text, usize)>,
    },
    Sparse {
        length: usize,
    },
    Date {
        iso: Text,
    },
    Bigint,
    Function,
    Symbol,
    Cycle,
}
impl Wire {
    pub fn decode(&self) -> std::result::Result<Js, &'static str> {
        if self.nodes.len() > 65536 {
            return Err("s08_protocol_input_rejected");
        }
        let rejected = "s08_protocol_input_rejected";
        let mut work = vec![(self.root, false)];
        let mut values = Vec::new();
        let mut active = std::collections::HashSet::new();
        let mut budget = 65536usize;
        while let Some((id, finish)) = work.pop() {
            let n = self.nodes.get(id).ok_or(rejected)?;
            let children: Vec<usize> = match n {
                Node::Array { items } => items.clone(),
                Node::Object { entries }
                | Node::Inherited { entries }
                | Node::NullPrototype { entries }
                | Node::SymbolKeys { entries }
                | Node::Accessor { entries } => entries.iter().map(|(_, id)| *id).collect(),
                Node::Decorated { items, entries } => items
                    .iter()
                    .copied()
                    .chain(entries.iter().map(|(_, id)| *id))
                    .collect(),
                _ => vec![],
            };
            if !finish {
                if budget == 0 || !active.insert(id) {
                    return Err(rejected);
                }
                budget -= 1;
                work.push((id, true));
                for child in children.iter().rev() {
                    work.push((*child, false));
                }
                continue;
            }
            active.remove(&id);
            let start = values.len().checked_sub(children.len()).ok_or(rejected)?;
            let mut c = values.split_off(start).into_iter();
            let props = |entries: &[(Text, usize)],
                         c: &mut std::vec::IntoIter<Js>|
             -> std::result::Result<Vec<(Text, Js)>, &'static str> {
                let mut out = vec![];
                for (key, _) in entries {
                    if out.iter().any(|(k, _)| k == key) {
                        return Err(rejected);
                    }
                    out.push((key.clone(), c.next().ok_or(rejected)?));
                }
                Ok(out)
            };
            let value = match n {
                Node::Null => Js::Null,
                Node::Undefined => Js::Undefined,
                Node::Hole => Js::Hole,
                Node::Bool { value } => Js::Bool(*value),
                Node::Number { bits } => {
                    if bits.len() != 16 || !bits.bytes().all(|b| b.is_ascii_hexdigit()) {
                        return Err(rejected);
                    }
                    Js::Num(f64::from_bits(
                        u64::from_str_radix(bits, 16).map_err(|_| rejected)?,
                    ))
                }
                Node::String { units } => Js::Str(units.clone()),
                Node::Array { .. } => Js::Arr(c.collect()),
                Node::Object { entries } => Js::Obj(props(entries, &mut c)?),
                Node::Inherited { entries } => Js::Inherited(props(entries, &mut c)?),
                Node::NullPrototype { entries } => Js::NullPrototype(props(entries, &mut c)?),
                Node::SymbolKeys { entries } => Js::SymbolKeys(props(entries, &mut c)?),
                Node::Accessor { entries } => Js::Accessor(props(entries, &mut c)?),
                Node::Decorated { items, entries } => {
                    let a = c.by_ref().take(items.len()).collect();
                    Js::Decorated(a, props(entries, &mut c)?)
                }
                Node::Sparse { length } => {
                    if *length > 65536 {
                        return Err(rejected);
                    }
                    Js::Sparse(*length)
                }
                Node::Date { iso } => Js::Date(iso.clone()),
                Node::Bigint => Js::BigInt,
                Node::Function => Js::Function,
                Node::Symbol => Js::Symbol,
                Node::Cycle => Js::Cycle,
            };
            values.push(value);
        }
        if values.len() != 1 {
            return Err(rejected);
        }
        values.pop().ok_or(rejected)
    }
}
/// A test provider must be supplied before intake/review/hash. No production default.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestPorts {
    pub uuid: String,
    pub now: String,
}
