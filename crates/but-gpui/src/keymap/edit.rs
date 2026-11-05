//! Changes to a keymap file's text that leave the rest as written: its comments, its order and
//! its spacing. The command editor makes them. The files are JSON with comments and trailing
//! commas, and order matters in them, so they're never written back from what was parsed.

use std::ops::Range;

use anyhow::{Context as _, Result, bail};
use gpui::Keystroke;

use super::sections;

/// A key in a section's `bindings`, as the text has it.
struct Pair {
    keys: String,
    /// The key, with its quotes.
    key: Range<usize>,
    /// Where its value ends.
    end: usize,
    /// The comma after it, when there is one.
    comma: Option<usize>,
}

/// A section, as the text has it.
struct SectionText {
    object: Range<usize>,
    /// The comma after it, when there is one.
    comma: Option<usize>,
    /// Between the braces of its `bindings`.
    inside: Range<usize>,
    pairs: Vec<Pair>,
}

struct FileText {
    sections: Vec<SectionText>,
    /// Where the list of sections closes.
    closing: usize,
}

struct Scanner<'a> {
    text: &'a str,
    at: usize,
}

impl Scanner<'_> {
    fn rest(&self) -> &str {
        self.text.get(self.at..).unwrap_or_default()
    }

    /// Past whitespace and comments.
    fn skip(&mut self) -> Result<()> {
        loop {
            let rest = self.rest();
            self.at += rest.len() - rest.trim_start().len();
            let rest = self.rest();
            if rest.starts_with("//") {
                self.at += rest.find('\n').unwrap_or(rest.len());
            } else if rest.starts_with("/*") {
                self.at += rest.find("*/").context("a comment doesn't end")? + 2;
            } else {
                return Ok(());
            }
        }
    }

    /// Past `c` if it comes next, after whitespace and comments, with where it was.
    fn eat(&mut self, c: char) -> Result<Option<usize>> {
        self.skip()?;
        if !self.rest().starts_with(c) {
            return Ok(None);
        }
        self.at += c.len_utf8();
        Ok(Some(self.at - c.len_utf8()))
    }

    fn expect(&mut self, c: char) -> Result<usize> {
        self.eat(c)?
            .with_context(|| format!("expected {c:?} at byte {}", self.at))
    }

    /// A string's text and where it is, quotes included.
    fn string(&mut self) -> Result<(String, Range<usize>)> {
        let start = self.expect('"')?;
        let mut escaped = false;
        for (ix, c) in self.rest().char_indices() {
            match (escaped, c) {
                (false, '"') => {
                    let range = start..self.at + ix + 1;
                    self.at = range.end;
                    let quoted = self.text.get(range.clone()).unwrap_or_default();
                    return Ok((serde_json_lenient::from_str(quoted)?, range));
                }
                (false, '\\') => escaped = true,
                (true, _) | (false, _) => escaped = false,
            }
        }
        bail!("a string doesn't end")
    }

    /// Past a value of any kind.
    fn value(&mut self) -> Result<()> {
        self.skip()?;
        match self.rest().chars().next() {
            Some('"') => {
                self.string()?;
            }
            Some('{') => {
                self.expect('{')?;
                while self.eat('}')?.is_none() {
                    self.string()?;
                    self.expect(':')?;
                    self.value()?;
                    self.eat(',')?;
                }
            }
            Some('[') => {
                self.expect('[')?;
                while self.eat(']')?.is_none() {
                    self.value()?;
                    self.eat(',')?;
                }
            }
            Some(_) => {
                let rest = self.rest();
                let end = rest.find([',', '}', ']']).context("a value doesn't end")?;
                self.at += end;
            }
            None => bail!("the file ends early"),
        }
        Ok(())
    }

    fn bindings(&mut self) -> Result<(Range<usize>, Vec<Pair>)> {
        let open = self.expect('{')?;
        let mut pairs = Vec::new();
        loop {
            if let Some(close) = self.eat('}')? {
                return Ok((open + 1..close, pairs));
            }
            let (keys, key) = self.string()?;
            self.expect(':')?;
            self.value()?;
            let end = self.at;
            let comma = self.eat(',')?;
            pairs.push(Pair {
                keys,
                key,
                end,
                comma,
            });
        }
    }

    fn section(&mut self) -> Result<SectionText> {
        let start = self.expect('{')?;
        let mut bindings = None;
        let end = loop {
            if self.eat('}')?.is_some() {
                break self.at;
            }
            let (name, _) = self.string()?;
            self.expect(':')?;
            match name.as_str() {
                "bindings" => bindings = Some(self.bindings()?),
                _ => self.value()?,
            }
            self.eat(',')?;
        };
        let (inside, pairs) = bindings.context("a section has no bindings")?;
        let comma = self.eat(',')?;
        Ok(SectionText {
            object: start..end,
            comma,
            inside,
            pairs,
        })
    }
}

