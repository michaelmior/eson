macro_rules! map(
  { $($key:expr => $value:expr),+ } => {
    {
      let mut m = ::std::collections::HashMap::new();
      $(m.insert($key, $value);)+
      m
    }
  };
);

macro_rules! table(
  { $name:expr } => {
    Table { name: $name.to_string(), ..Default::default() }
  };
);
