use std::collections::{HashMap, HashSet};

use model::Table;

pub trait Normalizable {
  fn normalize(&mut self) -> ();
}

impl<'a> Normalizable for HashMap<String, Table<'a>> {
  fn normalize(&mut self) -> () {
    for table_name in self.keys() {
      // Skip tables already in BCNF
      let t1 = self.get(table_name).unwrap();
      if t1.is_bcnf() {
        continue;
      }

      // Loop over violating FDs
      for fd in t1.fds.values() {
        if fd.is_trivial() || t1.is_superkey(&fd.lhs) {
          continue;
        }

        // TODO Actually perform normalization
      }
    }
  }
}
