#[cfg(test)]
macro_rules! fields(
  { $($field:expr),+ } => {
    collect! [ $($field.name => $field),+ ]
  };
);

#[cfg(test)]
macro_rules! field(
  ($name:expr) => {
    Field { name: $name.parse().unwrap(), field_type: "".to_string(), key: false }
  };
  ($name:expr, $field_type:expr) => {
    Field { name: $name.parse().unwrap(), field_type: $field_type.to_string(), key: false }
  };
  ($name:expr, $field_type:expr, $key:expr) => {
    Field { name: $name.parse().unwrap(), field_type: $field_type.to_string(), key: $key }
  }
);

#[cfg(test)]
macro_rules! table(
  ($name:expr) => {
    Table { name: $name.parse().unwrap(), ..Default::default() }
  };
  ($name:expr, $fields:expr) => {
    Table { name: $name.parse().unwrap(), fields: $fields, ..Default::default() }
  };
);

#[cfg(test)]
macro_rules! add_fd(
  ($table:expr, $lhs:expr, $rhs:expr) => {
    $table.add_fd(
      $lhs.iter().map(|f| f.parse().unwrap()).collect::<Vec<_>>(),
      $rhs.iter().map(|f| f.parse().unwrap()).collect::<Vec<_>>()
    );
  };
);

#[cfg(test)]
macro_rules! add_ind(
  ($schema:expr, $left_table:expr, $left_fields:expr, $right_table:expr, $right_fields:expr) => {
    $schema.add_ind(IND {
      left_table: $left_table.parse().unwrap(),
      left_fields: $left_fields.iter().map(|f| f.parse().unwrap()).collect::<Vec<_>>(),
      right_table: $right_table.parse().unwrap(),
      right_fields: $right_fields.iter().map(|f| f.parse().unwrap()).collect::<Vec<_>>()
    });
  };
);

#[cfg(test)]
macro_rules! schema(
  ($($table:expr),+) => {
    Schema {
      tables: collect![$($table.name.to_string().parse().unwrap() => $table),+],
      ..Default::default()
    }
  };
);