fn file_text(text: &str) -> Result<FileText> {
    let mut scanner = Scanner { text, at: 0 };
    scanner.expect('[')?;
    let mut sections = Vec::new();
    loop {
        if let Some(closing) = scanner.eat(']')? {
            return Ok(FileText { sections, closing });
        }
        sections.push(scanner.section()?);
    }
}

fn section_at(file: &FileText, section: usize) -> Result<&SectionText> {
    (file.sections.get(section)).with_context(|| format!("there's no section {section}"))
}

/// `text` with each range replaced, the ranges not overlapping.
fn replaced(text: &str, mut edits: Vec<(Range<usize>, String)>) -> String {
    edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut text = text.to_owned();
    for (range, with) in edits {
        text.replace_range(range, &with);
    }
    text
}

/// Where the line with `at` in it starts.
fn line_start(text: &str, at: usize) -> usize {
    let before = text.get(..at).unwrap_or_default();
    before.rfind('\n').map_or(0, |newline| newline + 1)
}

/// Where the line after `at` starts, or the end of the text.
fn next_line(text: &str, at: usize) -> usize {
    let after = text.get(at..).unwrap_or_default();
    after
        .find('\n')
        .map_or(text.len(), |newline| at + newline + 1)
}

/// Whether only spaces come before `at` on its line.
fn starts_line(text: &str, at: usize) -> bool {
    let start = line_start(text, at);
    (text.get(start..at)).is_some_and(|before| before.trim().is_empty())
}

/// Whether only spaces or a comment come after `at` on its line.
fn ends_line(text: &str, at: usize) -> bool {
    let line = text.get(at..next_line(text, at)).unwrap_or_default();
    line.trim().is_empty() || line.trim_start().starts_with("//")
}

/// `keys` and `action` as a binding in a section writes them.
fn pair_text(keys: &str, action: &str) -> Result<String> {
    Ok(format!(
        "{}: {}",
        serde_json::to_string(keys)?,
        serde_json::to_string(action)?
    ))
}

/// Replace the keys `old` of a binding in `section` with `new`, where it is.
fn change(text: &str, section: usize, old: &str, new: &str) -> Result<String> {
    let file = file_text(text)?;
    let pair = (section_at(&file, section)?.pairs.iter())
        .find(|pair| pair.keys == old)
        .with_context(|| format!("{old:?} isn't in section {section}"))?;
    let new = serde_json::to_string(new)?;
    Ok(replaced(text, Vec::from([(pair.key.clone(), new)])))
}

