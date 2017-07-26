use std::collections::{HashMap, HashSet};

use dependencies::IND;
use model::{Field, Schema, Table};
use symbols::{FieldName, TableName};

pub trait Normalizable {
  fn normalize(&mut self) -> bool;
  fn subsume(&mut self) -> bool;
}

fn decomposed_tables(tables: &mut HashMap<TableName, Table>, table_name: TableName)
                     -> (Table, Table) {
  let t = tables.get(&table_name).unwrap();

  // Find a violating FD
  let vfd = t.violating_fd().unwrap();

  info!("Decomposing {} because of {}", t, vfd);

  // Construct t1 with only fields from the FD
  let t1_fields = t.fields.clone().into_iter().filter(|&(ref k, _)|
    !vfd.rhs.contains(k)
  ).map(|(k, v)|
    (k, if v.key && vfd.rhs.contains(&v.name) {
      Field { name: v.name, key: false }
    } else {
      v
    })
  ).collect::<HashMap<FieldName, Field>>();
  let mut t1 = Table { name: (t.name.to_string().clone() + "_base").parse().unwrap(), fields: t1_fields, ..Default::default() };
  t1.copy_fds(t);

  // Construct t2 excluding fields which are only on the RHS of the FD
  let t2_fields = t.fields.clone().into_iter().filter(|&(ref k, _)|
    vfd.lhs.contains(k) || vfd.rhs.contains(k)
  ).map(|(k, v)|
    (k, if !v.key && vfd.lhs.contains(&v.name) {
      Field { name: v.name, key: true }
    } else if v.key && !vfd.lhs.contains(&v.name) {
      Field { name: v.name, key: false }
    } else {
      v
    })
  ).collect::<HashMap<FieldName, Field>>();
  let mut t2 = Table { name: (t.name.to_string().clone() + "_ext").parse().unwrap(), fields: t2_fields, ..Default::default() };
  t2.copy_fds(t);

  (t1, t2)
}

impl Normalizable for Schema {
  fn normalize(&mut self) -> bool {
    let mut any_changed = false;
    let mut changed = true;

    while changed {
      changed = false;

      // Get a copy of all table names
      let mut table_names = Vec::new();
      for key in self.tables.keys() {
        table_names.push(key.clone());
      }

      for table_name in table_names {
        // Skip tables already in BCNF
        {
          let t = &self.tables[&table_name];
          if t.is_bcnf() {
            continue;
          }
        }

        // Decompose the tables and update the map
        changed = true;
        any_changed = true;
        let (t1, t2) = decomposed_tables(&mut self.tables, table_name.clone());
        info!("Decomposed tables are {} and {}", t1, t2);

        let t1_name = t1.name.clone();
        let t2_name = t2.name.clone();

        let mut ind_fields = Vec::new();
        for key in t1.key_fields() {
          ind_fields.push(key);
        }
        for key in t2.key_fields() {
          if !ind_fields.contains(&key) {
            ind_fields.push(key);
          }
        }
        ind_fields.sort();

        assert!(t1.name != t2.name);
        let ind = IND { left_table: t1.name.clone(),
                        left_fields: ind_fields.clone(),
                        right_table: t2.name.clone(),
                        right_fields: ind_fields };
        info!("Adding INDs {} and {}", ind, ind.reverse());
        self.add_ind(ind.clone().reverse());
        self.add_ind(ind);

        self.tables.insert(t1.name.clone(), t1);
        self.tables.insert(t2.name.clone(), t2);

        self.copy_inds(&table_name, &t1_name);
        self.copy_inds(&table_name, &t2_name);

        self.tables.remove(&table_name);

        self.prune_inds();
      }
    }

    any_changed
  }

