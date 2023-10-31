use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;

#[cfg(test)]
use crate::dependencies::FD;

use crate::dependencies::IND;
use crate::model::{Field, Schema, Table};
use crate::symbols::{FieldName, TableName};

pub struct Normalizer {
  pub use_stats: bool,
  pub fd_threshold: Option<f32>
}

impl Normalizer {
  /// Decompose a table according to a BCNF-violating FD, producing two new tables
  fn decomposed_tables(&self, tables: &mut HashMap<TableName, Table>, table_name: TableName)
                       -> (Table, Table) {
    let t = tables.get(&table_name).unwrap();

    // Find a violating FD
    let vfd = t.violating_fd(self.use_stats, self.fd_threshold).unwrap();

    debug!("Decomposing {} because of {}", t, vfd);

    // Construct t1 with only fields from the FD
    let t1_fields = t.fields.clone().into_iter().filter(|&(ref k, _)|
      !vfd.rhs.contains(k)
    ).map(|(k, v)|
      (k, if v.key && vfd.rhs.contains(&v.name) {
        Field {
          name: v.name,
          key: false,
          cardinality: v.cardinality,
          max_length: v.max_length
        }
      } else {
        v
      })
    ).collect::<IndexMap<FieldName, Field>>();
    let mut t1 = Table {
      name: (t.name.to_string().clone() + "_base").parse().unwrap(),
      fields: t1_fields,
      ..Default::default()
    };
    t1.add_pk_fd();
    t1.copy_fds(t);

    // Construct t2 excluding fields which are only on the RHS of the FD
    let t2_fields = t.fields.clone().into_iter().filter(|&(ref k, _)|
      vfd.lhs.contains(k) || vfd.rhs.contains(k)
    ).map(|(k, v)|
      (k, if !v.key && vfd.lhs.contains(&v.name) {
        Field {
          name: v.name,
          key: true,
          cardinality: v.cardinality,
          max_length: v.max_length
        }
      } else if v.key && !vfd.lhs.contains(&v.name) {
        Field {
          name: v.name,
          key: false,
          cardinality: v.cardinality,
          max_length: v.max_length
        }
      } else {
        v
      })
    ).collect::<IndexMap<FieldName, Field>>();
    let mut t2 = Table {
      name: (t.name.to_string().clone() + "_ext").parse().unwrap(),
      fields: t2_fields,
      ..Default::default()
    };
    t2.add_pk_fd();
    t2.copy_fds(t);

    if self.use_stats {
      t1.set_primary_key(true);
      t2.set_primary_key(true);
    }

    (t1, t2)
  }

  /// Perform BCNF normalization on tables in a schema
  pub fn normalize(&self, schema: &mut Schema) -> bool {
    let mut any_changed = false;
    let mut changed = true;

    while changed {
      changed = false;

      // Get a copy of all table names
      let mut table_names = Vec::new();
      for key in schema.tables.keys() {
        table_names.push(key.clone());
      }

      for table_name in table_names {
        // Skip tables already in BCNF
        {
          let t = &schema.tables[&table_name];
          if t.is_bcnf(self.use_stats, self.fd_threshold) {
            continue;
          }
        }

        // Decompose the tables and update the map
        changed = true;
        any_changed = true;
        let (t1, t2) = self.decomposed_tables(&mut schema.tables, table_name.clone());
        debug!("Decomposed tables are {} and {}", t1, t2);

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

        assert_ne!(t1.name, t2.name);
        let ind = IND { left_table: t1.name.clone(),
                        left_fields: ind_fields.clone(),
                        right_table: t2.name.clone(),
                        right_fields: ind_fields };
        debug!("Adding INDs {} and {}", ind, ind.reverse());
        schema.add_ind(ind.clone().reverse());
        schema.add_ind(ind);

        schema.tables.insert(t1.name.clone(), t1);
        schema.tables.insert(t2.name.clone(), t2);

        schema.copy_inds(&table_name, &t1_name);
        schema.copy_inds(&table_name, &t2_name);

        schema.tables.remove(&table_name);

        schema.prune_inds();
      }
    }

    any_changed
  }

