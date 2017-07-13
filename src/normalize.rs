use std::collections::HashMap;

use model::{Field, Schema, Table};
use symbols::{FieldName, TableName};

pub trait Normalizable {
  fn normalize(&mut self) -> bool;
}

fn decomposed_tables(tables: &mut HashMap<TableName, Table>, table_name: TableName) -> (Table, Table) {
  let t = tables.get(&table_name).unwrap();

  // Find a violating FD
  let vfd = t.violating_fd().unwrap();

  // Construct t1 with only fields from the FD
  let t1_fields = t.fields.clone().into_iter().filter(|&(ref k, _)|
    vfd.lhs.contains(k) || vfd.rhs.contains(k)
  ).collect::<HashMap<FieldName, Field>>();
  let mut t1 = Table { name: (t.name.to_string().clone() + "_base").parse().unwrap(), fields: t1_fields, ..Default::default() };
  t1.copy_fds(t);

  // Construct t2 excluding fields which are only on the RHS of the FD
  let t2_fields = t.fields.clone().into_iter().filter(|&(ref k, _)|
    !vfd.rhs.contains(k) || vfd.lhs.contains(k)
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
        any_changed = true;
        let (t1, t2) = decomposed_tables(&mut self.tables, table_name.clone());
        debug!("Decomposing {} into {} and {}", table_name, t1, t2);

        self.copy_inds(&table_name, &t1.name);
        self.copy_inds(&table_name, &t2.name);

        self.tables.remove(&table_name);
        self.tables.insert(t1.name.clone(), t1);
        self.tables.insert(t2.name.clone(), t2);

        self.prune_inds();
      }
    }

    any_changed
  }
}
