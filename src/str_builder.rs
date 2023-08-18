use std::fmt::{Debug, Display, Write as _};
use std::ops::{Deref, DerefMut};

use json::JsonValue;
use lazy_static::lazy_static;
use osmpbf::{RelMember, RelMemberType};
use percent_encoding::utf8_percent_encode;
use regex::Regex;

use crate::utils;
use crate::utils::{Element, ElementInfo, PERCENT_ENC_SET};

lazy_static! {
    /// Total length of the maximum "valid" local name is 60 (58 + first + last char)
    /// Local name may contain letters, numbers anywhere, and -:_ symbols anywhere except first and last position
    pub static ref RE_SIMPLE_LOCAL_NAME: Regex = Regex::new(r"^[0-9a-zA-Z_]([-:0-9a-zA-Z_]{0,58}[0-9a-zA-Z_])?$").unwrap();
    pub static ref RE_WIKIDATA_KEY: Regex = Regex::new(r"(.:)?wikidata$").unwrap();
    pub static ref RE_WIKIDATA_VALUE: Regex = Regex::new(r"^Q[1-9][0-9]{0,18}$").unwrap();
    pub static ref RE_WIKIDATA_MULTI_VALUE: Regex = Regex::new(r"^Q[1-9][0-9]{0,18}(\s*;\s*Q[1-9][0-9]{0,18})+$").unwrap();
    pub static ref RE_WIKIPEDIA_VALUE: Regex = Regex::new(r"^([-a-z]+):(.+)$").unwrap();
}

#[repr(transparent)]
pub struct StringBuf {
    pub buf: String,
}

impl Deref for StringBuf {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.buf
    }
}

impl DerefMut for StringBuf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buf
    }
}

impl Debug for StringBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() && self.buf.contains('\n') {
            // Print as a raw string with real newlines and quotes. Pretend like it is using the indoc! macro.
            f.write_str(r#"indoc! {r#""#)?;
            f.write_char('\n')?;
            let mut is_newline = true;
            let mut iter = self.buf.escape_debug().peekable();
            while let Some(ch) = iter.next() {
                if Some(ch) == Some('\\') {
                    let Some(next) = iter.peek() else { continue };
                    if *next == '\'' || *next == '"' {
                        continue;
                    } else if *next == 'n' {
                        iter.next();
                        f.write_char('\n')?;
                        is_newline = true;
                        continue;
                    }
                }
                if is_newline {
                    f.write_str("    ")?;
                    is_newline = false;
                }
                f.write_char(ch)?;
            }
            f.write_str(r#""}#""#)
        } else {
            Debug::fmt(&self.buf, f)
        }
    }
}

impl Display for StringBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.buf, f)
    }
}

impl StringBuf {
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: String::with_capacity(capacity),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn add_value(&mut self, predicate: impl Display, value: impl XsdValue) {
        writeln!(self, r#"{predicate} {value};"#).unwrap();
    }

    pub fn add_tags<'t, TTags: Iterator<Item = (&'t str, &'t str)> + ExactSizeIterator>(
        &mut self,
        tags: TTags,
    ) {
        for (key, val) in tags {
            if key == "created_by" {
                continue;
            }
            if !RE_SIMPLE_LOCAL_NAME.is_match(key) {
                // Record any unusual tag name in a "osmm:badkey" statement
                self.add_value("osmm:badkey", XsdStr(key));
                continue;
            }

            let prop = XsdRaw("osmt", key);
            if key.contains("wikidata") {
                if RE_WIKIDATA_VALUE.is_match(val) {
                    self.add_value(prop, XsdRaw("wd", val));
                    continue;
                } else if RE_WIKIDATA_MULTI_VALUE.is_match(val) {
                    let vals = || val.split(';').map(|v| XsdRaw("wd", v.trim()));
                    self.add_value(prop, XsdIter(vals));
                    continue;
                }
            } else if key.contains("wikipedia") {
                if let Some(v) = RE_WIKIPEDIA_VALUE.captures(val) {
                    let lang = v.get(1).unwrap().as_str();
                    let title = v.get(2).unwrap().as_str();
                    let title = title.replace(' ', "_");
                    let title = &utf8_percent_encode(&title, PERCENT_ENC_SET);
                    self.add_value(prop, XsdWikipedia { lang, title });
                    continue;
                }
            }
            self.add_value(prop, XsdStr(val));
        }
    }

