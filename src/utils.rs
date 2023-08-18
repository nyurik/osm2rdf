use std::fmt::{Debug, Display};

use chrono::{DateTime, TimeZone, Utc};
use osmpbf::{DenseNodeInfo, Info};
use percent_encoding::{AsciiSet, CONTROLS};

use crate::str_builder::StringBuf;

pub fn to_utc(milli_timestamp: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(milli_timestamp / 1000, (milli_timestamp % 1000) as u32)
        .unwrap()
}

pub const PERCENT_ENC_SET: &AsciiSet = &CONTROLS
    .add(b';')
    .add(b'@')
    .add(b'$')
    .add(b'!')
    .add(b'*')
    .add(b'(')
    .add(b')')
    .add(b',')
    .add(b'/')
    .add(b'~')
    .add(b':')
    // The "#" is also safe - used for anchoring
    .add(b'#');

#[derive(Clone, Default, Debug)]
pub struct Stats {
    pub added_nodes: u64,
    pub added_rels: u64,
    pub added_ways: u64,
    pub skipped_nodes: u64,
    pub deleted_nodes: u64,
    pub deleted_rels: u64,
    pub deleted_ways: u64,
    pub blocks: u64,
}

impl Stats {
    pub(crate) fn combine(&mut self, other: Stats) {
        self.added_nodes += other.added_nodes;
        self.added_rels += other.added_rels;
        self.added_ways += other.added_ways;
        self.skipped_nodes += other.skipped_nodes;
        self.deleted_nodes += other.deleted_nodes;
        self.deleted_rels += other.deleted_rels;
        self.deleted_ways += other.deleted_ways;
        self.blocks += 1;
    }
}

#[derive(Debug)]
pub enum Statement {
    Skip,
    Delete {
        elem: Element,
        id: i64,
    },
    Create {
        elem: Element,
        id: i64,
        ts: i64,
        val: StringBuf,
    },
}

#[derive(Debug)]
pub enum Element {
    Node,
    Way,
    Relation,
}

impl Display for Element {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Element::Node => write!(f, "osmnode"),
            Element::Way => write!(f, "osmway"),
            Element::Relation => write!(f, "osmrel"),
        }
    }
}

pub struct ElementInfo<'a> {
    pub is_deleted: bool,
    pub version: i32,
    pub user: Option<&'a str>,
    pub milli_timestamp: i64,
    pub changeset: i64,
}

impl<'a> From<Info<'a>> for ElementInfo<'a> {
    fn from(info: Info<'a>) -> Self {
        Self {
            is_deleted: info.deleted(),
            version: info.version().unwrap(),
            user: info.user().map(|v| v.unwrap()),
            milli_timestamp: info.milli_timestamp().unwrap(),
            changeset: info.changeset().unwrap(),
        }
    }
}

impl<'a> From<&DenseNodeInfo<'a>> for ElementInfo<'a> {
    fn from(info: &DenseNodeInfo<'a>) -> Self {
        Self {
            is_deleted: info.deleted(),
            version: info.version(),
            user: info.user().ok(),
            milli_timestamp: info.milli_timestamp(),
            changeset: info.changeset(),
        }
    }
}
