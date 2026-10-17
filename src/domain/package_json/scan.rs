//! Locating the top-level members of a JSON object without parsing it into
//! a value, so an edit can leave every other byte alone.

use std::ops::Range;

const FIELD: &str = "packageManager";

/// Where the top-level `"packageManager"` edit goes in a JSON object.
pub(super) struct Layout {
    open: usize,
    close: usize,
    first_key: Option<usize>,
    last_value_end: Option<usize>,
    value: Option<Range<usize>>,
}

impl Layout {
    /// Scans `text`, which must already be a valid JSON object.
    pub(super) fn of(text: &str) -> Option<Self> {
        let mut cursor = Cursor { text, at: 0 };
        cursor.skip_space();
        let open = cursor.at;
        cursor.expect(b'{')?;
        let mut layout = Self {
            open,
            close: 0,
            first_key: None,
            last_value_end: None,
            value: None,
        };
        loop {
            cursor.skip_space();
            match cursor.peek()? {
                b'}' => {
                    layout.close = cursor.at;
                    return Some(layout);
                }
                b',' => cursor.at += 1,
                _ => layout.member(&mut cursor)?,
            }
        }
    }

    fn member(&mut self, cursor: &mut Cursor) -> Option<()> {
        let key = cursor.string()?;
        cursor.skip_space();
        cursor.expect(b':')?;
        let value = cursor.value()?;
        self.first_key.get_or_insert(key.start);
        self.last_value_end = Some(value.end);
        if cursor.text.get(key).is_some_and(is_field) {
            self.value = Some(value);
        }
        Some(())
    }

    /// `text` with the member set to `quoted` (an encoded JSON string).
    pub(super) fn splice(&self, text: &str, quoted: &str) -> String {
        let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let member = format!("\"{FIELD}\": {quoted}");
        match (&self.value, self.last_value_end) {
            (Some(value), _) => replace(text, value.clone(), quoted),
            (None, Some(end)) => {
                let indent = self.indent(text, newline);
                replace(text, end..end, &format!(",{indent}{member}"))
            }
            (None, None) => {
                let body = format!("{newline}  {member}{newline}");
                replace(text, self.open + 1..self.close, &body)
            }
        }
    }

    /// The line break and indentation before the first member; nothing for
    /// an object written on one line.
    fn indent(&self, text: &str, newline: &str) -> String {
        let before = &text[..self.first_key.unwrap_or(0)];
        match before.rsplit_once('\n') {
            Some((_, indent)) if indent.trim().is_empty() => format!("{newline}{indent}"),
            _ => String::new(),
        }
    }
}

fn replace(text: &str, range: Range<usize>, with: &str) -> String {
    format!("{}{with}{}", &text[..range.start], &text[range.end..])
}

/// Whether a JSON string token decodes to the field name.
fn is_field(token: &str) -> bool {
    serde_json::from_str::<String>(token).is_ok_and(|name| name == FIELD)
}

struct Cursor<'a> {
    text: &'a str,
    at: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }

    fn expect(&mut self, byte: u8) -> Option<()> {
        (self.peek()? == byte).then(|| self.at += 1)
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.at += 1;
        }
    }

    /// A string token, quotes included.
    fn string(&mut self) -> Option<Range<usize>> {
        let start = self.at;
        self.expect(b'"')?;
        loop {
            match self.peek()? {
                b'\\' => self.at += 2,
                b'"' => {
                    self.at += 1;
                    return Some(start..self.at);
                }
                _ => self.at += 1,
            }
        }
    }

    /// Any value, after optional space: string, object, array or scalar.
    fn value(&mut self) -> Option<Range<usize>> {
        self.skip_space();
        let start = self.at;
        match self.peek()? {
            b'"' => return self.string(),
            b'{' | b'[' => self.nested()?,
            _ => self.scalar(),
        }
        Some(start..self.at)
    }

    fn nested(&mut self) -> Option<()> {
        let mut depth = 0_usize;
        loop {
            match self.peek()? {
                b'"' => {
                    self.string()?;
                    continue;
                }
                b'{' | b'[' => depth += 1,
                b'}' | b']' => depth = depth.checked_sub(1)?,
                _ => {}
            }
            self.at += 1;
            if depth == 0 {
                return Some(());
            }
        }
    }

    fn scalar(&mut self) {
        while self
            .peek()
            .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b',' | b'}' | b']'))
        {
            self.at += 1;
        }
    }
}
