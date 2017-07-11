use model::{Table};

use std::collections::{HashMap, HashSet};

extern crate group_by;

pub trait Closure {
  fn closure(&mut self, tables: Option<&mut HashMap<String, Table>>) -> bool;
}

#[derive(Debug, PartialEq, Eq)]
pub struct FD<'a> {
  pub lhs: HashSet<&'a str>,
  pub rhs: HashSet<&'a str>,
}

impl<'a> FD<'a> {
  pub fn is_trivial(&self) -> bool {
    self.rhs.is_subset(&self.lhs)
  }
}

impl<'a> Closure for HashMap<Vec<&'a str>, FD<'a>> {
  fn closure(&mut self, _: Option<&mut HashMap<String, Table>>) -> bool {
    let mut any_changed = false;
    let mut changed = true;

    while changed {
      changed = false;
      let mut new_fds = Vec::new();

      for fd1 in self.values() {
        for fd2 in self.values() {
          // Check if a new FD can be inferred via transitivity
          if fd1 == fd2 || !fd1.rhs.is_subset(&fd2.lhs) { continue; }

          let mut lhs_copy = fd1.lhs.clone().into_iter().collect::<Vec<_>>();
          lhs_copy.sort();

          let new_fd;
          if self.contains_key(&lhs_copy) {
            let mut new_rhs = self.get(&lhs_copy).unwrap().rhs.clone();
            new_rhs.extend(fd2.rhs.clone().into_iter());
            new_fd = FD { lhs: fd1.lhs.clone(), rhs: new_rhs };
          } else {
            new_fd = FD { lhs: fd1.lhs.clone(), rhs: fd2.rhs.clone() };
          }

          new_fds.push(new_fd);
        }
      }

      // Add any new FDs which were discovered
      if new_fds.len() > 0 {
        for new_fd in new_fds.into_iter() {
          let mut lhs_copy = new_fd.lhs.clone().into_iter().collect::<Vec<_>>();
          lhs_copy.sort();

          changed = true;
          self.insert(lhs_copy, new_fd);
        }
      }

      if changed {
        any_changed = true;
      }
    }

    any_changed
  }
}

#[derive(PartialEq)]
pub struct IND<'a> {
  pub left_table: &'a str,
  pub left_fields: Vec<&'a str>,
  pub right_table: &'a str,
  pub right_fields: Vec<&'a str>,
}

impl<'a> IND<'a> {
  fn reverse(&self) -> IND {
    IND { left_table: self.right_table.clone(),
          left_fields: self.right_fields.clone(),
          right_table: self.left_table.clone(),
          right_fields: self.left_fields.clone() }
  }
}

impl<'a> Closure for HashMap<(&'a str, &'a str), Vec<IND<'a>>> {
  fn closure(&mut self, tables: Option<&mut HashMap<String, Table>>) -> bool {
    let table_map = tables.unwrap();
    let mut any_changed = false;
    let mut changed = true;

    while changed {
      changed = false;
      let mut new_inds = Vec::new();
      let mut delete_inds: HashMap<_, Vec<usize>> = HashMap::new();

      // Perform inference based on FDs
      for inds in self.values() {
        for (i, ind1) in inds.iter().enumerate() {
          // Find all fields which can be inferred from the current FDs
          let mut all_fields = ind1.left_fields.clone().into_iter().collect::<HashSet<_>>();
          let left_table = table_map.get(ind1.left_table).unwrap();
          for fd in left_table.fds.values() {
            if fd.lhs.clone().into_iter().collect::<HashSet<_>>().is_subset(&all_fields) {
              all_fields.extend(fd.rhs.clone());
            }
          }

          for (j, ind2) in inds.iter().enumerate() {
            if i == j { continue; }

            let mut new_left = ind1.left_fields.clone();
            let mut added_fields = ind2.left_fields.clone();
            added_fields.retain(|f| !new_left.contains(&f));
            new_left.extend(added_fields);

            if new_left.iter().collect::<HashSet<_>>().is_subset(&all_fields.iter().collect::<HashSet<_>>()) {
              continue;
            }

            let mut new_right = ind1.right_fields.clone();
            added_fields = ind2.right_fields.clone();
            added_fields.retain(|f| !new_right.contains(&f));
            new_right.extend(added_fields);

            // Construct the new IND
            let new_ind = IND { left_table: ind1.left_table.clone(),
                                left_fields: new_left,
                                right_table: ind1.right_table.clone(),
                                right_fields: new_right };
            let ind_key = (ind1.left_table, ind1.right_table);

            // If the IND doesn't already exist add it and delete old ones
            if !self.get(&ind_key).unwrap().contains(&new_ind) {
              new_inds.push(new_ind);

              if delete_inds.contains_key(&ind_key) {
                let inds = delete_inds.get_mut(&ind_key).unwrap();
                inds.push(i);
                inds.push(j);
              } else {
                delete_inds.insert(ind_key, vec![i, j]);
              }
            }
          }
        }
      }

      // Infer new FDs by transitivity
      {
        // Group INDs by table and fields
        let ind_vec: Vec<&IND> = self.values().flat_map(|inds| inds.clone()).collect();
        let grouped_inds = group_by::group_by(ind_vec.iter(),
          |ind| (ind.left_table.clone(), ind.left_fields.clone()));

        for ind1 in ind_vec.iter() {
          // Check for a matching the RHS (implies a new IND via transitivity)
          let ind_key = &(ind1.right_table.clone(), ind1.right_fields.clone());
          if let Some(other_inds) = grouped_inds.get(&ind_key) {
            for ind2 in other_inds.iter() {
              // Add a new IND for each transitive relation
              let new_ind = IND { left_table: ind1.left_table.clone(),
                                  left_fields: ind1.left_fields.clone(),
                                  right_table: ind2.right_table.clone(),
                                  right_fields: ind2.right_fields.clone() };

              let table_key = (new_ind.left_table, new_ind.right_table);
              if !self.get(&table_key).unwrap_or(&vec![]).contains(&new_ind) {
                new_inds.push(new_ind);
              }
            }
          }
        }
      }

      if new_inds.len() > 0 || delete_inds.len() > 0 {
        changed = true;

        // Add new INDs
        for new_ind in new_inds.into_iter() {
          let ind_key = (new_ind.left_table, new_ind.right_table);
          if self.contains_key(&ind_key) {
            self.get_mut(&ind_key).unwrap().push(new_ind);
          } else {
            self.insert(ind_key, vec![new_ind]);
          }
        }

        // Delete old INDs
        for (tables, delete_indices) in delete_inds.iter_mut() {
          let mut inds = self.get_mut(&tables).unwrap();
          delete_indices.sort_by(|a, b| a.cmp(b).reverse());
          for delete_index in delete_indices.iter() {
            inds.remove(*delete_index);
          }
        }
      }

      if changed {
        any_changed = true;
      }
    }

    any_changed
  }
}
