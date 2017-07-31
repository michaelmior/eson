use std::collections::{HashMap, HashSet};
use std::fmt;

use defaultmap::DefaultHashMap;

use dependencies::{FD, FDClosure, IND};
use symbols::{FieldName, TableName};

/// A schema encapsulating tables and their dependencies
#[derive(Default)]
pub struct Schema {
  /// Tables keyed by their name
  pub tables: HashMap<TableName, Table>,

  /// Inclusion dependencies between tables
  pub inds: DefaultHashMap<(TableName, TableName), Vec<IND>>
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
    self.inds.get_mut(ind_key).push(ind);
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
            if ind.left_fields.iter().any(|f| dst_table.fields.contains_key(f)) {
              let new_ind = IND { left_table: dst.clone(),
                                  left_fields: ind.left_fields.clone(),
                                  right_table: ind.right_table.clone(),
                                  right_fields: ind.right_fields.clone() };
              new_inds.push(new_ind);
            }
          }

          if ind.right_table == *src {
            if ind.right_fields.iter().any(|f| dst_table.fields.contains_key(f)) {
              let new_ind = IND { left_table: ind.left_table.clone(),
                                  left_fields: ind.left_fields.clone(),
                                  right_table: dst.clone(),
                                  right_fields: ind.right_fields.clone() };
              new_inds.push(new_ind);
            }
          }
        }
      }
    }

    for new_ind in new_inds {
      self.add_ind(new_ind);
    }

    self.prune_inds();
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

  // Copy FDs between tables based on inclusion dependencies
  pub fn copy_fds(&mut self) {
    let mut new_fds = Vec::new();

    // Loop over INDs
    for ind_vec in self.inds.values() {
      for ind in ind_vec.iter() {
        let mut left_fields = <HashSet<_>>::new();
        for field in self.tables.get(&ind.left_table).unwrap().fields.keys() {
          left_fields.insert(field.clone());
        }
        let left_key = self.tables.get(&ind.left_table).unwrap()
            .fields.values().filter(|f| f.key).map(|f| f.name.clone()).into_iter().collect::<HashSet<_>>();

        new_fds.extend(self.tables.get(&ind.right_table).unwrap().fds.values().map(|fd| {
          let fd_lhs = fd.lhs.clone().into_iter().collect::<HashSet<_>>();
          let fd_rhs = fd.rhs.clone().into_iter().collect::<HashSet<_>>();

          // Check that the fields in the LHS of the FD are a subset of the
          // primary key for the table and that the RHS contains new fields
          let implies_fd = fd_lhs.is_subset(&left_key) &&
                           !fd_rhs.is_disjoint(&left_fields);

          if implies_fd {
            let left_vec = fd.lhs.clone().into_iter().collect::<Vec<_>>();
            let right_vec = fd.rhs.clone().into_iter().filter(|f| left_fields.contains(f)).collect::<Vec<_>>();
            Some((ind.left_table.clone(), left_vec, right_vec))
          } else {
            None
          }
        }).filter(|x| x.is_some()).map(|x| x.unwrap()));

      }
    }

    // Add any new FDs which were found
    for fd in new_fds {
      self.tables.get_mut(&fd.0).unwrap().add_fd(fd.1, fd.2);
    }
  }

  /// Check that all of the `FD`s in the schema are valid
  #[cfg(test)]
  fn validate_fds(&self) {
    for table in self.tables.values() {
      for (key, fd) in table.fds.iter() {
        // Ensure the key in the hash table is correct
        let mut lhs = fd.lhs.iter().map(|f| (*f).clone()).collect::<Vec<_>>();
        lhs.sort();
        assert_eq!(lhs, *key, "FD key {:?} does not match for {}", key, fd);

        // Check that the table contains all the fields
        assert!(fd.lhs.iter().all(|f| table.fields.contains_key(f)),
          "Missing fields for LHS of {}", fd);
        assert!(fd.rhs.iter().all(|f| table.fields.contains_key(f)),
          "Missing fields for RHS of {}", fd);
      }
    }
  }

  /// Check that all of the `IND`s in the schema are valid
  #[cfg(test)]
  fn validate_inds(&self) {
    for (ind_key, inds) in self.inds.iter() {
      for ind in inds {
        assert_eq!(*ind_key, (ind.left_table.clone(), ind.right_table.clone()),
          "IND key {:?} does not match for {}", ind_key, ind);

        // Check that the left table and its fields exist
        let left_table = self.tables.get(&ind.left_table)
          .expect(&format!("Table {} not found for IND {}", ind.left_table, ind));
        assert!(ind.left_fields.iter().all(|f| left_table.fields.contains_key(f)),
          "Missing fields for LHS of {}", ind);

        // Check that the right table and its fields exist
        let right_table = self.tables.get(&ind.right_table)
          .expect(&format!("Table {} not found for IND {}", ind.right_table, ind));
        assert!(ind.right_fields.iter().all(|f| right_table.fields.contains_key(f)),
          "Missing fields for RHS of {}", ind);
      }
    }
  }

  /// Ensure all the dependencies are consistent with the tables
  #[cfg(test)]
  pub fn validate(&self) {
    self.validate_fds();
    self.validate_inds();
  }
}

