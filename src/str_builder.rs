use std::fmt::{Debug, Display, Write as _};
use std::ops::{Deref, DerefMut};

use crate::parser::{
    RE_SIMPLE_LOCAL_NAME, RE_WIKIDATA_MULTI_VALUE, RE_WIKIDATA_VALUE, RE_WIKIPEDIA_VALUE,
};
use json::JsonValue;
use percent_encoding::utf8_percent_encode;

use crate::utils;
use crate::utils::PERCENT_ENC_SET;

#[repr(transparent)]
pub struct StringBuf {
    buf: String,
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

    pub fn push_str_value(&mut self, predicate: &'static str, value: &str) {
        self.push_str(predicate);
        self.push(' ');
        self.push_quoted_str(value);
    }

    pub fn push_quoted_str(&mut self, value: &str) {
        writeln!(self, "{};", JsonValue::from(value).dump()).unwrap();
    }

    pub fn push_char_value(&mut self, predicate: &'static str, value: char) {
        writeln!(self, r#"{predicate} "{value}";"#).unwrap();
    }

    pub fn push_bool_value(&mut self, predicate: &'static str, value: bool) {
        let value = if value { "true" } else { "false" };
        writeln!(self, r#"{predicate} "{value}"^^xsd:boolean;"#).unwrap();
    }

    pub fn push_int_value(&mut self, predicate: &'static str, value: i64) {
        writeln!(self, r#"{predicate} "{value}"^^xsd:integer;"#).unwrap();
    }

    pub fn push_date_value(&mut self, predicate: &'static str, milli_timestamp: i64) {
        // "{0:%Y-%m-%dT%H:%M:%S}Z"^^xsd:dateTime
        let ts = utils::to_utc(milli_timestamp);
        writeln!(self, r#"{predicate} "{ts}"^^xsd:dateTime;"#,).unwrap();
    }

    pub fn push_point(&mut self, predicate: &'static str, lat: f64, lon: f64) {
        writeln!(self, r#"{predicate} "Point({lon} {lat})"^^geo:wktLiteral;"#).unwrap();
    }

    pub fn push_tag(&mut self, key: &str, value: &str) {
        if !RE_SIMPLE_LOCAL_NAME.is_match(key) {
            // Record any unusual tag name in a "osmm:badkey" statement
            self.push_str_value("osmm:badkey", value);
            return;
        }

        write!(self, "osmt:{key} ").unwrap();
        let mut parsed = false;
        if key.contains("wikidata") {
            if RE_WIKIDATA_VALUE.is_match(value) {
                write!(self, "wd:{value}").unwrap();
                parsed = true;
            } else if RE_WIKIDATA_MULTI_VALUE.is_match(value) {
                for v in value.split(';') {
                    write!(self, "wd:{v};").unwrap();
                }
                self.pop(); // remove trailing ","
                parsed = true;
            }
        } else if key.contains("wikipedia") {
            if let Some(v) = RE_WIKIPEDIA_VALUE.captures(value) {
                let lang = v.get(1).unwrap().as_str();
                let title = v.get(2).unwrap().as_str();
                let title = title.replace(' ', "_");
                let title = utf8_percent_encode(&title, PERCENT_ENC_SET);
                writeln!(self, "<https://{lang}.wikipedia.org/wiki/{title}>;").unwrap();
                parsed = true;
            }
        }
        if !parsed {
            self.push_quoted_str(value);
        }
    }

    pub fn push_metadata(
        &mut self,
        version: i32,
        user: &str,
        milli_timestamp: i64,
        changeset: i64,
    ) {
        self.push_int_value("osmm:version", version as i64);
        self.push_str_value("osmm:user", user);
        self.push_date_value("osmm:timestamp", milli_timestamp);
        self.push_int_value("osmm:changeset", changeset);
        self.pop(); // remove trailing "\n"
        self.pop(); // remove trailing ";"
        self.push_str(".\n");
    }
}
