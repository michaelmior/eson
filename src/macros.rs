#[cfg(test)]
macro_rules! map(
  { $($key:expr => $value:expr),+ } => {
    {
      let mut m = ::std::collections::HashMap::new();
      $(m.insert($key, $value);)+
      m
    }
  };
);

#[cfg(test)]
macro_rules! fields(
  { $($field:expr),+ } => {
    map! { $($field.name => $field),+ }
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
