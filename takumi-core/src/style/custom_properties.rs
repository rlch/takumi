//! The custom properties in scope on an element: their specified values and the
//! `@property` rules that govern them.
//!
//! Shaped as Stylo's `ComputedCustomProperties`: the registrations are one
//! registry per stylesheet, shared by every element; the values an element
//! hands its children live apart from the values that stay on it, so a child
//! shares its parent's inherited map until it sets something; and a registered
//! name with no value reads its registration's initial value rather than one
//! stored on every element.

use std::{
  collections::{HashMap, HashSet},
  sync::{Arc, LazyLock},
};

use crate::{style::selector::PropertyRule, viewport::Viewport};

/// The `@property` rules in effect, keyed by name; the last rule for a name wins.
pub(crate) type Registrations = Arc<HashMap<String, PropertyRule>>;

/// The empty map an element starts its own values from, shared by all of them.
static NO_VALUES: LazyLock<Arc<HashMap<String, String>>> = LazyLock::new(Arc::default);

/// The custom properties an element resolves `var()` against, with the
/// `@property` registrations that decide how each one inherits.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomProperties {
  /// Values a child inherits: every unregistered name, and the registered
  /// names that inherit. Shared with the parent until this element sets one.
  inherited: Arc<HashMap<String, String>>,
  /// Values that stay on this element: registered names that do not inherit,
  /// and the names a utility engine wrote as this element's own state.
  non_inherited: Arc<HashMap<String, String>>,
  registrations: Registrations,
  /// Names a utility engine wrote as this element's own composition state.
  /// Unlike a registration, this does not reach the children.
  element_state: Arc<HashSet<String>>,
}

impl Default for CustomProperties {
  fn default() -> Self {
    Self {
      inherited: NO_VALUES.clone(),
      non_inherited: NO_VALUES.clone(),
      registrations: Registrations::default(),
      element_state: Arc::default(),
    }
  }
}

/// Collects the registrations of `rules` whose media queries match, in order,
/// so a later rule for a name replaces an earlier one.
pub(crate) fn collect_registrations<'r>(
  rules: impl IntoIterator<Item = &'r PropertyRule>,
  viewport: Viewport,
) -> Registrations {
  Arc::new(
    rules
      .into_iter()
      .filter(|rule| {
        rule
          .media_queries
          .iter()
          .all(|query| query.matches(viewport))
      })
      .map(|rule| (rule.name.clone(), rule.clone()))
      .collect(),
  )
}

impl CustomProperties {
  /// The value of `name` before `var()` substitution: the specified value in
  /// reach, or else the registered initial value.
  pub fn get(&self, name: &str) -> Option<&str> {
    let value = match self.registration(name) {
      Some(rule) => self.map_for(rule).get(name),
      None => self
        .non_inherited
        .get(name)
        .or_else(|| self.inherited.get(name)),
    };

    value
      .map(String::as_str)
      .or_else(|| self.registration(name)?.initial_value.as_deref())
  }

  /// Whether `name` has a value, specified or initial.
  pub fn contains(&self, name: &str) -> bool {
    self.get(name).is_some()
  }

  fn map_for(&self, rule: &PropertyRule) -> &HashMap<String, String> {
    if rule.inherits {
      &self.inherited
    } else {
      &self.non_inherited
    }
  }

  pub(crate) fn set(&mut self, name: String, value: String) {
    let inherits = match self.registration(&name) {
      Some(rule) => rule.inherits,
      None => !self.element_state.contains(&name),
    };

    let map = if inherits {
      &mut self.inherited
    } else {
      &mut self.non_inherited
    };
    Arc::make_mut(map).insert(name, value);
  }

  /// The `@property` rule governing `name`, if one registered it.
  pub(crate) fn registration(&self, name: &str) -> Option<&PropertyRule> {
    self.registrations.get(name)
  }

  #[cfg(test)]
  pub(crate) fn register(&mut self, rule: PropertyRule) {
    Arc::make_mut(&mut self.registrations).insert(rule.name.clone(), rule);
  }

  /// Whether every registration in `registrations` already governs these
  /// properties. A pointer check when they come from the same stylesheet.
  pub(crate) fn has_registrations(&self, registrations: &Registrations) -> bool {
    Arc::ptr_eq(&self.registrations, registrations)
      || registrations
        .iter()
        .all(|(name, rule)| self.registration(name) == Some(rule))
  }

  /// Puts a stylesheet's registrations in scope, over any already here.
  pub(crate) fn adopt_registrations(&mut self, registrations: &Registrations) {
    if self.registrations.is_empty() {
      self.registrations = registrations.clone();
      return;
    }

    Arc::make_mut(&mut self.registrations).extend(
      registrations
        .iter()
        .map(|(name, rule)| (name.clone(), rule.clone())),
    );
  }

  /// Records state a utility engine wrote for this element alone. Tailwind's
  /// own stylesheet says so with an `@property` rule; the engine has no
  /// stylesheet, so it says so here. An author's rule for the name wins.
  pub(crate) fn register_element_state(&mut self, name: &str) {
    if self.registration(name).is_some() {
      return;
    }

    Arc::make_mut(&mut self.element_state).insert(name.to_owned());

    // What an ancestor set under the name stays on this element and stops here.
    if let Some(value) = self.inherited.get(name).cloned() {
      Arc::make_mut(&mut self.inherited).remove(name);
      Arc::make_mut(&mut self.non_inherited).insert(name.to_owned(), value);
    }
  }

  /// The properties a child starts from: the parent's inherited values and
  /// registrations, shared, and none of the values that stayed on the parent.
  pub(crate) fn inherited(&self) -> Self {
    Self {
      inherited: self.inherited.clone(),
      non_inherited: NO_VALUES.clone(),
      registrations: self.registrations.clone(),
      element_state: Default::default(),
    }
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use super::{CustomProperties, collect_registrations};
  use crate::{style::selector::PropertyRule, viewport::Viewport};

  fn rule(name: &str, inherits: bool, initial_value: &str) -> PropertyRule {
    PropertyRule {
      name: name.to_owned(),
      syntax: "*".to_owned(),
      inherits,
      initial_value: Some(initial_value.to_owned()),
      media_queries: Vec::new(),
    }
  }

  fn with_theme() -> CustomProperties {
    let rules = [rule("--tw-shadow", false, "0 0 #0000")];
    let mut properties = CustomProperties::default();
    properties.adopt_registrations(&collect_registrations(&rules, Viewport::default()));
    properties.set("--color-red".to_owned(), "red".to_owned());
    properties
  }

  /// A registered name nobody set reads its initial value, which no element stores.
  #[test]
  fn an_unset_registered_name_reads_its_initial_value() {
    let properties = with_theme();

    assert_eq!(properties.get("--tw-shadow"), Some("0 0 #0000"));
    assert!(properties.non_inherited.is_empty());
  }

  /// Setting a non-inheriting name leaves the inherited values shared with the
  /// parent, and the child starts from them again without a copy.
  #[test]
  fn a_child_shares_the_inherited_values_its_parent_did_not_change() {
    let parent = with_theme();
    let mut element = parent.inherited();
    element.set("--tw-shadow".to_owned(), "0 1px red".to_owned());
    let child = element.inherited();

    assert!(Arc::ptr_eq(&parent.inherited, &element.inherited));
    assert!(Arc::ptr_eq(&element.inherited, &child.inherited));
    assert_eq!(element.get("--tw-shadow"), Some("0 1px red"));
    assert_eq!(child.get("--tw-shadow"), Some("0 0 #0000"));
    assert_eq!(child.get("--color-red"), Some("red"));
  }
}
