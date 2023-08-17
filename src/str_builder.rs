use std::fmt::{Debug, Display, Write as _};
use std::ops::{Deref, DerefMut};

use json::JsonValue;
use osmpbf::{RelMember, RelMemberType};

use crate::utils;
use crate::utils::Element;

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
            f.write_str(r##"indoc! {r#""##)?;
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
            f.write_str(r##""}#""##)
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

    pub fn push_value(&mut self, predicate: impl Display, value: impl XsdValue) {
        writeln!(self, r#"{predicate} {value};"#).unwrap();
    }

    pub fn push_metadata(
        &mut self,
        version: i32,
        user: &str,
        milli_timestamp: i64,
        changeset: i64,
    ) {
        let value = version as i64;
        self.push_value("osmm:version", XsdInteger(value));
        self.push_value("osmm:user", XsdStr(user));
        self.push_value("osmm:timestamp", XsdDateTime(milli_timestamp));
        self.push_value("osmm:changeset", XsdInteger(changeset));
        self.pop(); // remove trailing "\n"
        self.pop(); // remove trailing ";"
        self.push_str(".\n");
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

// pub struct XsdChar(pub char);
// impl XsdValue for XsdChar {}
// impl Display for XsdChar {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, r#""{}""#, self.0)
//     }
// }

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

pub struct XsdIter<F, I, V>(pub F)
where
    F: Fn() -> I,
    I: Iterator<Item = V>,
    V: Display;
impl<F, I, V> XsdValue for XsdIter<F, I, V>
where
    F: Fn() -> I,
    I: Iterator<Item = V>,
    V: Display,
{
}
impl<F, I, V> Display for XsdIter<F, I, V>
where
    F: Fn() -> I,
    I: Iterator<Item = V>,
    V: Display,
{
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