/// A field inside a `Table`
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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

  /// Prune `FD`s which reference fields which no longer exist
  pub fn prune_fds(&mut self) {
    let fields = self.fields.keys().collect::<HashSet<_>>();
    for fd in self.fds.values_mut() {
      fd.lhs.retain(|f| fields.contains(&f));
      fd.rhs.retain(|f| fields.contains(&f));
    }

    self.fds.retain(|_, fd| !fd.lhs.is_empty() && !fd.rhs.is_empty());
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
  fn prune_fds() {
    let mut t = table!("foo", fields! {
      field!("foo", true),
      field!("bar")
    });
    add_fd!(t, vec!["quux"], vec!["qux"]);
    t.prune_fds();
    assert!(t.fds.len() == 0)
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
  fn schema_copy_inds() {
    let t1 = table!("foo", fields! {
      field!("bar", true),
      field!("baz")
    });
    let t2 = table!("quux", fields! {
      field!("bar", true),
      field!("baz")
    });
    let t3 = table!("corge", fields! {
      field!("grault", true),
      field!("garply")
    });
    let mut schema = schema! {t1, t2, t3};
    add_ind!(schema, "quux", vec!["bar", "baz"], "corge", vec!["grault", "garply"]);

    schema.validate();
    schema.copy_inds(&TableName::from("quux"), &TableName::from("foo"));
    schema.validate();

    let inds = &schema.inds[&(TableName::from("foo"), TableName::from("corge"))];
    assert_eq!(inds.len(), 1);

    let ind = &inds[0];
    assert_eq!(ind.left_fields, field_vec!["bar", "baz"]);
    assert_eq!(ind.right_fields, field_vec!["grault", "garply"]);
  }

  #[test]
  fn schema_copy_inds_partial() {
    let t1 = table!("foo", fields! {
      field!("bar", true)
    });
    let t2 = table!("quux", fields! {
      field!("bar", true),
      field!("baz")
    });
    let t3 = table!("corge", fields! {
      field!("grault", true),
      field!("garply")
    });
    let mut schema = schema! {t1, t2, t3};
    add_ind!(schema, "quux", vec!["bar", "baz"], "corge", vec!["grault", "garply"]);

    schema.validate();
    schema.copy_inds(&TableName::from("quux"), &TableName::from("foo"));
    schema.validate();

    let inds = &schema.inds[&(TableName::from("foo"), TableName::from("corge"))];
    assert_eq!(inds.len(), 1);

    let ind = &inds[0];
    assert_eq!(ind.left_fields, field_vec!["bar"]);
    assert_eq!(ind.right_fields, field_vec!["grault"]);
  }

  #[test]
  fn schema_prune_inds_yes() {
    let t = table!("foo", fields! {
      field!("bar", true)
    });
    let mut schema = schema! {t};
    add_ind!(schema, "foo", vec!["bar"], "baz", vec!["quux"]);

    // !schema.validate();
    schema.prune_inds();
    schema.validate();

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

    schema.validate();
    schema.prune_inds();
    schema.validate();

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

    // !schema.validate();
    schema.prune_inds();
    schema.validate();

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

    // !schema.validate();
    schema.prune_inds();
    schema.validate();

    let ind = schema.inds.values().next().unwrap().iter().next().unwrap();

    assert_eq!(ind.left_fields.len(), 1);
    assert_eq!(ind.left_fields.iter().next().unwrap(),
               &FieldName::from("bar"));

    assert_eq!(ind.right_fields.len(), 1);
    assert_eq!(ind.right_fields.iter().next().unwrap(),
               &FieldName::from("quux"));
  }
}