/// Remove the binding of `keys` in `section`, and the section with its comment when that was
/// its last. A comment over the binding stays, as it can be about the ones after it too.
pub fn remove(text: &str, section: usize, keys: &str) -> Result<String> {
    let file = file_text(text)?;
    let this = section_at(&file, section)?;
    let ix = (this.pairs.iter())
        .position(|pair| pair.keys == keys)
        .with_context(|| format!("{keys:?} isn't in section {section}"))?;
    if this.pairs.len() == 1 {
        return Ok(replaced(text, section_removal(text, &file, section)?));
    }
    let pair = this.pairs.get(ix).context("the pair is gone")?;
    let before = ix.checked_sub(1).and_then(|ix| this.pairs.get(ix));
    let after = this.pairs.get(ix + 1);
    let end = pair.comma.map_or(pair.end, |comma| comma + 1);
    let mut edits = Vec::new();
    if starts_line(text, pair.key.start) && ends_line(text, end) {
        edits.push((
            line_start(text, pair.key.start)..next_line(text, end),
            String::new(),
        ));
        // It closed the list without a comma, so now the one before it does.
        if let (Some(before), None, None) = (before, after, pair.comma) {
            edits.extend(before.comma.map(|comma| (comma..comma + 1, String::new())));
        }
    } else if let Some(after) = after {
        edits.push((pair.key.start..after.key.start, String::new()));
    } else if let Some(before) = before {
        edits.push((before.end..end, String::new()));
    }
    Ok(replaced(text, edits))
}

/// What takes `section` and the comment right over it out of `text`.
fn section_removal(
    text: &str,
    file: &FileText,
    section: usize,
) -> Result<Vec<(Range<usize>, String)>> {
    let this = section_at(file, section)?;
    let mut start = this.object.start;
    let end = this.comma.map_or(this.object.end, |comma| comma + 1);
    let mut edits = Vec::new();
    if starts_line(text, start) && ends_line(text, end) {
        start = line_start(text, start);
        while let Some(above) = start.checked_sub(1).map(|at| line_start(text, at)) {
            let line = text.get(above..start).unwrap_or_default();
            if !line.trim_start().starts_with("//") {
                break;
            }
            start = above;
        }
        edits.push((start..next_line(text, end), String::new()));
    } else {
        edits.push((start..end, String::new()));
    }
    // It closed the list without a comma, so now the one before it does.
    let last = section + 1 == file.sections.len();
    if last && this.comma.is_none() {
        let before = section.checked_sub(1).and_then(|ix| file.sections.get(ix));
        edits.extend(
            before
                .and_then(|before| before.comma)
                .map(|comma| (comma..comma + 1, String::new())),
        );
    }
    Ok(edits)
}

/// Add a binding at the end of `section`.
fn append(text: &str, section: usize, keys: &str, action: &str) -> Result<String> {
    let file = file_text(text)?;
    let this = section_at(&file, section)?;
    let pair = pair_text(keys, action)?;
    let Some(last) = this.pairs.last() else {
        return Ok(replaced(
            text,
            Vec::from([(this.inside.clone(), format!(" {pair} "))]),
        ));
    };
    if !starts_line(text, last.key.start) {
        return Ok(replaced(
            text,
            Vec::from([(last.end..last.end, format!(", {pair}"))]),
        ));
    }
    let indent = (text.get(line_start(text, last.key.start)..last.key.start)).unwrap_or_default();
    let line_end = next_line(text, last.comma.unwrap_or(last.end));
    // A list that ends in a comma goes on doing so.
    let comma = if last.comma.is_some() { "," } else { "" };
    let mut edits = Vec::from([(line_end..line_end, format!("{indent}{pair}{comma}\n"))]);
    if last.comma.is_none() {
        edits.push((last.end..last.end, ",".to_owned()));
    }
    Ok(replaced(text, edits))
}

/// Add a section of one binding at the end of the file.
fn append_section(
    text: &str,
    keys: &str,
    action: &str,
    context: Option<&str>,
    os: Option<&[String]>,
) -> Result<String> {
    let file = file_text(text)?;
    let mut lines = Vec::from(["  {".to_owned()]);
    if let Some(os) = os {
        lines.push(format!("    \"os\": {},", serde_json::to_string(os)?));
    }
    if let Some(context) = context {
        let context = serde_json::to_string(context)?;
        lines.push(format!("    \"context\": {context},"));
    }
    lines.push("    \"bindings\": {".to_owned());
    lines.push(format!("      {}", pair_text(keys, action)?));
    lines.push("    }".to_owned());
    lines.push("  }\n".to_owned());
    let at = line_start(text, file.closing);
    let mut edits = Vec::from([(at..at, lines.join("\n"))]);
    if let Some(last) = file.sections.last().filter(|last| last.comma.is_none()) {
        edits.push((last.object.end..last.object.end, ",".to_owned()));
    }
    Ok(replaced(text, edits))
}

