use model::{Field, Table};

struct FD<'a> {
  lhs: Vec<&'a Field>,
  rhs: Vec<&'a Field>,
}

struct IND<'a> {
  left_table: &'a Table,
  left_fields: Vec<&'a Field>,
  right_table: &'a Table,
  right_fields: Vec<&'a Field>,
}

impl<'a> IND<'a> {
  fn reverse(&self) -> IND {
    IND { left_table: self.right_table, left_fields: self.right_fields.clone(),
          right_table: self.left_table, right_fields: self.left_fields.clone() }
  }
}
