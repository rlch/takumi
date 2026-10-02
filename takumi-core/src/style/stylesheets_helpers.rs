use std::borrow::Cow;

use cssparser::{Delimiter, ParseError, Parser, ParserInput, Token, parse_important};

use super::{LonghandId, ParsedDeclarations, PropertyId, ShorthandId};
use crate::style::{CssInput, CssWideKeyword, FromCss};

impl CssWideKeyword {
  /// The keyword a string input spells, if any.
  pub(crate) fn from_css_input(css_input: &CssInput<'_>) -> Option<Self> {
    let CssInput::Str(value) = css_input else {
      return None;
    };
    let mut parser_input = ParserInput::new(value.as_ref());
    let mut parser = Parser::new(&mut parser_input);

    Self::from_css(&mut parser).ok()
  }
}

impl PropertyId {
  pub(crate) fn from_name(name: &str, normalize: fn(&str) -> Cow<'_, str>) -> PropertyId {
    if name.starts_with("--") {
      return PropertyId::Custom;
    }

    let normalized = normalize(name);

    Self::resolve(normalized.as_ref())
      .or_else(|| strip_vendor_prefix(normalized.as_ref()).and_then(Self::resolve))
      .unwrap_or(PropertyId::Ignored)
  }

  fn resolve(normalized: &str) -> Option<PropertyId> {
    if let Some(property) = legacy_alias_property_id(normalized) {
      return Some(property);
    }

    match PropertyId::from_normalized_name(normalized) {
      PropertyId::Ignored => None,
      property => Some(property),
    }
  }
}

// Ref: https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/row-gap
fn legacy_alias_property_id(name: &str) -> Option<PropertyId> {
  match name {
    "grid_gap" => Some(PropertyId::Shorthand(ShorthandId::Gap)),
    "grid_row_gap" => Some(PropertyId::Longhand(LonghandId::RowGap)),
    "grid_column_gap" => Some(PropertyId::Longhand(LonghandId::ColumnGap)),
    // `continue` is a Rust keyword; its longhand field is `r#continue`, so the
    // name-derived lookup can't reach it.
    "continue" => Some(PropertyId::Longhand(LonghandId::Continue)),
    "page_break_before" => Some(PropertyId::Longhand(LonghandId::BreakBefore)),
    "page_break_after" => Some(PropertyId::Longhand(LonghandId::BreakAfter)),
    "page_break_inside" => Some(PropertyId::Longhand(LonghandId::BreakInside)),
    _ => None,
  }
}

fn strip_vendor_prefix(normalized: &str) -> Option<&str> {
  ["webkit_", "moz_", "ms_", "o_"]
    .into_iter()
    .find_map(|prefix| normalized.strip_prefix(prefix))
}

pub(crate) fn expand_shorthand<T>(
  value: T,
  expand: impl FnOnce(T, &mut ParsedDeclarations),
) -> ParsedDeclarations {
  let mut declarations = ParsedDeclarations::new();
  expand(value, &mut declarations);
  declarations
}

pub(crate) fn normalize_kebab_property_name(name: &str) -> Cow<'_, str> {
  if !name
    .bytes()
    .any(|byte| byte == b'-' || byte.is_ascii_uppercase())
  {
    return Cow::Borrowed(name);
  }

  let normalized = name
    .chars()
    .map(|ch| match ch {
      '-' => '_',
      _ => ch.to_ascii_lowercase(),
    })
    .collect();

  without_leading_underscores(normalized)
}

pub(crate) fn normalize_camel_property_name(name: &str) -> Cow<'_, str> {
  if !name.starts_with('_') && !name.bytes().any(|byte| byte.is_ascii_uppercase()) {
    return Cow::Borrowed(name);
  }

  let mut normalized = String::with_capacity(name.len() + 4);
  for ch in name.chars() {
    if ch.is_ascii_uppercase() {
      normalized.push('_');
      normalized.push(ch.to_ascii_lowercase());
    } else {
      normalized.push(ch);
    }
  }

  without_leading_underscores(normalized)
}

fn without_leading_underscores(mut normalized: String) -> Cow<'static, str> {
  let leading = normalized.len() - normalized.trim_start_matches('_').len();

  normalized.drain(..leading);
  Cow::Owned(normalized)
}

pub(crate) fn contains_var_function(specified_value: &str) -> bool {
  fn contains_in_parser(input: &mut Parser<'_, '_>) -> bool {
    loop {
      let should_check_nested_block = match input.next_including_whitespace_and_comments() {
        Ok(Token::Function(name)) if name.eq_ignore_ascii_case("var") => return true,
        Ok(
          Token::Function(_)
          | Token::ParenthesisBlock
          | Token::SquareBracketBlock
          | Token::CurlyBracketBlock,
        ) => true,
        Ok(_) => false,
        Err(_) => break,
      };

      if should_check_nested_block
        && input
          .parse_nested_block(|input| {
            Ok::<_, ParseError<'_, Cow<'_, str>>>(contains_in_parser(input))
          })
          .unwrap_or(true)
      {
        return true;
      }
    }

    false
  }

  let mut parser_input = ParserInput::new(specified_value);
  let mut parser = Parser::new(&mut parser_input);
  contains_in_parser(&mut parser)
}

/// Advances to the first `!` at the top level, leaving it unread. A value can
/// carry one for its own sake, so the caller decides whether what follows is an
/// importance marker.
pub(crate) fn skip_to_bang(parser: &mut Parser<'_, '_>) {
  let _ = parser.parse_until_before(Delimiter::Bang, |parser| {
    while parser.next_including_whitespace_and_comments().is_ok() {}

    Ok::<_, ParseError<'_, ()>>(())
  });
}

/// The byte index where a trailing `!important` starts, if the value ends with one.
pub(crate) fn important_start(value: &str) -> Option<usize> {
  if !value.contains('!') {
    return None;
  }

  let mut parser_input = ParserInput::new(value);
  let mut parser = Parser::new(&mut parser_input);

  skip_to_bang(&mut parser);
  let end = parser.position().byte_index();

  (parse_important(&mut parser).is_ok() && parser.is_exhausted()).then_some(end)
}

/// Splits a trailing `!important` off a declaration value.
pub(crate) fn split_important(css_input: CssInput<'_>) -> (CssInput<'_>, bool) {
  let CssInput::Str(value) = css_input else {
    return (css_input, false);
  };

  let Some(end) = important_start(value.as_ref()) else {
    return (CssInput::Str(value), false);
  };

  let value = match value {
    Cow::Borrowed(value) => Cow::Borrowed(&value[..end]),
    Cow::Owned(mut value) => {
      value.truncate(end);
      Cow::Owned(value)
    }
  };

  (CssInput::Str(value), true)
}
