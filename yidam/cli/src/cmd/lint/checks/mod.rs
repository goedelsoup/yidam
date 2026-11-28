//! The invariants `yidam lint` enforces.
//!
//! Each function returns a [`Check`] whether or not it found anything — a check that
//! reports nothing when it passes cannot be distinguished from a check that did not run,
//! and the difference matters when someone is deciding whether the gate covers a case.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::parse::{CATALOG_LOCATION_KINDS, CATALOG_LOCATION_KINDS_RETIRED, CATALOG_TYPES};

use super::model::{Check, CheckSpec, Severity, Violation};
use super::quotations::Declared;
use crate::corpus::{
    authored_relationships, edge_views, normalize, sink_classes, source_classes, Class,
    DecisionRecord, EdgePolicy, Edges, Node, Omission, Source,
};

mod catalog;
mod claims;
mod classes;
mod edges;
mod instances;
mod intervals;
mod parse;
mod properties;
mod prose_links;
mod resolutions;
mod tables;

pub use catalog::*;
pub use claims::*;
pub use classes::*;
pub use edges::*;
pub use instances::*;
pub use intervals::*;
pub use parse::*;
pub use properties::*;
pub use prose_links::*;
pub use resolutions::*;
pub use tables::*;
