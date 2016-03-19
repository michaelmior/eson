use std::collections::HashMap;

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
  pub fds: Vec<FD>,
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
