use std::fmt;
use std::collections::{HashMap, HashSet};

extern crate group_by;

use model::Schema;
use symbols::{TableName, FieldName};

#[derive(Debug, PartialEq, Eq)]
pub struct FD {
  pub lhs: HashSet<FieldName>,
  pub rhs: HashSet<FieldName>,
}

impl fmt::Display for FD {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?} -> {:?}", self.lhs, self.rhs)
  }
}

impl FD {
  pub fn is_trivial(&self) -> bool {
    self.rhs.is_subset(&self.lhs)
  }
}

pub trait FDClosure {
  fn closure(&mut self) -> bool;
}

impl FDClosure for HashMap<Vec<FieldName>, FD> {
  fn closure(&mut self) -> bool {
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

          debug!("Inferred {} via transitivity", new_fd);
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

#[derive(Debug, PartialEq)]
pub struct IND {
  pub left_table: TableName,
  pub left_fields: Vec<FieldName>,
  pub right_table: TableName,
  pub right_fields: Vec<FieldName>,
}

impl fmt::Display for IND {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let right_fields = if self.left_fields == self.right_fields {
      "...".to_string()
    } else {
      format!("{:?}", self.right_fields)
    };
    write!(f, "{}({:?}) <= {}({})", self.left_table, self.left_fields, self.right_table, right_fields)
  }
}

impl IND {
  #[cfg(test)]
  fn reverse(&self) -> IND {
    IND { left_table: self.right_table.clone(),
          left_fields: self.right_fields.clone(),
          right_table: self.left_table.clone(),
          right_fields: self.left_fields.clone() }
  }
}

pub trait INDClosure {
  fn ind_closure(&mut self) -> bool;
}

impl INDClosure for Schema {
  fn ind_closure(&mut self) -> bool {
    let mut any_changed = false;
    let mut changed = true;

    while changed {
      changed = false;
      let mut new_inds = Vec::new();
      let mut delete_inds: HashMap<_, Vec<_>> = HashMap::new();

      // Perform inference based on FDs
      for inds in self.inds.values() {
        for (i, ind1) in inds.iter().enumerate() {
          // Find all fields which can be inferred from the current FDs
          let mut all_fields = ind1.left_fields.clone().into_iter().collect::<HashSet<_>>();
          let left_table = self.tables.get(&ind1.left_table).unwrap();
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
            let ind_key = (ind1.left_table.clone(), ind1.right_table.clone());
            debug!("Inferred {} via inference using FDs", new_ind);

            // If the IND doesn't already exist add it and delete old ones
            if !self.inds.get(&ind_key).unwrap().contains(&new_ind) {
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
        let ind_vec: Vec<&IND> = self.inds.values().flat_map(|inds| inds.clone()).collect();
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
              debug!("Inferred {} via transitivity", new_ind);

              let table_key = (new_ind.left_table.clone(), new_ind.right_table.clone());
              if !self.inds.get(&table_key).unwrap_or(&vec![]).contains(&new_ind) {
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
          self.add_ind(new_ind);
        }

        // Delete old INDs
        for (tables, delete_indices) in delete_inds.iter_mut() {
          let mut inds = self.inds.get_mut(&tables).unwrap();
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn ind_reverse() {
    let ind = IND {
      left_table: "foo".parse().unwrap(), left_fields: vec!["bar".parse().unwrap()],
      right_table: "baz".parse().unwrap(), right_fields: vec!["quux".parse().unwrap()]
    };
    let rev = IND {
      left_table: "baz".parse().unwrap(), left_fields: vec!["quux".parse().unwrap()],
      right_table: "foo".parse().unwrap(), right_fields: vec!["bar".parse().unwrap()]
    };

    assert_eq!(ind.reverse(), rev)
  }
}