/// Bind `keys` to `action` in `context` on the platforms `os`: at the end of the last section
/// for them with room, as a section's `bindings` holds a key once, or in a new one.
pub fn add(
    text: &str,
    keys: &str,
    action: &str,
    context: Option<&str>,
    os: Option<&[String]>,
) -> Result<String> {
    let found = (sections(text)?.iter().enumerate().rev()).find_map(|(ix, section)| {
        let same = section.context.as_deref() == context && section.os.as_deref() == os;
        (same && !section.bindings.contains_key(keys)).then_some(ix)
    });
    match found {
        Some(section) => append(text, section, keys, action),
        None => append_section(text, keys, action, context, os),
    }
}

/// What's bound and where, for [`rekey`].
pub struct Bound<'a> {
    pub section: usize,
    pub keys: &'a str,
    pub action: &'a str,
    pub context: Option<&'a str>,
    pub os: Option<&'a [String]>,
}

/// Give a binding the keys `new`: where it is, unless its section has them for another action,
/// which a section can't hold, so then in the next place [`add`] finds.
pub fn rekey(text: &str, bound: &Bound<'_>, new: &str) -> Result<String> {
    let parsed = sections(text)?;
    let section = (parsed.get(bound.section)).context("the section is gone")?;
    if !section.bindings.contains_key(new) {
        return change(text, bound.section, bound.keys, new);
    }
    let text = remove(text, bound.section, bound.keys)?;
    add(&text, new, bound.action, bound.context, bound.os)
}

/// How a section for the platforms `section_os` writes `keystroke`, typed on `os`. The
/// platform's main modifier is `secondary`, so a key means the same everywhere, or the name it
/// has on a section's platforms. A symbol shift makes is written as itself (`?`), as the files
/// do and Windows and Linux report it apart from macOS.
pub fn written(keystroke: &Keystroke, section_os: Option<&[String]>, os: &str) -> String {
    let modifiers = keystroke.modifiers;
    let symbol = (keystroke.key_char.as_deref())
        .filter(|_| modifiers.shift && !modifiers.alt)
        .filter(|c| c.len() == 1 && c.chars().all(|c| c.is_ascii_punctuation()));
    let on_mac = os == "macos";
    let secondary = match section_os {
        None => "secondary",
        Some(names) if names.iter().all(|name| name == "macos") => "cmd",
        Some(names) if names.iter().all(|name| name != "macos") => "ctrl",
        Some(_) => "secondary",
    };
    let mut words = Vec::new();
    if (on_mac && modifiers.platform) || (!on_mac && modifiers.control) {
        words.push(secondary);
    }
    if on_mac && modifiers.control && !words.contains(&"ctrl") {
        words.push("ctrl");
    }
    if modifiers.alt {
        words.push("alt");
    }
    if modifiers.shift && symbol.is_none() {
        words.push("shift");
    }
    if !on_mac && modifiers.platform {
        words.push("super");
    }
    if modifiers.function {
        words.push("fn");
    }
    words.push(symbol.unwrap_or(&keystroke.key));
    words.join("-")
}

#[cfg(test)]
mod tests {
    use gpui::{Keystroke, Modifiers};

    use super::super::{KeymapFile, sections};
    use super::{Bound, add, append_section, change, file_text, rekey, remove, written};

    const FILE: &str = r#"// The header.
[
  // First.
  {
    "bindings": {
      // About a.
      "a": "x::A", // Trailing.
      "b": "x::B"
    }
  },
  // Second.
  {
    "context": "Diff",
    "bindings": { "c": "x::C", "d": "x::D" }
  },
  { "os": ["macos"], "context": "Diff", "bindings": { "e": "x::E" } },
  // Third.
  {
    "context": "Files",
    "bindings": {
      "f": "x::F",
      "g": "x::G",
    }
  }
]
"#;

