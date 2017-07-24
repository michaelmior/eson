use std::collections::{HashMap, HashSet};
use std::fmt;

use dependencies::{FD, FDClosure, IND};
use symbols::{FieldName, TableName};

/// A schema encapsulating tables and their dependencies
#[derive(Default)]
pub struct Schema {
  /// Tables keyed by their name
  pub tables: HashMap<TableName, Table>,

  /// Inclusion dependencies between tables
  pub inds: HashMap<(TableName, TableName), Vec<IND>>
}

impl fmt::Display for Schema {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    for table in self.tables.values() {
      writeln!(f, "{}", table)?;
      for fd in table.fds.values() {
        writeln!(f, "  {}", fd)?;
      }
      writeln!(f)?;
    }

    for ind_group in self.inds.values() {
      for ind in ind_group {
        writeln!(f, "{}", ind)?;
      }
    }

    Ok(())
  }
}

impl Schema {
  /// Add a new `IND` to the schema
  pub fn add_ind(&mut self, ind: IND) {
    let ind_key = (ind.left_table.clone(), ind.right_table.clone());
    if self.inds.contains_key(&ind_key) {
      let ind_list = self.inds.get_mut(&ind_key).unwrap();
      ind_list.push(ind);
    } else {
      self.inds.insert(ind_key, vec![ind]);
    }
  }

  #[allow(dead_code)]
  pub fn delete_ind(&mut self, ind: &IND) {
    for inds in self.inds.values_mut() {
      let index = inds.iter().position(|i| i == ind);
      if index.is_some() {
        inds.remove(index.unwrap());
      }
    }
  }

  /// Check if this schema contains a given IND
  pub fn contains_ind(&self, ind: &IND) -> bool {
    self.inds.values().any(|inds| inds.iter().any(|i| i == ind))
  }

  /// Copy `IND`s from the table in `src` to the table in `dst`
  pub fn copy_inds(&mut self, src: &TableName, dst: &TableName) {
    let mut new_inds = Vec::new();
    {
      let dst_table = &self.tables[dst];
      for ind_group in self.inds.values() {
        for ind in ind_group {
          if ind.left_table == *src {
            let mut new_lhs = ind.left_fields.clone();
            new_lhs.retain(|f| dst_table.fields.contains_key(f));

            if !new_lhs.is_empty() {
              let new_ind = IND { left_table: src.clone(),
                                  left_fields: new_lhs,
                                  right_table: ind.right_table.clone(),
                                  right_fields: ind.right_fields.clone() };
              new_inds.push(new_ind);
            }
          }

          if ind.right_table == *src {
            let mut new_rhs = ind.right_fields.clone();
            new_rhs.retain(|f| dst_table.fields.contains_key(f));

            if !new_rhs.is_empty() {
              let new_ind = IND { left_table: ind.left_table.clone(),
                                  left_fields: ind.left_fields.clone(),
                                  right_table: src.clone(),
                                  right_fields: new_rhs };
              new_inds.push(new_ind);
            }
          }
        }
      }
    }

    for new_ind in new_inds {
      self.add_ind(new_ind);
    }
  }

  /// Prune `IND`s which reference tables which no longer exist
  pub fn prune_inds(&mut self) {
    let tables = self.tables.keys().collect::<HashSet<&TableName>>();
    self.inds.retain(|key, _|
      tables.contains(&key.0) && tables.contains(&key.1)
    );

    for inds in self.inds.values_mut() {
      for ind in inds.iter_mut() {
        // Get the indexes of all fields in each table to keep
        let left_table = &self.tables[&ind.left_table];
        let right_table = &self.tables[&ind.right_table];
        let left_indexes = ind.left_fields.iter().enumerate().filter(|&(_, field)|
          left_table.fields.contains_key(field)
        ).map(|(i, _)| i).collect::<HashSet<_>>();
        let right_indexes = ind.right_fields.iter().enumerate().filter(|&(_, field)|
          right_table.fields.contains_key(field)
        ).map(|(i, _)| i).collect::<HashSet<_>>();

        // We can only keep fields which are in both tables
        let retain_indexes = left_indexes.intersection(&right_indexes).collect::<HashSet<_>>();
        for index in (0..ind.left_fields.len()).rev() {
          if !retain_indexes.contains(&index) {
            ind.left_fields.remove(index);
            ind.right_fields.remove(index);
          }
        }
      }
    }

    // Remove any INDs which are now empty
    for inds in self.inds.values_mut() {
      inds.retain(|ind| !ind.left_fields.is_empty() && !ind.right_fields.is_empty());
    }
  }
}

/// A field inside a `Table`
#[derive(Clone, Debug)]
pub struct Field {
  /// The name of the field
  pub name: FieldName,

