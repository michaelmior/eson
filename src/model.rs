use std::collections::{HashMap, HashSet};
use std::fmt;

use dependencies::{FD, Closure};

#[derive(Debug)]
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
    let field_names: Vec<_> = self.fields.keys().map(|key| key.to_string()).collect();
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

  pub fn is_bcnf(&self) -> bool {
    for fd in self.fds.values() {
      if !fd.rhs.is_subset(&fd.lhs) {
        let keys = self.fields.values().filter(|f| f.key).map(|f| f.name.as_str()).collect::<HashSet<_>>();
        if !fd.lhs.is_subset(&keys) {
          return false
        }
      }
    }

    true
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
      let t = Table {
        name: "foo".to_string(),
        fields: map! {
          "foo".to_string() => Field {
            name: "foo".to_string(),
            field_type: "String".to_string(),
            key: false
          }
        },
        ..Default::default()
      };
      assert_eq!(format!("{}", t), "foo(foo)")
  }

  #[test]
  fn table_is_bcnf_yes() {
    let mut t = Table {
      name: "foo".to_string(),
      fields: map! {
        "foo".to_string() => Field {
          name: "foo".to_string(),
          field_type: "String".to_string(),
          key: true
        },
        "bar".to_string() => Field {
          name: "bar".to_string(),
          field_type: "String".to_string(),
          key: false
        }
      },
      ..Default::default()
    };
    t.add_fd(vec!["foo"], vec!["bar"]);
    assert!(t.is_bcnf())
  }

  #[test]
  fn table_is_bcnf_no() {
    let mut t = Table {
      name: "foo".to_string(),
      fields: map! {
        "foo".to_string() => Field {
          name: "foo".to_string(),
          field_type: "String".to_string(),
          key: true
        },
        "bar".to_string() => Field {
          name: "bar".to_string(),
          field_type: "String".to_string(),
          key: false
        },
        "baz".to_string() => Field {
          name: "baz".to_string(),
          field_type: "String".to_string(),
          key: false
        }
      },
      ..Default::default()
    };
    t.add_fd(vec!["foo"], vec!["bar"]);
    t.add_fd(vec!["bar"], vec!["baz"]);
    assert!(!t.is_bcnf())
  }
}
