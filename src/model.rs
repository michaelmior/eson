use std::collections::{HashMap, HashSet};

use dependencies::{FD};

pub struct Field {
  pub name: String,
  pub field_type: String,
  pub key: bool
}

#[derive(Default)]
pub struct Table {
  pub name: String,
  pub fields: HashMap<String, Field>,
  pub fds: HashSet<FD>,
}

impl Table {
  pub fn add_fd(&mut self, mut lhs: Vec<String>, mut rhs: Vec<String>) {
    lhs.sort();
    rhs.sort();
    self.fds.insert(FD { lhs: lhs, rhs: rhs });
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
