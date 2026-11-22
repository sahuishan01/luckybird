//! Syntax colours.
//!
//! Fixed categories (keywords, strings, comments, ...) have one colour each.
//! Names the user chose (functions, types, variables) get a colour per distinct
//! name, drawn from a palette family per kind, so `foo` and `foo2` differ while
//! a function still looks different from a type or a variable.

use std::collections::HashMap;

pub type Rgb = (u8, u8, u8);

/// Colour for a capture name that has one fixed colour, or `None` to leave the
/// text in the default colour. Only the part before the first `.` is used.
pub fn color_for(capture: &str) -> Option<Rgb> {
    let base = capture.split('.').next().unwrap_or(capture);
    let rgb = match base {
        "keyword" | "conditional" | "repeat" | "include" | "exception" | "storageclass" => (198, 120, 221),
        "string" | "character" | "text" => (152, 195, 121),
        "comment" => (110, 118, 135),
        "function" | "method" | "constructor" => (97, 175, 239),
        "type" | "class" | "namespace" | "module" => (229, 192, 123),
        "number" | "float" | "boolean" | "constant" => (209, 154, 102),
        "property" | "attribute" | "field" | "tag" | "label" | "parameter" => (224, 108, 117),
        "escape" | "embedded" | "regexp" | "operator" => (86, 182, 194),
        "variable" => return capture.contains("builtin").then_some((224, 108, 117)),
        _ => return None,
    };
    Some(rgb)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NameKind {
    Function,
    Type,
    Variable,
}

/// Cool, saturated: functions and methods.
const FUNCTION_PALETTE: [Rgb; 12] = [
    (97, 175, 239),
    (64, 196, 170),
    (130, 130, 255),
    (80, 210, 230),
    (170, 190, 255),
    (50, 150, 210),
    (110, 230, 200),
    (150, 110, 240),
    (40, 180, 140),
    (120, 200, 255),
    (90, 120, 230),
    (180, 230, 240),
];

/// Warm, saturated: types, classes, namespaces.
const TYPE_PALETTE: [Rgb; 12] = [
    (229, 192, 123),
    (240, 150, 90),
    (255, 200, 80),
    (230, 120, 70),
    (250, 220, 140),
    (210, 170, 80),
    (255, 170, 110),
    (240, 100, 60),
    (200, 150, 60),
    (255, 235, 120),
    (225, 140, 50),
    (190, 130, 90),
];

/// Pastel, all hues: variables, parameters, properties, constants.
const VARIABLE_PALETTE: [Rgb; 12] = [
    (255, 171, 171),
    (255, 200, 150),
    (245, 235, 150),
    (200, 240, 160),
    (160, 235, 200),
    (160, 225, 240),
    (170, 200, 255),
    (200, 180, 255),
    (240, 170, 240),
    (255, 170, 210),
    (230, 200, 170),
    (190, 215, 200),
];

impl NameKind {
    const ALL: [NameKind; 3] = [NameKind::Function, NameKind::Type, NameKind::Variable];

    fn palette(self) -> &'static [Rgb; 12] {
        match self {
            NameKind::Function => &FUNCTION_PALETTE,
            NameKind::Type => &TYPE_PALETTE,
            NameKind::Variable => &VARIABLE_PALETTE,
        }
    }
}

/// Which kind of user-chosen name a capture is, if any. Builtins, macros and
/// special forms keep their fixed colour.
pub fn name_kind(capture: &str) -> Option<NameKind> {
    if ["builtin", "macro", "special"].iter().any(|w| capture.contains(w)) {
        return None;
    }
    let base = capture.split('.').next().unwrap_or(capture);
    match base {
        "function" | "method" => Some(NameKind::Function),
        "type" | "class" | "interface" | "enum" | "struct" | "namespace" | "module" | "constructor" => {
            Some(NameKind::Type)
        }
        "variable" | "parameter" | "property" | "field" | "attribute" | "constant" => Some(NameKind::Variable),
        _ => None,
    }
}

fn fnv1a(name: &str) -> u32 {
    name.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193))
}