  fn subsume(&mut self) -> bool {
    let mut any_changed = false;
    let mut changed = true;

    while changed {
      changed = false;

      let mut remove_table: Option<TableName> = None;
      let mut remove_fields: Vec<FieldName> = Vec::new();
      for inds in self.inds.values() {
        for ind in inds {
          if ind.left_table == ind.right_table {
            continue;
          }
          let right_table = &self.tables[&ind.right_table];
          let right_key = right_table.key_fields();
          if !right_key.iter().all(|v| ind.right_fields.contains(v)) {
            continue;
          }

          // Get all fields implied by the FDs relevant to this IND
          // (the LHS of the IND contains all the fields)
          let fds = right_table.fds.values().filter(|fd|
            fd.lhs.iter().all(|f| ind.right_fields.contains(f))
          ).collect::<Vec<_>>();
          let fd_fields = fds.iter().flat_map(|fd| fd.rhs.clone()).fold(HashSet::new(), |mut fields: HashSet<FieldName>, field|
            match ind.right_fields.iter().position(|f| f == &field) {
              Some(index) => {
                fields.insert(ind.left_fields[index].clone());
                fields
              },
              None => fields
            }
          );

          // We can remove all fields implied by the FDs
          let left_table = &self.tables[&ind.left_table];
          remove_fields.extend(ind.left_fields.iter().map(|f| f.clone()).filter(|f| {
            fd_fields.contains(f) && left_table.fields.contains_key(f)
          }));

          // Check that we actually have fields to remove
          if remove_fields.is_empty() {
            continue;
          }

          // Mark the changes and save the fields to remove
          changed = true;
          any_changed = true;
          remove_table = Some(ind.left_table.clone());
          break;
        }
      }

      if let Some(table_name) = remove_table {
        // Remove the fields from the table (possibly removing the table)
        let mut table = self.tables.get_mut(&table_name).unwrap();
        info!("Removing {:?} from table {}", remove_fields, table);
        for field in remove_fields {
          table.fields.remove(&field);
        }
        table.prune_fds();
      }

      // Prune any INDs which may no longer be valid
      self.prune_inds();
    }

    // Remove tables which are subsumed by INDs
    let mut remove_tables: Vec<TableName> = Vec::new();
    for inds in self.inds.values() {
      for ind in inds {
        if ind.left_table == ind.right_table && !remove_tables.contains(&ind.right_table) {
          continue;
        }
        // If the LHS of the IND includes all the fields of the table
        let left_table = self.tables.get(&ind.left_table);
        if left_table.unwrap().fields.keys().all(|f| ind.left_fields.contains(f)) {
          // and the reverse IND exists, then we can remove the left table
          let reverse_ind = ind.reverse();

          if self.contains_ind(&reverse_ind) {
            remove_tables.push(ind.left_table.clone());
          }
        }
      }
    }

    // Actually remove the tables
    if !remove_tables.is_empty() {
      for table in remove_tables {
        info!("Subsuming table {}", table);
        self.tables.remove(&table);
      }

      self.prune_inds();
      any_changed = true;
    }

    // Merge tables which have a common key
    let mut remove_tables: HashSet<TableName> = HashSet::new();
    let mut new_tables: Vec<(Table, TableName, TableName)> = Vec::new();
    {
      for inds in self.inds.values() {
        for ind in inds {
          // Skip over tables we are going to remove
          // and any tables which are equal
          // (we use an inequality for deterministic results and it
          //  doesn't matter since we need the reverse IND anyway)
          if remove_tables.contains(&ind.left_table) ||
             remove_tables.contains(&ind.right_table) ||
             ind.left_table >= ind.right_table {
            continue;
          }

          let left_table = &self.tables[&ind.left_table];
          let right_table = &self.tables[&ind.right_table];
          let has_all_left = left_table.key_fields().iter().all(|f| ind.left_fields.contains(f));
          let has_all_right = right_table.key_fields().iter().all(|f| ind.right_fields.contains(f));
          if has_all_left && has_all_right && self.contains_ind(&ind.reverse()) {
            // Copy the fields and FDs from the left table into a new table
            let mut new_table = Table {
              name: format!("{}_{}", left_table.name, right_table.name).parse().unwrap(),
              ..Default::default()
            };
            for (name, field) in &left_table.fields {
              new_table.fields.insert(name.clone(), field.clone());
            }
            for fd in left_table.fds.values() {
              new_table.add_fd(fd.lhs.iter().map(|f| f.clone()).collect::<Vec<_>>(),
                               fd.rhs.iter().map(|f| f.clone()).collect::<Vec<_>>());
            }

            // Add fields from the right table, renaming if needed
            let mut new_right_names: HashMap<&FieldName, FieldName> = HashMap::new();
            let right_keys = right_table.key_fields();
            for field in right_table.fields.values() {
              // Don't add keys since we already have them from the left table
              if right_keys.contains(&field.name) {
                continue;
              }

              let mut new_name = field.name.clone();
              let mut suffix = 2;
              while new_table.fields.contains_key(&new_name) {
                new_name = format!("{}{}", new_name, suffix).as_str().parse().unwrap();
                suffix += 1;
              }
              new_right_names.insert(&field.name, new_name.clone());
              new_table.fields.insert(new_name.clone(), Field {name: new_name, key: field.key});
            }
            for fd in left_table.fds.values() {
              new_table.add_fd(
                fd.lhs.iter().map(|f| new_right_names[f].clone()).collect::<Vec<_>>(),
                fd.rhs.iter().map(|f| new_right_names[f].clone()).collect::<Vec<_>>()
              );
            }

            any_changed = true;
            new_tables.push((new_table, ind.left_table.clone(), ind.right_table.clone()));
            remove_tables.insert(ind.left_table.clone());
            remove_tables.insert(ind.right_table.clone());
          }
        }
      }
    }

    // Add the new table and copy over INDs
    for (new_table, old1, old2) in new_tables {
      let new_name = new_table.name.clone();
      self.tables.insert(new_table.name.clone(), new_table);
      self.copy_inds(&old1, &new_name);
      self.copy_inds(&old2, &new_name);
    }

    // Remove the old tables
    for table in remove_tables {
      self.tables.remove(&table);
    }

    self.prune_inds();

    any_changed
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn normalize() {
    let mut t = table!("foo", fields! {
      field!("foo", true),
      field!("bar"),
      field!("baz")
    });
    add_fd!(t, vec!["foo"], vec!["bar"]);
    add_fd!(t, vec!["bar"], vec!["baz"]);
    let mut schema = schema! {t};

    schema.validate();
    schema.normalize();
    schema.validate();

    let t1 = schema.tables.get(&TableName::from("foo_base")).unwrap();
    assert_has_key!(t1, field_names!["foo"]);
    assert_has_fields!(t1, field_names!["foo", "bar"]);

    let t2 = schema.tables.get(&TableName::from("foo_ext")).unwrap();
    assert_has_key!(t2, field_names!["bar"]);
    assert_has_fields!(t2, field_names!["bar", "baz"]);
  }

  #[test]
  fn normalize_change_keys() {
    let mut t = table!("foo", fields! {
      field!("foo", true),
      field!("bar", true),
      field!("baz", true)
    });
    add_fd!(t, vec!["foo"], vec!["bar", "baz"]);
    let mut schema = schema! {t};

    schema.validate();
    schema.normalize();
    schema.validate();

    let t1 = schema.tables.get(&TableName::from("foo_base")).unwrap();
    assert_has_key!(t1, field_names!["foo"]);
    assert_has_fields!(t1, field_names!["foo"]);

    let t2 = schema.tables.get(&TableName::from("foo_ext")).unwrap();
    assert_has_key!(t2, field_names!["foo"]);
    assert_has_fields!(t2, field_names!["foo", "bar", "baz"]);
  }

  #[test]
  fn subsume_fields() {
    let t1 = table!("foo", fields! {
      field!("bar", true),
      field!("baz")
    });

    let mut t2 = table!("qux", fields! {
      field!("quux", true),
      field!("corge")
    });
    add_fd!(t2, vec!["quux"], vec!["corge"]);

    let mut schema = schema! {t1, t2};
    add_ind!(schema, "foo", vec!["bar", "baz"], "qux", vec!["quux", "corge"]);

    schema.validate();
    assert!(schema.subsume());
    schema.validate();

    let table = schema.tables.get(&TableName::from("foo")).unwrap();
    assert_has_fields!(table, field_names!["bar"]);
    assert_missing_fields!(table, field_names!["baz"]);
  }

  #[test]
  fn subsume_table() {
    let t1 = table!("foo", fields! {
      field!("bar", true),
      field!("baz")
    });

    let t2 = table!("qux", fields! {
      field!("quux", true),
      field!("corge"),
      field!("grault")
    });

    let mut schema = schema! {t1, t2};
    add_ind!(schema, "foo", vec!["bar", "baz"], "qux", vec!["quux", "corge"]);
    add_ind!(schema, "qux", vec!["quux", "corge"], "foo", vec!["bar", "baz"]);

    schema.validate();
    assert!(schema.subsume());
    schema.validate();

    assert!(!schema.tables.contains_key(&TableName::from("foo")));
  }

  #[test]
  fn subsume_merge() {
    let t1 = table!("foo", fields! {
      field!("bar", true),
      field!("baz")
    });

    let t2 = table!("qux", fields! {
      field!("quux", true),
      field!("corge")
    });

    let mut schema = schema! {t1, t2};
    add_ind!(schema, "foo", vec!["bar"], "qux", vec!["quux"]);
    add_ind!(schema, "qux", vec!["quux"], "foo", vec!["bar"]);

    schema.validate();
    assert!(schema.subsume());
    schema.validate();

    let table = schema.tables.get(&TableName::from("foo_qux")).unwrap();
    assert_has_fields!(table, field_names!["bar", "baz", "corge"]);
    assert_missing_fields!(table, field_names!["quux"]);
  }
}
