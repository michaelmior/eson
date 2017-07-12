use std::collections::{HashMap, HashSet};
use std::fmt;

use dependencies::{FD, Closure};

#[derive(Clone, Debug)]
pub struct Field {
  pub name: String,
  pub field_type: String,
  pub key: bool
}

#[derive(Debug, Default)]
pub struct Table<'a> {
  pub name: String,
  pub fields: HashMap<String, Field>,
  pub fds: HashMap<Vec<&'a str>, FD<'a>>,
}

impl<'a> PartialEq for Table<'a> {
  fn eq(&self, other: &Self) -> bool { self.name == other.name }
}

impl<'a> fmt::Display for Table<'a> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut field_names: Vec<_> = self.fields.values().map(|f|
      if f.key {
        let mut key_name = "*".to_owned();
        key_name.push_str(f.name.as_str());
        key_name
      } else {
        f.name.to_string()
      }).collect();
    field_names.sort();
    let fields = field_names.join(", ");
    write!(f, "{}({})", &self.name, &fields)
  }
}

impl<'a> Table<'a> {
  pub fn add_fd(&mut self, mut lhs: Vec<&'a str>, mut rhs: Vec<&'a str>) {
    lhs.sort();
    lhs.dedup();

    // Merge this FD with others having the same LHS
    if self.fds.contains_key(&lhs) {
      let old_fd = self.fds.remove(&lhs).unwrap();
      rhs.extend(old_fd.rhs.into_iter());
    }

    let lhs_copy = lhs.clone();
    let left_set = lhs.into_iter().collect::<HashSet<_>>();
    let right_set = rhs.into_iter().collect::<HashSet<_>>();

    self.fds.insert(lhs_copy, FD { lhs: left_set, rhs: right_set });
    self.fds.closure(None);
  }

  pub fn key_fields(&self) -> HashSet<&str> {
    self.fields.values().filter(|f| f.key).map(|f| f.name.as_str()).collect::<HashSet<_>>()
  }

  pub fn is_superkey(&self, fields: &HashSet<&str>) -> bool {
    self.key_fields().is_subset(fields)
  }

  pub fn is_bcnf(&self) -> bool {
    for fd in self.fds.values() {
      if !fd.is_trivial() && !self.is_superkey(&fd.lhs) {
        return false;
      }
    }

    true
  }

  pub fn violating_fd(&self) -> Option<&FD> {
    self.fds.values().find(|fd| !fd.is_trivial() && !self.is_superkey(&fd.lhs))
  }
}

pub enum Literal {
  Float(f64),
  Int(i64),
  Json(HashMap<String, Literal>),
  Str(String)
}

pub enum Define {
  Field(Field),
  Key(String)
}

pub enum TableOption {
  Parameter((String, Literal)),
  Order(Vec<(String, bool)>)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn tables_equal_by_name() {
      let t1 = table!("foo");
      let t2 = table!("foo");
      assert_eq!(t1, t2)
  }

  #[test]
  fn table_format_string() {
    let t = table!("foo", fields! {
      field!("bar", "String", true),
      field!("baz")
    });
    assert_eq!(format!("{}", t), "foo(*bar, baz)")
  }

  #[test]
  fn table_is_bcnf_yes() {
    let mut t = table!("foo", fields! {
      field!("foo", "String", true),
      field!("bar")
    });
    t.add_fd(vec!["foo"], vec!["bar"]);
    assert!(t.is_bcnf())
  }

  #[test]
  fn table_violating_fd() {
    let mut t = table!("foo", fields! {
      field!("foo", "String", true),
      field!("bar")
    });
    t.add_fd(vec!["bar"], vec!["foo"]);
    let fd = t.fds.values().next().unwrap();
    assert_eq!(t.violating_fd().unwrap(), fd)
  }

  #[test]
  fn table_is_bcnf_no() {
    let mut t = table!("foo", fields! {
      field!("foo", "String", true),
      field!("bar"),
      field!("baz")
    });
    t.add_fd(vec!["foo"], vec!["bar"]);
    t.add_fd(vec!["bar"], vec!["baz"]);
    assert!(!t.is_bcnf())
  }

  #[test]
  fn table_key_fields() {
    let t = table!("foo", fields! {
      field!("foo", "String", true),
      field!("bar")
    });
    let key_fields = t.key_fields();
    assert!(key_fields.contains("foo"));
    assert!(!key_fields.contains("bar"));
  }

  #[test]
  fn table_is_superkey_yes() {
    let t = table!("foo", fields! {
      field!("foo", "String", true),
      field!("bar")
    });
    let mut key = HashSet::new();
    key.insert("foo");
    key.insert("bar");
    assert!(t.is_superkey(&key))
  }

  #[test]
  fn table_is_superkey_no() {
    let t = table!("foo", fields! {
      field!("foo", "String", true),
      field!("bar")
    });
    let mut key = HashSet::new();
    key.insert("bar");
    assert!(!t.is_superkey(&key))
  }
}