/// Give every distinct `(kind, name)` its own colour from the kind's palette.
///
/// A name starts at its hash slot and probes forward when the slot is taken, so
/// up to 12 names of one kind never share a colour, and the same set of names
/// always gets the same colours regardless of input order.
pub fn assign_name_colors<'a>(
    names: impl Iterator<Item = (NameKind, &'a str)>,
) -> HashMap<(NameKind, &'a str), Rgb> {
    let mut all: Vec<(NameKind, &str)> = names.collect();
    all.sort_unstable();
    all.dedup();
    let mut out = HashMap::with_capacity(all.len());

    for kind in NameKind::ALL {
        let palette = kind.palette();
        let mut names: Vec<&str> = all.iter().filter(|(k, _)| *k == kind).map(|(_, n)| *n).collect();
        names.sort_by_key(|n| (fnv1a(n), *n));

        let mut used = [false; 12];
        for name in names {
            let mut slot = fnv1a(name) as usize % palette.len();
            while used[slot] && used.iter().any(|u| !u) {
                slot = (slot + 1) % palette.len();
            }
            used[slot] = true;
            out.insert((kind, name), palette[slot]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_captures_have_colours() {
        assert!(color_for("keyword").is_some());
        assert!(color_for("string.special").is_some());
        assert_eq!(color_for("function.builtin"), color_for("function"));
    }

    #[test]
    fn plain_variables_and_unknown_captures_stay_default() {
        assert_eq!(color_for("variable"), None);
        assert_eq!(color_for("punctuation.delimiter"), None);
        assert!(color_for("variable.builtin").is_some());
    }

    #[test]
    fn kinds_come_from_capture_names() {
        assert_eq!(name_kind("function"), Some(NameKind::Function));
        assert_eq!(name_kind("function.method"), Some(NameKind::Function));
        assert_eq!(name_kind("method"), Some(NameKind::Function));
        assert_eq!(name_kind("type"), Some(NameKind::Type));
        assert_eq!(name_kind("constructor"), Some(NameKind::Type));
        assert_eq!(name_kind("namespace"), Some(NameKind::Type));
        assert_eq!(name_kind("variable"), Some(NameKind::Variable));
        assert_eq!(name_kind("variable.parameter"), Some(NameKind::Variable));
        assert_eq!(name_kind("property"), Some(NameKind::Variable));
    }

    #[test]
    fn builtins_and_other_captures_have_no_name_colour() {
        for c in ["function.builtin", "type.builtin", "variable.builtin", "constant.builtin", "function.macro", "keyword", "string"] {
            assert_eq!(name_kind(c), None, "{c}");
        }
    }

    #[test]
    fn distinct_names_get_distinct_colours_within_a_kind() {
        let colors = assign_name_colors(
            [
                (NameKind::Function, "foo"),
                (NameKind::Function, "foo2"),
                (NameKind::Function, "bar"),
                (NameKind::Function, "foo"),
            ]
            .into_iter(),
        );
        assert_eq!(colors.len(), 3);
        let f = |n| colors[&(NameKind::Function, n)];
        assert_ne!(f("foo"), f("foo2"));
        assert_ne!(f("foo"), f("bar"));
        assert_ne!(f("foo2"), f("bar"));
    }

    #[test]
    fn twelve_names_of_a_kind_never_collide() {
        let names: Vec<String> = (0..12).map(|i| format!("name_{i}")).collect();
        for kind in NameKind::ALL {
            let colors = assign_name_colors(names.iter().map(|n| (kind, n.as_str())));
            let distinct: std::collections::HashSet<_> = colors.values().collect();
            assert_eq!(distinct.len(), 12, "{kind:?}");
        }
    }

    #[test]
    fn kinds_use_different_palettes() {
        let colors = assign_name_colors(
            [(NameKind::Function, "x"), (NameKind::Type, "x"), (NameKind::Variable, "x")].into_iter(),
        );
        let distinct: std::collections::HashSet<_> = colors.values().collect();
        assert_eq!(distinct.len(), 3);
    }

    #[test]
    fn colours_do_not_depend_on_input_order() {
        let a = assign_name_colors([(NameKind::Type, "A"), (NameKind::Type, "B")].into_iter());
        let b = assign_name_colors([(NameKind::Type, "B"), (NameKind::Type, "A")].into_iter());
        assert_eq!(a, b);
    }
}