    pub fn finalize(mut self, info: ElementInfo) -> StringBuf {
        self.add_value("osmm:version", XsdInteger(info.version as i64));
        if let Some(user) = info.user {
            self.add_value("osmm:user", XsdStr(user));
        }
        self.add_value("osmm:timestamp", XsdDateTime(info.milli_timestamp));
        self.add_value("osmm:changeset", XsdInteger(info.changeset));
        self.pop(); // remove trailing "\n"
        self.pop(); // remove trailing ";"
        self.push_str(".\n");
        self
    }
}

pub trait XsdValue: Display {}

pub struct XsdPoint {
    pub lat: f64,
    pub lon: f64,
}
impl XsdValue for XsdPoint {}
impl Display for XsdPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#""Point({lon} {lat})"^^geo:wktLiteral"#,
            lon = self.lon,
            lat = self.lat,
        )
    }
}

pub struct XsdWikipedia<'a, T: Display> {
    pub lang: &'a str,
    pub title: &'a T,
}

impl<T: Display> XsdValue for XsdWikipedia<'_, T> {}
impl<T: Display> Display for XsdWikipedia<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<https://{lang}.wikipedia.org/wiki/{title}>",
            lang = self.lang,
            title = self.title,
        )
    }
}

pub struct XsdInteger(i64);
impl XsdValue for XsdInteger {}
impl Display for XsdInteger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, r#""{}"^^xsd:integer"#, self.0)
    }
}

pub struct XsdBoolean(pub bool);
impl XsdValue for XsdBoolean {}
impl Display for XsdBoolean {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = if self.0 { "true" } else { "false" };
        write!(f, r#""{value}"^^xsd:boolean"#)
    }
}

pub struct XsdDateTime(i64);
impl XsdValue for XsdDateTime {}
impl Display for XsdDateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // "{0:%Y-%m-%dT%H:%M:%S}Z"^^xsd:dateTime
        let ts = utils::to_utc(self.0);
        write!(f, r#""{ts}"^^xsd:dateTime"#)
    }
}

pub struct XsdStr<'a>(pub &'a str);
impl XsdValue for XsdStr<'_> {}
impl Display for XsdStr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&JsonValue::from(self.0).dump())
    }
}

pub struct XsdRaw<'a>(pub &'a str, pub &'a str);
impl XsdValue for XsdRaw<'_> {}
impl Display for XsdRaw<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.0, self.1)
    }
}

pub struct XsdElement(pub Element);
impl XsdValue for XsdElement {}
impl Display for XsdElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self.0 {
            Element::Node => 'n',
            Element::Way => 'w',
            Element::Relation => 'r',
        };
        write!(f, r#""{value}""#)
    }
}

pub struct XsdRelMember<'a>(pub &'a RelMember<'a>);
impl XsdValue for XsdRelMember<'_> {}
impl Display for XsdRelMember<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = match self.0.member_type {
            RelMemberType::Node => "osmnode:",
            RelMemberType::Way => "osmway:",
            RelMemberType::Relation => "osmrel:",
        };
        let id = self.0.member_id;
        write!(f, r#"{prefix}{id}"#)
    }
}

pub struct XsdIter<F>(pub F);
impl<F: Fn() -> I, I: Iterator<Item = V>, V: Display> XsdValue for XsdIter<F> {}
impl<F: Fn() -> I, I: Iterator<Item = V>, V: Display> Display for XsdIter<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.0();
        if let Some(first) = iter.next() {
            write!(f, "{first}")?;
            for item in iter {
                write!(f, ",{item}")?;
            }
        }
        Ok(())
    }
}
