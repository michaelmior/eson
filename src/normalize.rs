use std::collections::HashMap;

use model::{Field, Table};

pub trait Normalizable {
  fn normalize(&mut self) -> bool;
}

fn decomposed_tables<'a, 'b>(tables: &mut HashMap<String, Table<'a>>, table_name: String) -> (Table<'b>, Table<'b>) {
  let t = tables.get(&table_name).unwrap();

  // Find a violating FD
  let vfd = t.violating_fd();

  // Construct t1 with only fields from the FD
  let t1_fields = t.fields.clone().into_iter().filter(|&(ref k, _)|
    vfd.lhs.contains(&k.as_str()) || vfd.rhs.contains(&k.as_str())
  ).collect::<HashMap<String, Field>>();
  let t1 = Table { name: t.name.clone() + "_base", fields: t1_fields, ..Default::default() };

  // Construct t2 excluding fields which are only on the RHS of the FD
  let t2_fields = t.fields.clone().into_iter().filter(|&(ref k, _)|
    !vfd.rhs.contains(&k.as_str()) || vfd.lhs.contains(&k.as_str())
  ).collect::<HashMap<String, Field>>();
  let t2 = Table { name: t.name.clone() + "_ext", fields: t2_fields, ..Default::default() };

  (t1, t2)
}

impl<'a> Normalizable for HashMap<String, Table<'a>> {
  fn normalize(&mut self) -> bool {
    let mut any_changed = false;
    let mut changed = true;

    while changed {
      changed = false;

      // Get a copy of all table names
      let mut table_names = Vec::new();
      for key in self.keys() {
        table_names.push(key.clone());
      }

      for table_name in table_names {
        // Skip tables already in BCNF
        {
          let t = self.get(&table_name).unwrap();
          if t.is_bcnf() {
            continue;
          }
        }

        // Decompose the tables and update the map
        any_changed = true;
        let (t1, t2) = decomposed_tables(self, table_name.clone());
        debug!("Decomposing {} into {} and {}", table_name, t1, t2);

        self.remove(&table_name);
        self.insert(t1.name.clone(), t1);
        self.insert(t2.name.clone(), t2);
      }
    }

    any_changed
  }
}
