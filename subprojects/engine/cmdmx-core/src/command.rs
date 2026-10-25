//! Command line parser (Titan keypad syntax): `specs/modules/programmer.md` PRG-08, 09, 11;
//! `Palette n` is `specs/modules/palettes.md` PAL-10, a selection ending in `Locate` PRG-10.
//!
//! Pure: tokens in, a parsed [`Command`] out. Resolving fixtures and applying
//! the result is the engine's job.

use crate::CoreError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Fixture,
    Group,
    /// PRG-48: the `.`-first forms, the selected fixtures.
    Selected,
}

/// PRG-48: the cells of an item.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cells {
    All,
    /// `from > to` means the typed direction is descending.
    Range {
        from: u32,
        to: u32,
    },
}

/// `From` or `From Thro To`; `from > to` means the typed direction is descending.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Item {
    pub kind: Kind,
    pub from: u32,
    pub to: u32,
    /// PRG-48: `None` = the whole fixture.
    pub cells: Option<Cells>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    /// Items in typed order; `true` = `Not` (remove), `false` = first item or `And` (add).
    pub selection: Vec<(bool, Item)>,
    /// The `@` level as typed (0..), before the percent conversion.
    pub level: Option<f32>,
    /// `Palette n`: apply palette number n (PAL-10).
    pub palette: Option<u32>,
    /// A selection ended with `Locate` instead of `Enter` (PRG-10).
    pub locate: bool,
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

type Tokens<'a, 'b> = std::iter::Peekable<std::slice::Iter<'a, &'b str>>;

fn peek_number(it: &mut Tokens) -> Option<u32> {
    it.peek().and_then(|t| t.parse().ok())
}

/// After a `.`: an optional cell number.
fn dot_cell(it: &mut Tokens) -> Option<Cells> {
    Some(match peek_number(it) {
        Some(m) => {
            it.next();
            Cells::Range { from: m, to: m }
        }
        None => Cells::All,
    })
}

/// One item after any `Fixture`/`Group` word (PRG-48 grammar); `from..to` are the fixtures.
fn item(it: &mut Tokens, kind: Kind) -> Option<Item> {
    let from = number(it.next())?;
    let (mut to, mut cells) = (from, None);
    if it.peek() == Some(&&"Thro") {
        it.next();
        to = number(it.next())?;
        if it.peek() == Some(&&".") {
            it.next();
            cells = dot_cell(it);
        }
    } else if it.peek() == Some(&&".") {
        it.next();
        cells = dot_cell(it);
        if let Some(Cells::Range { from: m, .. }) = cells {
            if it.peek() == Some(&&"Thro") {
                it.next();
                if it.peek() == Some(&&".") {
                    it.next();
                    cells = Some(Cells::Range {
                        from: m,
                        to: number(it.next())?,
                    });
                } else {
                    let x = number(it.next())?;
                    if it.peek() == Some(&&".") {
                        it.next();
                        to = x;
                        cells = Some(Cells::Range {
                            from: m,
                            to: number(it.next())?,
                        });
                    } else {
                        cells = Some(Cells::Range { from: m, to: x });
                    }
                }
            }
        }
    }
    (kind == Kind::Fixture || cells.is_none()).then_some(Item {
        kind,
        from,
        to,
        cells,
    })
}

fn selection(tokens: &[&str]) -> Option<Vec<(bool, Item)>> {
    let mut items = Vec::new();
    let mut kind = Kind::Fixture;
    let mut not = false;
    let mut it = tokens.iter().peekable();
    loop {
        let one = if it.peek() == Some(&&".") {
            it.next();
            let cells = match dot_cell(&mut it)? {
                Cells::Range { from, .. } if it.peek() == Some(&&"Thro") => {
                    it.next();
                    (it.next() == Some(&".")).then_some(())?;
                    Cells::Range {
                        from,
                        to: number(it.next())?,
                    }
                }
                c => c,
            };
            Item {
                kind: Kind::Selected,
                from: 0,
                to: 0,
                cells: Some(cells),
            }
        } else {
            match it.peek() {
                Some(&&"Fixture") => kind = Kind::Fixture,
                Some(&&"Group") => kind = Kind::Group,
                _ => {}
            }
            if matches!(it.peek(), Some(&&"Fixture") | Some(&&"Group")) {
                it.next();
            }
            item(&mut it, kind)?
        };
        items.push((not, one));
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
    if last == "Locate" {
        return Some(Command {
            selection: selection(body)?,
            level: None,
            palette: None,
            locate: true,
        });
    }
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
            locate: false,
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
        locate: false,
    })
}

/// PRG-11: anything that does not fit the grammar is refused, naming the line.
pub fn parse(tokens: &[&str]) -> Result<Command, CoreError> {
    parse_inner(tokens).ok_or_else(|| CoreError::InvalidCommand(tokens.join(" ")))
}
