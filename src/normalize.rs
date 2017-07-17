use std::collections::HashMap;

use dependencies::IND;
use model::{Field, Schema, Table};
use symbols::{FieldName, TableName};

pub trait Normalizable {
  fn normalize(&mut self) -> bool;
}

fn decomposed_tables(tables: &mut HashMap<TableName, Table>, table_name: TableName) -> (Table, Table) {
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
          let t = self.tables.get(&table_name).unwrap();
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
        let ind = IND {
          left_table: t1.name.clone(),
          left_fields: ind_fields.clone(),
          right_table: t2.name.clone(),
          right_fields: ind_fields
        };
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

    schema.normalize();

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

    schema.normalize();

    let t1 = schema.tables.get(&TableName::from("foo_base")).unwrap();
    assert_has_key!(t1, field_names!["foo"]);
    assert_has_fields!(t1, field_names!["foo"]);

    let t2 = schema.tables.get(&TableName::from("foo_ext")).unwrap();
    assert_has_key!(t2, field_names!["foo"]);
    assert_has_fields!(t2, field_names!["foo", "bar", "baz"]);
  }
}