  /// Whether this field is a key of its parent `Table`
  pub key: bool
}

/// A table, it's field and any intra-table dependencies
#[derive(Debug)]
pub struct Table {
  /// The name of the table
  pub name: TableName,

  /// All `Field`s in the table keyed by the name
  pub fields: HashMap<FieldName, Field>,

  /// Functional dependencies keyed by their left-hand side
  pub fds: HashMap<Vec<FieldName>, FD>,
}

impl Default for Table {
  fn default() -> Table {
    Table { name: TableName::from(""), fields: HashMap::new(), fds: HashMap::new() }
  }
}

impl PartialEq for Table {
  fn eq(&self, other: &Self) -> bool { self.name == other.name }
}

impl fmt::Display for Table {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut field_names: Vec<_> = self.fields.values().map(|f|
      if f.key {
        let mut key_name = "*".to_owned();
        key_name.push_str(f.name.as_ref());
        key_name
      } else {
        f.name.to_string()
      }).collect();
    field_names.sort();
    let fields = field_names.join(", ");
    write!(f, "{}({})", &self.name, &fields)
  }
}

impl Table {
  /// Add a new `FD` to this table
  pub fn add_fd(&mut self, mut lhs: Vec<FieldName>, mut rhs: Vec<FieldName>) {
    lhs.sort();
    lhs.dedup();

    // Merge this FD with others having the same LHS
    let key = &lhs.iter().map(|f| f.clone()).collect::<Vec<_>>();
    if self.fds.contains_key(key) {
      let old_fd = self.fds.remove(key).unwrap();
      rhs.extend(old_fd.rhs.into_iter());
    }

    let left_set = lhs.into_iter().collect::<HashSet<_>>();
    let right_set = rhs.into_iter().collect::<HashSet<_>>();

    self.fds.insert(key.clone(), FD { lhs: left_set, rhs: right_set });
    self.fds.closure();
  }

  /// Copy `FD`s from another given `Table`
  pub fn copy_fds(&mut self, other: &Table) {
    for fd in other.fds.values() {
      let new_lhs = fd.lhs.iter().map(|f| f.to_string().parse().unwrap())
          .filter(|f| self.fields.contains_key(f)).collect::<Vec<_>>();
      let new_rhs = fd.rhs.iter().map(|f| f.to_string().parse().unwrap())
          .filter(|f| self.fields.contains_key(f)).collect::<Vec<_>>();
      if !new_lhs.is_empty() && !new_rhs.is_empty() {
        self.add_fd(new_lhs, new_rhs);
      }
    }
  }

  /// Produce all fields marked as a key
  pub fn key_fields(&self) -> HashSet<FieldName> {
    self.fields.values().filter(|f| f.key).map(|f| f.name.clone()).collect::<HashSet<_>>()
  }

  /// Check if a set of fields is a superkey for this table
  pub fn is_superkey(&self, fields: &HashSet<FieldName>) -> bool {
    self.key_fields().is_subset(fields)
  }

  /// Check if this table is in BCNF according to its functional dependencies
  pub fn is_bcnf(&self) -> bool {
    self.violating_fd().is_none()
  }

