#[cfg(test)]
macro_rules! assert_has_key(
  ($table:expr, $field_names:expr) => {{
    let key_fields = $table.key_fields();
    for field_name in $field_names {
      assert!(key_fields.contains(&field_name),
        format!("key {} missing from {}", field_name, $table.name));
    }
    assert!(key_fields.len() == $field_names.len(),
      format!("{} has additional keys", $table.name));
  }};
);

#[cfg(test)]
macro_rules! field_vec(
  { $($field:expr),+ } => {
    vec! [ $(FieldName::from($field)),+ ]
  };
);

#[cfg(test)]
macro_rules! field_set(
  { $($field:expr),+ } => {
    collect![as HashSet<_>: $(FieldName::from($field)),+ ]
  };
);

#[cfg(test)]
macro_rules! assert_fields(
  ($table:expr, $field_names:expr, true) => {{
    let fields = $table.fields.keys().map(|f| f.clone()).into_iter().collect::<Vec<_>>();
    assert_eq!(fields, $field_names)
  }};
  ($table:expr, $field_names:expr, false) => {{
    for field_name in $field_names {
      assert!(!$table.fields.contains_key(&field_name),
        format!("{} found in {}", field_name, $table.name));
    }
  }};
);

#[cfg(test)]
macro_rules! assert_has_fields(
  ($table:expr, $field_names:expr) => {{
    assert_fields!($table, $field_names, true);
  }};
);

#[cfg(test)]
macro_rules! assert_missing_fields(
  ($table:expr, $field_names:expr) => {{
    assert_fields!($table, $field_names, false);
  }};
);

#[cfg(test)]
macro_rules! fields(
  { $($field:expr),+ } => {
    collect! [ $($field.name => $field),+ ]
  };
);

#[cfg(test)]
macro_rules! field(
  ($name:expr) => {
    Field {
      name: FieldName::from($name),
      key: false,
      cardinality: None,
      max_length: None
    }
  };
  ($name:expr, $key:expr) => {
    Field {
      name: FieldName::from($name),
      key: $key,
      cardinality: None,
      max_length: None
    }
  }
);

#[cfg(test)]
macro_rules! table(
  ($name:expr) => {
    Table { name: TableName::from($name), ..Default::default() }
  };
  ($name:expr, $fields:expr) => {
    Table { name: TableName::from($name), fields: $fields, ..Default::default() }
  };
);

#[cfg(test)]
macro_rules! add_fd(
  ($table:expr, $lhs:expr, $rhs:expr) => {{
    let mut lhs = $lhs.iter().map(|f| FieldName::from(f)).collect::<Vec<_>>();
    lhs.sort();

    let mut rhs = $rhs.iter().map(|f| FieldName::from(f)).collect::<Vec<_>>();
    rhs.sort();

    $table.add_fd(lhs, rhs);
  }};
);

#[cfg(test)]
macro_rules! add_ind(
  ($schema:expr, $left_table:expr, $left_fields:expr, $right_table:expr, $right_fields:expr) => {{
    extern crate permutation;

    let lhs = $left_fields.iter().map(|f| FieldName::from(f)).collect::<Vec<_>>();
    let permutation = permutation::sort(&lhs[..]);

    let rhs = $right_fields.iter().map(|f| FieldName::from(f)).collect::<Vec<_>>();

    $schema.add_ind(IND {
      left_table: TableName::from($left_table),
      left_fields: permutation.apply_slice(&lhs[..]),
      right_table: TableName::from($right_table),
      right_fields: permutation.apply_slice(&rhs[..])
    });
  }};
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