    #[test]
    fn the_scanner_finds_every_binding_the_parser_does() {
        let found: Vec<Vec<String>> = (file_text(FILE).expect("it scans").sections.iter())
            .map(|section| section.pairs.iter().map(|p| p.keys.clone()).collect())
            .collect();
        assert_eq!(
            found,
            [
                Vec::from(["a".to_owned(), "b".to_owned()]),
                Vec::from(["c".to_owned(), "d".to_owned()]),
                Vec::from(["e".to_owned()]),
                Vec::from(["f".to_owned(), "g".to_owned()]),
            ]
        );
    }

    #[test]
    fn the_scanner_reads_the_gitbutler_keymap_as_the_parser_does() {
        scans_as_parsed(KeymapFile::GitButler.embedded());
    }

    #[test]
    fn the_scanner_reads_the_tui_keymap_as_the_parser_does() {
        scans_as_parsed(KeymapFile::Tui.embedded());
    }

    #[test]
    fn the_scanner_reads_the_shared_keys_as_the_parser_does() {
        scans_as_parsed(KeymapFile::Base.embedded());
    }

    fn scans_as_parsed(text: &str) {
        let scanned: Vec<Vec<String>> = (file_text(text).expect("it scans").sections.iter())
            .map(|section| section.pairs.iter().map(|p| p.keys.clone()).collect())
            .collect();
        let parsed: Vec<Vec<String>> = (sections(text).expect("it parses").iter())
            .map(|section| section.bindings.keys().cloned().collect())
            .collect();
        assert_eq!(scanned, parsed);
    }