  /// Perform subsumption of tables in a Schema based on INDs
  pub fn subsume(&self, schema: &mut Schema) -> bool {
    let mut any_changed = false;
    let mut changed = true;

    while changed {
      changed = false;

      let mut to_remove: Option<(TableName, Vec<FieldName>)> = None;
      for inds in schema.inds.values() {
        for ind in inds {
          if ind.left_table == ind.right_table {
            continue;
          }
          let right_table = &schema.tables[&ind.right_table];
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
          let left_table = &schema.tables[&ind.left_table];
          let remove_fields = ind.left_fields.iter().filter(|f| {
            fd_fields.contains(*f) && left_table.fields.contains_key(*f)
          }).cloned().collect::<Vec<_>>();

          // Check that we actually have fields to remove
          if remove_fields.is_empty() {
            continue;
          }

          debug!("Removing {:?} from table {} because of {}",
                 remove_fields, ind.left_table, ind);

          // Mark the changes and save the fields to remove
          changed = true;
          any_changed = true;
          to_remove = Some((ind.left_table.clone(), remove_fields));
          break;
        }
      }

      if let Some((table_name, remove_fields)) = to_remove {
        // Remove the fields from the table (possibly removing the table)
        let mut remove_name = None;

        {
          let table = schema.tables.get_mut(&table_name).unwrap();
          for field in remove_fields {
            table.fields.remove(&field);
          }
          table.prune_fds();

          if table.fields.is_empty() {
            remove_name = Some(table.name.clone());
          }
        }

        // Remove the table if it was found to be empty
        if remove_name.is_some() {
          schema.tables.remove(&remove_name.unwrap());
        }
      }

      // Prune any INDs which may no longer be valid
      schema.prune_inds();
    }

    // Remove tables which are subsumed by INDs
    let mut remove_tables: Vec<TableName> = Vec::new();
    for inds in schema.inds.values() {
      for ind in inds {
        if ind.left_table == ind.right_table && !remove_tables.contains(&ind.right_table) {
          continue;
        }
        // If the LHS of the IND includes all the fields of the table
        let left_table = schema.tables.get(&ind.left_table);
        if left_table.unwrap().fields.keys().all(|f| ind.left_fields.contains(f)) {
          // and the reverse IND exists, then we can remove the left table
          let reverse_ind = ind.reverse();

          if schema.contains_ind(&reverse_ind) {
            remove_tables.push(ind.left_table.clone());
          }
        }
      }
    }

    // Actually remove the tables
    if !remove_tables.is_empty() {
      for table in remove_tables {
        debug!("Subsuming table {}", table);
        schema.tables.remove(&table);
      }

      schema.prune_inds();
      any_changed = true;
    }

    // Merge tables which have a common key
    let mut remove_tables: HashSet<TableName> = HashSet::new();
    let mut new_tables: Vec<(Table, TableName, TableName)> = Vec::new();
    {
      for inds in schema.inds.values() {
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

          let left_table = &schema.tables[&ind.left_table];
          let right_table = &schema.tables[&ind.right_table];

          // Get the keys from each table in the IND and make sure they match
          let left_keys = ind.left_fields.iter().enumerate()
            .filter(|&(_, f)| left_table.fields[f].key).collect::<Vec<_>>();
          let right_keys = ind.right_fields.iter().enumerate()
            .filter(|&(_, f)| right_table.fields[f].key).collect::<Vec<_>>();
          let mut keys_match = left_keys.iter().map(|&(i, _)| i).collect::<Vec<_>>() ==
            right_keys.iter().map(|&(j, _)| j).collect::<Vec<_>>();
          keys_match = keys_match && left_table.key_fields().len() == left_keys.len();
          keys_match = keys_match && right_table.key_fields().len() == right_keys.len();

          if keys_match && schema.contains_ind(&ind.reverse()) {
            // Copy the fields and FDs from the left table into a new table
            let mut new_table = Table {
              name: format!("{}_{}", left_table.name, right_table.name).parse().unwrap(),
              ..Default::default()
            };
            for (name, field) in &left_table.fields {
              new_table.fields.insert(name.clone(), field.clone());
            }
            for fd in left_table.fds.values() {
              new_table.add_fd(fd.lhs.iter().cloned().collect::<Vec<_>>(),
                               fd.rhs.iter().cloned().collect::<Vec<_>>());
            }

            // Add fields from the right table, renaming if needed
            let mut new_right_names: HashMap<&FieldName, FieldName> = HashMap::new();

            // Add the new names for each of the keys
            for (i, &(_, field)) in right_keys.iter().enumerate() {
              new_right_names.insert(field, left_keys[i].1.clone());
            }

            for field in right_table.fields.values() {
              // Skip keys which we have already renamed
              if new_right_names.contains_key(&field.name) {
                continue;
              }

              let mut new_name = field.name.clone();
              let mut suffix = 2;
              while new_table.fields.contains_key(&new_name) {
                new_name = format!("{}{}", new_name, suffix).as_str().parse().unwrap();
                suffix += 1;
              }
              new_right_names.insert(&field.name, new_name.clone());
              new_table.fields.insert(new_name.clone(), Field {
                name: new_name,
                key: field.key,
                cardinality: field.cardinality,
                max_length: field.max_length
              });
            }
            for fd in right_table.fds.values() {
              new_table.add_fd(
                fd.lhs.iter().map(|f| new_right_names[f].clone()).collect::<Vec<_>>(),
                fd.rhs.iter().map(|f| new_right_names[f].clone()).collect::<Vec<_>>()
              );
            }
            new_table.add_pk_fd();

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
      schema.tables.insert(new_table.name.clone(), new_table);
      schema.copy_inds(&old1, &new_name);
      schema.copy_inds(&old2, &new_name);
    }

    // Remove the old tables
    for table in remove_tables {
      schema.tables.remove(&table);
    }

    schema.prune_inds();

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
    let normalizer = Normalizer { use_stats: false, fd_threshold: None};
    normalizer.normalize(&mut schema);
    schema.validate();

    let t1 = schema.tables.get(&TableName::from("foo_base")).unwrap();
    assert_has_key!(t1, field_vec!["foo"]);
    assert_has_fields!(t1, field_vec!["foo", "bar"]);

    let t2 = schema.tables.get(&TableName::from("foo_ext")).unwrap();
    assert_has_key!(t2, field_vec!["bar"]);
    assert_has_fields!(t2, field_vec!["bar", "baz"]);
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
    let normalizer = Normalizer { use_stats: false, fd_threshold: None};
    normalizer.normalize(&mut schema);
    schema.validate();

    let t1 = schema.tables.get(&TableName::from("foo_base")).unwrap();
    assert_has_key!(t1, field_vec!["foo"]);
    assert_has_fields!(t1, field_vec!["foo"]);

    let t2 = schema.tables.get(&TableName::from("foo_ext")).unwrap();
    assert_has_key!(t2, field_vec!["foo"]);
    assert_has_fields!(t2, field_vec!["foo", "bar", "baz"]);
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
    let normalizer = Normalizer { use_stats: false, fd_threshold: None};
    assert!(normalizer.subsume(&mut schema));
    schema.validate();

    let table = schema.tables.get(&TableName::from("foo")).unwrap();
    assert_has_fields!(table, field_vec!["bar"]);
    assert_missing_fields!(table, field_vec!["baz"]);
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
    let normalizer = Normalizer { use_stats: false, fd_threshold: None};
    assert!(normalizer.subsume(&mut schema));
    schema.validate();

    assert!(!schema.tables.contains_key(&TableName::from("foo")));
  }

  #[test]
  fn subsume_merge() {
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
    add_ind!(schema, "foo", vec!["bar"], "qux", vec!["quux"]);
    add_ind!(schema, "qux", vec!["quux"], "foo", vec!["bar"]);

    schema.validate();
    let normalizer = Normalizer { use_stats: false, fd_threshold: None};
    assert!(normalizer.subsume(&mut schema));
    schema.validate();

    let table = schema.tables.get(&TableName::from("foo_qux")).unwrap();
    assert_has_fields!(table, field_vec!["bar", "baz", "corge"]);
    assert_missing_fields!(table, field_vec!["quux"]);

    let fd = FD {
      lhs: field_set!["bar"],
      rhs: field_set!["corge"]
    };
    assert!(table.contains_fd(&fd));
  }
}
