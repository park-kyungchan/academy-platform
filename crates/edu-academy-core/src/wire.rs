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
        fn read(w: &Wire, id: usize, d: usize, budget: &mut usize) -> Option<Js> {
            if d > 96 || *budget == 0 {
                return None;
            }
            *budget -= 1;
            let child = |i, b: &mut usize| read(w, i, d + 1, b);
            let list = |a: &[usize], b: &mut usize| {
                a.iter().map(|i| child(*i, b)).collect::<Option<Vec<_>>>()
            };
            let props = |a: &[(Text, usize)], b: &mut usize| {
                let mut out = Vec::new();
                for (k, i) in a {
                    if out.iter().any(|(old, _)| old == k) {
                        return None;
                    }
                    out.push((k.clone(), child(*i, b)?));
                }
                Some(out)
            };
            Some(match w.nodes.get(id)? {
                Node::Null => Js::Null,
                Node::Undefined => Js::Undefined,
                Node::Hole => Js::Hole,
                Node::Bool { value } => Js::Bool(*value),
                Node::Number { bits } => {
                    if bits.len() != 16 || !bits.bytes().all(|b| b.is_ascii_hexdigit()) {
                        return None;
                    }
                    Js::Num(f64::from_bits(u64::from_str_radix(bits, 16).ok()?))
                }
                Node::String { units } => Js::Str(units.clone()),
                Node::Array { items } => Js::Arr(list(items, budget)?),
                Node::Object { entries } => Js::Obj(props(entries, budget)?),
                Node::NullPrototype { entries } => Js::NullPrototype(props(entries, budget)?),
                Node::SymbolKeys { entries } => Js::SymbolKeys(props(entries, budget)?),
                Node::Accessor { entries } => Js::Accessor(props(entries, budget)?),
                Node::Decorated { items, entries } => {
                    Js::Decorated(list(items, budget)?, props(entries, budget)?)
                }
                Node::Sparse { length } => {
                    if *length > 65536 {
                        return None;
                    }
                    Js::Sparse(*length)
                }
                Node::Date { iso } => Js::Date(iso.clone()),
                Node::Bigint => Js::BigInt,
                Node::Function => Js::Function,
                Node::Symbol => Js::Symbol,
                Node::Cycle => Js::Cycle,
            })
        }
        if self.nodes.len() > 65536 {
            return Err("s08_protocol_input_rejected");
        }
        read(self, self.root, 0, &mut 65536).ok_or("s08_protocol_input_rejected")
    }
}
/// A test provider must be supplied before intake/review/hash. No production default.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestPorts {
    pub uuid: String,
    pub now: String,
}