    #[test]
    fn changing_keys_changes_only_them() {
        let new = change(FILE, 0, "a", "secondary-a").expect("a is there");
        assert_eq!(
            new,
            FILE.replace(r#""a": "x::A""#, r#""secondary-a": "x::A""#)
        );
    }

    #[test]
    fn keys_that_need_escaping_are_escaped() {
        let new = change(FILE, 0, "a", "\\").expect("a is there");
        let parsed = sections(&new).expect("it parses");
        assert!(
            parsed
                .first()
                .expect("a section")
                .bindings
                .contains_key("\\")
        );
    }

    #[test]
    fn changing_keys_a_section_doesnt_have_fails() {
        assert!(change(FILE, 0, "z", "y").is_err());
    }

    #[test]
    fn removing_a_binding_on_its_line_takes_the_line_with_its_trailing_comment() {
        let new = remove(FILE, 0, "a").expect("a is there");
        assert_eq!(
            new,
            FILE.replace("      \"a\": \"x::A\", // Trailing.\n", "")
        );
    }

    #[test]
    fn removing_the_last_binding_takes_the_comma_before_it() {
        let new = remove(FILE, 0, "b").expect("b is there");
        let expected = FILE
            .replace("\"x::A\", // Trailing.", "\"x::A\" // Trailing.")
            .replace("      \"b\": \"x::B\"\n", "");
        assert_eq!(new, expected);
    }

    #[test]
    fn removing_the_last_binding_keeps_a_trailing_comma_style() {
        let new = remove(FILE, 3, "g").expect("g is there");
        assert_eq!(new, FILE.replace("      \"g\": \"x::G\",\n", ""));
    }

    #[test]
    fn removing_the_first_binding_on_a_shared_line_leaves_the_second() {
        let new = remove(FILE, 1, "c").expect("c is there");
        assert_eq!(new, FILE.replace(r#""c": "x::C", "#, ""));
    }

    #[test]
    fn removing_the_last_binding_on_a_shared_line_leaves_the_first() {
        let new = remove(FILE, 1, "d").expect("d is there");
        assert_eq!(new, FILE.replace(r#", "d": "x::D""#, ""));
    }

    #[test]
    fn removing_a_sections_only_binding_takes_the_section() {
        let new = remove(FILE, 2, "e").expect("e is there");
        let line = "  { \"os\": [\"macos\"], \"context\": \"Diff\", \"bindings\": { \"e\": \"x::E\" } },\n";
        assert_eq!(new, FILE.replace(line, ""));
    }

    #[test]
    fn removing_the_files_last_section_takes_the_comma_before_it_and_its_comment() {
        let file = "[\n  // One.\n  { \"bindings\": { \"a\": \"x::A\" } },\n  // Two.\n  { \"bindings\": { \"b\": \"x::B\" } }\n]\n";
        let new = remove(file, 1, "b").expect("b is there");
        assert_eq!(
            new,
            "[\n  // One.\n  { \"bindings\": { \"a\": \"x::A\" } }\n]\n"
        );
    }

    #[test]
    fn adding_goes_last_in_the_last_section_for_the_context() {
        let new = add(FILE, "z", "x::Z", Some("Diff"), None).expect("it adds");
        assert_eq!(
            new,
            FILE.replace(r#""d": "x::D" }"#, r#""d": "x::D", "z": "x::Z" }"#)
        );
    }

    #[test]
    fn adding_to_a_multiline_section_keeps_a_trailing_comment_on_its_line() {
        let file = "[\n  {\n    \"bindings\": {\n      \"a\": \"x::A\" // Why.\n    }\n  }\n]\n";
        let new = add(file, "z", "x::Z", None, None).expect("it adds");
        assert_eq!(
            new,
            "[\n  {\n    \"bindings\": {\n      \"a\": \"x::A\", // Why.\n      \"z\": \"x::Z\"\n    }\n  }\n]\n"
        );
    }

    #[test]
    fn adding_after_a_trailing_comma_keeps_the_style() {
        let new = add(FILE, "z", "x::Z", Some("Files"), None).expect("it adds");
        assert_eq!(
            new,
            FILE.replace(
                "      \"g\": \"x::G\",\n",
                "      \"g\": \"x::G\",\n      \"z\": \"x::Z\",\n"
            )
        );
    }

    #[test]
    fn adding_skips_a_section_that_has_the_keys() {
        let new = add(FILE, "c", "x::Other", Some("Diff"), None).expect("it adds");
        let parsed = sections(&new).expect("it parses");
        let last = parsed.last().expect("a section");
        assert_eq!(last.context.as_deref(), Some("Diff"));
        assert_eq!(last.bindings.get("c").map(String::as_str), Some("x::Other"));
    }

    #[test]
    fn adding_for_a_new_context_appends_a_section() {
        let new = add(FILE, "z", "x::Z", Some("New"), None).expect("it adds");
        let end = "      \"g\": \"x::G\",\n    }\n  }\n]\n";
        let with_section = "      \"g\": \"x::G\",\n    }\n  },\n  {\n    \"context\": \"New\",\n    \"bindings\": {\n      \"z\": \"x::Z\"\n    }\n  }\n]\n";
        assert_eq!(new, FILE.replace(end, with_section));
        assert!(sections(&new).is_ok());
    }

    #[test]
    fn a_new_section_writes_its_platforms() {
        let os = Vec::from(["macos".to_owned()]);
        let new = append_section("[\n]\n", "z", "x::Z", None, Some(&os)).expect("it adds");
        let parsed = sections(&new).expect("it parses");
        assert_eq!(parsed.first().and_then(|s| s.os.clone()), Some(os));
    }

    #[test]
    fn rekeying_is_in_place_when_the_section_has_room() {
        let bound = Bound {
            section: 0,
            keys: "a",
            action: "x::A",
            context: None,
            os: None,
        };
        let new = rekey(FILE, &bound, "z").expect("it rekeys");
        assert_eq!(new, FILE.replace(r#""a": "x::A""#, r#""z": "x::A""#));
    }

    #[test]
    fn rekeying_onto_keys_the_section_has_for_another_action_keeps_both() {
        let bound = Bound {
            section: 1,
            keys: "c",
            action: "x::C",
            context: Some("Diff"),
            os: None,
        };
        let new = rekey(FILE, &bound, "d").expect("it rekeys");
        let parsed = sections(&new).expect("it parses");
        let actions: Vec<&str> = (parsed.iter())
            .filter(|section| section.context.as_deref() == Some("Diff"))
            .filter_map(|section| section.bindings.get("d").map(String::as_str))
            .collect();
        assert_eq!(actions, ["x::D", "x::C"]);
    }

    #[test]
    fn every_edit_of_the_gitbutler_keymap_keeps_its_comments() {
        let text = KeymapFile::GitButler.embedded();
        let new = add(text, "f13", "app::Undo", None, None).expect("it adds");
        let new = change(&new, 0, "ctrl-y", "ctrl-shift-y").expect("it's there");
        assert_eq!(comments(&new), comments(text));
        assert!(sections(&new).is_ok());
    }

    #[test]
    fn every_edit_of_the_tui_keymap_keeps_its_comments() {
        let text = KeymapFile::Tui.embedded();
        let new = add(text, "f13", "app::Undo", None, None).expect("it adds");
        let new = remove(&new, 1, "y").expect("it's there");
        assert_eq!(comments(&new), comments(text));
        assert!(sections(&new).is_ok());
    }

    fn comments(text: &str) -> Vec<&str> {
        (text.lines())
            .filter(|line| line.trim_start().starts_with("//"))
            .collect()
    }

    fn keystroke(text: &str) -> Keystroke {
        Keystroke::parse(text).expect("a keystroke")
    }

    #[test]
    fn cmd_is_secondary_in_a_section_for_every_platform() {
        assert_eq!(written(&keystroke("cmd-k"), None, "macos"), "secondary-k");
    }

    #[test]
    fn ctrl_is_secondary_off_macos() {
        assert_eq!(written(&keystroke("ctrl-k"), None, "linux"), "secondary-k");
    }

    #[test]
    fn ctrl_stays_ctrl_on_macos() {
        assert_eq!(written(&keystroke("ctrl-k"), None, "macos"), "ctrl-k");
    }

    #[test]
    fn a_macos_section_writes_cmd() {
        let os = ["macos".to_owned()];
        assert_eq!(written(&keystroke("cmd-k"), Some(&os), "macos"), "cmd-k");
    }

    #[test]
    fn a_windows_and_linux_section_writes_ctrl_for_cmd() {
        let os = ["windows".to_owned(), "linux".to_owned()];
        assert_eq!(written(&keystroke("cmd-k"), Some(&os), "macos"), "ctrl-k");
    }

    #[test]
    fn a_control_key_pressed_on_a_mac_keeps_control_for_other_platforms() {
        let os = Vec::from(["windows".to_owned(), "linux".to_owned()]);
        assert_eq!(written(&keystroke("ctrl-y"), Some(&os), "macos"), "ctrl-y");
    }

    #[test]
    fn command_and_control_on_a_mac_write_control_once_for_other_platforms() {
        let os = Vec::from(["windows".to_owned(), "linux".to_owned()]);
        assert_eq!(
            written(&keystroke("ctrl-cmd-y"), Some(&os), "macos"),
            "ctrl-y"
        );
    }

    #[test]
    fn modifiers_go_in_the_files_order() {
        assert_eq!(
            written(&keystroke("shift-alt-cmd-k"), None, "macos"),
            "secondary-alt-shift-k"
        );
    }

    #[test]
    fn a_shifted_letter_keeps_shift() {
        let typed = Keystroke {
            modifiers: Modifiers::shift(),
            key: "y".to_owned(),
            key_char: Some("Y".to_owned()),
        };
        assert_eq!(written(&typed, None, "linux"), "shift-y");
    }

    #[test]
    fn a_shifted_symbol_is_written_as_itself() {
        let typed = Keystroke {
            modifiers: Modifiers::shift(),
            key: "/".to_owned(),
            key_char: Some("?".to_owned()),
        };
        assert_eq!(written(&typed, None, "linux"), "?");
    }

    #[test]
    fn a_symbol_macos_reports_as_itself_stays() {
        assert_eq!(written(&keystroke("cmd-}"), None, "macos"), "secondary-}");
    }

    #[test]
    fn minus_with_a_modifier_parses_back() {
        let text = written(&keystroke("cmd--"), None, "macos");
        assert_eq!(text, "secondary--");
        assert_eq!(keystroke(&text).key, "-");
    }
}
