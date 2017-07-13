use std::collections::{HashMap, HashSet};
use std::fmt;

use dependencies::{FD, FDClosure, IND};
use symbols::{FieldName, TableName};

pub struct Schema {
  pub tables: HashMap<TableName, Table>,
  pub inds: HashMap<(TableName, TableName), Vec<IND>>
}

#[derive(Clone, Debug)]
pub struct Field {
  pub name: FieldName,
  pub field_type: String,
  pub key: bool
}

#[derive(Debug)]
pub struct Table {
  pub name: TableName,
  pub fields: HashMap<FieldName, Field>,
  pub fds: HashMap<Vec<FieldName>, FD>,
}

impl Default for Table {
  fn default() -> Table {
    Table { name: TableName::from(""), fields: HashMap::new(), fds: HashMap::new() }
  }
}

impl PartialEq for Table {
  fn eq(&self, other: &Self) -> bool { self.name == other.name }
}

impl fmt::Display for Table {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut field_names: Vec<_> = self.fields.values().map(|f|
      if f.key {
        let mut key_name = "*".to_owned();
        key_name.push_str(f.name.as_ref());
        key_name
      } else {
        f.name.to_string()
      }).collect();
    field_names.sort();
    let fields = field_names.join(", ");
    write!(f, "{}({})", &self.name, &fields)
  }
}

impl Table {
  pub fn add_fd(&mut self, mut lhs: Vec<FieldName>, mut rhs: Vec<FieldName>) {
    lhs.sort();
    lhs.dedup();

    // Merge this FD with others having the same LHS
    let key = &lhs.iter().map(|f| f.clone()).collect::<Vec<_>>();
    if self.fds.contains_key(key) {
      let old_fd = self.fds.remove(key).unwrap();
      rhs.extend(old_fd.rhs.into_iter());
    }

    let left_set = lhs.into_iter().collect::<HashSet<_>>();
    let right_set = rhs.into_iter().collect::<HashSet<_>>();

    self.fds.insert(key.clone(), FD { lhs: left_set, rhs: right_set });
    self.fds.closure();
  }

  pub fn copy_fds(&mut self, other: &Table) {
    for fd in other.fds.values() {
      let new_lhs = fd.lhs.iter().map(|f| f.to_string().parse().unwrap())
          .filter(|f| self.fields.contains_key(f)).collect::<Vec<_>>();
      let new_rhs = fd.rhs.iter().map(|f| f.to_string().parse().unwrap())
          .filter(|f| self.fields.contains_key(f)).collect::<Vec<_>>();
      if new_lhs.len() > 0 && new_rhs.len() > 0 {
        self.add_fd(new_lhs, new_rhs);
      }
    }
  }

  pub fn key_fields(&self) -> HashSet<&str> {
    self.fields.values().filter(|f| f.key).map(|f| f.name.as_ref()).collect::<HashSet<_>>()
  }

  pub fn is_superkey(&self, fields: &HashSet<&str>) -> bool {
    self.key_fields().is_subset(fields)
  }

  pub fn is_bcnf(&self) -> bool {
    self.violating_fd().is_none()
  }

  pub fn violating_fd(&self) -> Option<&FD> {
    self.fds.values().find(|fd|
      !fd.is_trivial() &&
      !self.is_superkey(&fd.lhs.iter().map(|f| f.as_ref()).collect::<HashSet<_>>())
    )
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
  Key(FieldName)
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
    t.add_fd(vec!["foo".parse().unwrap()], vec!["bar".parse().unwrap()]);
    assert!(t.is_bcnf())
  }

  #[test]
  fn table_violating_fd() {
    let mut t = table!("foo", fields! {
      field!("foo", "String", true),
      field!("bar")
    });
    t.add_fd(vec!["bar".parse().unwrap()], vec!["foo".parse().unwrap()]);
    let fd = t.fds.values().next().unwrap();
    assert_eq!(t.violating_fd().unwrap(), fd)
  }

  #[test]
  fn table_no_violating_fd() {
    let mut t = table!("foo", fields! {
      field!("foo", "String", true),
      field!("bar")
    });
    t.add_fd(vec!["foo".parse().unwrap()], vec!["bar".parse().unwrap()]);
    assert!(t.violating_fd().is_none())
  }

  #[test]
  fn table_is_bcnf_no() {
    let mut t = table!("foo", fields! {
      field!("foo", "String", true),
      field!("bar"),
      field!("baz")
    });
    t.add_fd(vec!["foo".parse().unwrap()], vec!["bar".parse().unwrap()]);
    t.add_fd(vec!["bar".parse().unwrap()], vec!["baz".parse().unwrap()]);
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

  #[test]
  fn table_copy_fds() {
    let mut t1 = table!("foo", fields! {
      field!("foo", "String", true),
      field!("bar"),
      field!("baz")
    });
    let mut t2 = table!("foo", fields! {
      field!("foo", "String", true),
      field!("bar")
    });
    t1.add_fd(vec!["foo".parse().unwrap()], vec!["bar".parse().unwrap()]);
    t1.add_fd(vec!["foo".parse().unwrap()], vec!["baz".parse().unwrap()]);
    t2.copy_fds(t1);

    let copied_fd = FD {
      lhs: collect! [ "foo".parse().unwrap() ],
      rhs: collect! [ "bar".parse().unwrap() ]
    };
    let copied_fds = t2.fds.values().collect::<Vec<_>>();
    assert_eq!(vec![&copied_fd], copied_fds)
  }
}
