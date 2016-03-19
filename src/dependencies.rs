use model::{Field, Table};

pub struct FD {
  pub lhs: Vec<String>,
  pub rhs: Vec<String>,
}

pub struct IND<'a> {
  pub left_table: &'a Table,
  pub left_fields: Vec<String>,
  pub right_table: &'a Table,
  pub right_fields: Vec<String>,
}

impl<'a> IND<'a> {
  fn reverse(&self) -> IND {
    IND { left_table: self.right_table, left_fields: self.right_fields.clone(),
          right_table: self.left_table, right_fields: self.left_fields.clone() }
  }
}