  /// Find a functional dependency which violates BCNF
  pub fn violating_fd(&self) -> Option<&FD> {
    self.fds.values().find(|fd|
      !fd.is_trivial() &&
      !self.is_superkey(&fd.lhs)
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn tables_equal_by_name() {
      let t1 = table!("foo");
      let t2 = table!("foo");
      assert_eq!(t1, t2)
  }

  #[test]
  fn table_format_string() {
    let t = table!("foo", fields! {
      field!("bar", true),
      field!("baz")
    });
    assert_eq!(format!("{}", t), "foo(*bar, baz)")
  }

  #[test]
  fn table_is_bcnf_yes() {
    let mut t = table!("foo", fields! {
      field!("foo", true),
      field!("bar")
    });
    add_fd!(t, vec!["foo"], vec!["bar"]);
    assert!(t.is_bcnf())
  }

  #[test]
  fn table_violating_fd() {
    let mut t = table!("foo", fields! {
      field!("foo", true),
      field!("bar")
    });
    add_fd!(t, vec!["bar"], vec!["foo"]);
    let fd = t.fds.values().next().unwrap();
    assert_eq!(t.violating_fd().unwrap(), fd)
  }

  #[test]
  fn table_no_violating_fd() {
    let mut t = table!("foo", fields! {
      field!("foo", true),
      field!("bar")
    });
    add_fd!(t, vec!["foo"], vec!["bar"]);
    assert!(t.violating_fd().is_none())
  }

  #[test]
  fn table_is_bcnf_no() {
    let mut t = table!("foo", fields! {
      field!("foo", true),
      field!("bar"),
      field!("baz")
    });
    add_fd!(t, vec!["foo"], vec!["bar"]);
    add_fd!(t, vec!["bar"], vec!["baz"]);
    assert!(!t.is_bcnf())
  }

  #[test]
  fn table_key_fields() {
    let t = table!("foo", fields! {
      field!("foo", true),
      field!("bar")
    });
    let key_fields = t.key_fields();
    assert!(key_fields.contains("foo"));
    assert!(!key_fields.contains("bar"));
  }

  #[test]
  fn table_is_superkey_yes() {
    let t = table!("foo", fields! {
      field!("foo", true),
      field!("bar")
    });
    let key = collect![as HashSet<_>: FieldName::from("foo"), FieldName::from("bar")];
    assert!(t.is_superkey(&key))
  }

  #[test]
  fn table_is_superkey_no() {
    let t = table!("foo", fields! {
      field!("foo", true),
      field!("bar")
    });
    let key = collect![as HashSet<_>: FieldName::from("bar")];
    assert!(!t.is_superkey(&key))
  }

  #[test]
  fn table_copy_fds() {
    let mut t1 = table!("foo", fields! {
      field!("foo", true),
      field!("bar"),
      field!("baz")
    });
    let mut t2 = table!("foo", fields! {
      field!("foo", true),
      field!("bar")
    });
    add_fd!(t1, vec!["foo"], vec!["bar"]);
    add_fd!(t1, vec!["foo"], vec!["baz"]);
    t2.copy_fds(&t1);

    let copied_fd = FD { lhs: collect!["foo".parse().unwrap()],
                         rhs: collect!["bar".parse().unwrap()] };
    let copied_fds = t2.fds.values().collect::<Vec<_>>();
    assert_eq!(vec![&copied_fd], copied_fds)
  }

  #[test]
  fn schema_contains_ind() {
    let t1 = table!("foo", fields! {
      field!("bar", true)
    });
    let t2 = table!("baz", fields! {
      field!("quux", true)
    });
    let mut schema = schema! {t1, t2};
    add_ind!(schema, "foo", vec!["bar"], "baz", vec!["quux"]);

    let ind = IND { left_table: TableName::from("foo"),
                    left_fields: vec![FieldName::from("bar")],
                    right_table: TableName::from("baz"),
                    right_fields: vec![FieldName::from("quux")] };
    assert!(schema.contains_ind(&ind))
  }

  #[test]
  fn schema_prune_inds_yes() {
    let t = table!("foo", fields! {
      field!("bar", true)
    });
    let mut schema = schema! {t};
    add_ind!(schema, "foo", vec!["bar"], "baz", vec!["quux"]);
    schema.prune_inds();
    assert_eq!(schema.inds.len(), 0)
  }

  #[test]
  fn schema_prune_inds_no() {
    let t1 = table!("foo", fields! {
      field!("bar", true)
    });
    let t2 = table!("baz", fields! {
      field!("quux", true)
    });
    let mut schema = schema! {t1, t2};
    add_ind!(schema, "foo", vec!["bar"], "baz", vec!["quux"]);
    schema.prune_inds();
    assert_eq!(schema.inds.len(), 1)
  }

  #[test]
  fn schema_prune_inds_fields() {
    let t1 = table!("foo", fields! {
      field!("bar", true)
    });
    let t2 = table!("qux", fields! {
      field!("quux", true)
    });

    let mut schema = schema! {t1, t2};
    add_ind!(schema, "foo", vec!["bar", "baz"], "qux", vec!["quux", "corge"]);

    schema.prune_inds();

    let ind = schema.inds.values().next().unwrap().iter().next().unwrap();

    assert_eq!(ind.left_fields.len(), 1);
    assert_eq!(ind.left_fields.iter().next().unwrap(),
               &FieldName::from("bar"));

    assert_eq!(ind.right_fields.len(), 1);
    assert_eq!(ind.right_fields.iter().next().unwrap(),
               &FieldName::from("quux"));
  }

  #[test]
  fn schema_prune_inds_fields_one_side() {
    let t1 = table!("foo", fields! {
      field!("bar", true)
    });
    let t2 = table!("qux", fields! {
      field!("quux", true),
      field!("corge")
    });

    let mut schema = schema! {t1, t2};
    add_ind!(schema, "foo", vec!["bar", "baz"], "qux", vec!["quux", "corge"]);

    schema.prune_inds();

    let ind = schema.inds.values().next().unwrap().iter().next().unwrap();

    assert_eq!(ind.left_fields.len(), 1);
    assert_eq!(ind.left_fields.iter().next().unwrap(),
               &FieldName::from("bar"));

    assert_eq!(ind.right_fields.len(), 1);
    assert_eq!(ind.right_fields.iter().next().unwrap(),
               &FieldName::from("quux"));
  }
}
