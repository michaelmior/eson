use std::collections::HashMap;

pub struct Field {
  pub name: String,
  pub field_type: String,
  pub key: bool
}

pub struct Table {
  pub name: String,
  pub fields: HashMap<String, Field>
}

pub enum Define {
  Field(Field),
  Key(String)
}
