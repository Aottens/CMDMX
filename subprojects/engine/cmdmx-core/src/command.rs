//! Command line parser (Titan keypad syntax): `specs/modules/programmer.md` PRG-08, 09, 11;
//! `Palette n` is `specs/modules/palettes.md` PAL-10.
//!
//! Pure: tokens in, a parsed [`Command`] out. Resolving fixtures and applying
//! the result is the engine's job.

use crate::CoreError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Fixture,
    Group,
}

/// `From` or `From Thro To`; `from > to` means the typed direction is descending.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Item {
    pub kind: Kind,
    pub from: u32,
    pub to: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    /// Items in typed order; `true` = `Not` (remove), `false` = first item or `And` (add).
    pub selection: Vec<(bool, Item)>,
    /// The `@` level as typed (0..), before the percent conversion.
    pub level: Option<f32>,
    /// `Palette n`: apply palette number n (PAL-10).
    pub palette: Option<u32>,
}

fn number(token: Option<&&str>) -> Option<u32> {
    token?.parse().ok()
}

fn level(token: &str) -> Option<f32> {
    if token.is_empty() || !token.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    token.parse::<f32>().ok().filter(|v| v.is_finite())
}

fn selection(tokens: &[&str]) -> Option<Vec<(bool, Item)>> {
    let mut items = Vec::new();
    let mut kind = Kind::Fixture;
    let mut not = false;
    let mut it = tokens.iter().peekable();
    loop {
        match it.peek() {
            Some(&&"Fixture") => kind = Kind::Fixture,
            Some(&&"Group") => kind = Kind::Group,
            _ => {}
        }
        if matches!(it.peek(), Some(&&"Fixture") | Some(&&"Group")) {
            it.next();
        }
        let from = number(it.next())?;
        let to = if it.peek() == Some(&&"Thro") {
            it.next();
            number(it.next())?
        } else {
            from
        };
        items.push((not, Item { kind, from, to }));
        match it.next() {
            None => return Some(items),
            Some(&"And") => not = false,
            Some(&"Not") => not = true,
            Some(_) => return None,
        }
    }
}

fn parse_inner(tokens: &[&str]) -> Option<Command> {
    let (&last, body) = tokens.split_last()?;
    if last != "Enter" {
        return None;
    }
    if let Some(at) = body.iter().position(|&t| t == "Palette") {
        let (sel, rest) = body.split_at(at);
        let number = match rest {
            [_, n] => number(Some(n))?,
            _ => return None,
        };
        let selection = if sel.is_empty() {
            Vec::new()
        } else {
            selection(sel)?
        };
        return Some(Command {
            selection,
            level: None,
            palette: Some(number),
        });
    }
    let (sel, level) = match body.iter().position(|&t| t == "@") {
        Some(at) => {
            let (sel, rest) = body.split_at(at);
            let value = match rest {
                [_, v] => level(v)?,
                _ => return None,
            };
            (sel, Some(value))
        }
        None => (body, None),
    };
    let selection = if sel.is_empty() {
        // A bare `Enter` or `@` without a level is not a command.
        level?;
        Vec::new()
    } else {
        selection(sel)?
    };
    Some(Command {
        selection,
        level,
        palette: None,
    })
}

/// PRG-11: anything that does not fit the grammar is refused, naming the line.
pub fn parse(tokens: &[&str]) -> Result<Command, CoreError> {
    parse_inner(tokens).ok_or_else(|| CoreError::InvalidCommand(tokens.join(" ")))
}
