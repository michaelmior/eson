use std::collections::{HashMap, HashSet};

use dependencies::{FD, Closure};

pub struct Field {
  pub name: String,
  pub field_type: String,
  pub key: bool
}

#[derive(Default)]
pub struct Table {
  pub name: String,
  pub fields: HashMap<String, Field>,
  pub fds: HashMap<Vec<String>, FD>,
}

impl PartialEq for Table {
  fn eq(&self, other: &Self) -> bool { self.name == other.name }
}

impl Table {
  pub fn add_fd(&mut self, mut lhs: Vec<String>, mut rhs: Vec<String>) {
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
