use log::LogLevel::Info;
use std::collections::{HashMap, HashSet};
use std::fmt;

extern crate group_by;
extern crate permutation;

use itertools::Itertools;

#[cfg(test)]
use model::{Field, Table};
use model::Schema;
use symbols::{FieldName, TableName};

#[derive(Debug, PartialEq, Eq)]
pub struct FD {
  pub lhs: HashSet<FieldName>,
  pub rhs: HashSet<FieldName>,
}

impl fmt::Display for FD {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let lhs = self.lhs.iter().join(", ");
    let rhs = self.rhs.iter().join(", ");
    write!(f, "{} -> {}", lhs, rhs)
  }
}

impl FD {
  /// Check if this `FD` is trivial
  pub fn is_trivial(&self) -> bool {
    self.rhs.is_subset(&self.lhs)
  }

  /// Produce a new `FD` with the left and right sides switched
  pub fn reverse(&self) -> FD {
    FD { lhs: self.rhs.clone(), rhs: self.lhs.clone() }
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
      info!("FD closure loop...");

      changed = false;
      let mut new_fds = Vec::new();

      for fd1 in self.values() {
        for fd2 in self.values() {
          // Check if a new FD can be inferred via transitivity
          if fd1 == fd2 || !fd2.lhs.is_subset(&fd1.rhs) {
            continue;
          }

          let mut lhs_copy = fd1.lhs.clone().into_iter().collect::<Vec<_>>();
          lhs_copy.sort();

          let new_fd = if self.contains_key(&lhs_copy) {
            let mut new_rhs = self.get(&lhs_copy).unwrap().rhs.clone();
            new_rhs.extend(fd2.rhs.clone().into_iter());
            new_rhs.retain(|f| !lhs_copy.contains(f));

            FD { lhs: fd1.lhs.clone(),
                 rhs: new_rhs }
          } else {
            FD { lhs: fd1.lhs.clone(),
                 rhs: fd2.rhs.clone().into_iter().filter(|f| !fd1.lhs.contains(f)).collect::<HashSet<_>>() }
          };

          // Ensure the new FD actually adds fields
          if !self.contains_key(&lhs_copy) || !new_fd.rhs.is_subset(&self.get(&lhs_copy).unwrap().rhs) {
            new_fds.push(new_fd);
          }
        }
      }

      // Add any new FDs which were discovered
      if !new_fds.is_empty() {
        for new_fd in new_fds {
          let mut lhs_copy = new_fd.lhs.clone().into_iter().collect::<Vec<_>>();
          lhs_copy.sort();

          if !self.contains_key(&lhs_copy) || self[&lhs_copy] != new_fd {
            changed = true;
            debug!("Inferred {} via transitivity", new_fd);
            self.insert(lhs_copy, new_fd);
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

/// An inclusion depedency between two `Table`s
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct IND {
  /// The name of the `Table` on the left-hand side
  pub left_table: TableName,

  /// `Field`s on the left-hand side of the dependency
  pub left_fields: Vec<FieldName>,

  /// The name of the `Table` on the right-hand side
  pub right_table: TableName,

  /// `Field`s on the right-hand side of the dependency
  pub right_fields: Vec<FieldName>,
}

impl fmt::Display for IND {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let right_fields = if self.left_fields == self.right_fields {
      "...".to_string()
    } else {
      self.right_fields.iter().join(", ")
    };
    let left_fields = self.left_fields.iter().join(", ");
    write!(f, "{}({}) <= {}({})", self.left_table, left_fields, self.right_table, right_fields)
  }
}

impl IND {
  /// The reverse of this dependency
  pub fn reverse(&self) -> IND {
    let permutation = permutation::sort(&self.right_fields[..]);
    IND { left_table: self.right_table.clone(),
          left_fields: permutation.apply_slice(&self.right_fields[..]),
          right_table: self.left_table.clone(),
          right_fields: permutation.apply_slice(&self.left_fields[..]) }
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
      if log_enabled!(Info) {
        let total_inds: usize = self.inds.values().map(|inds| inds.len()).sum();
        info!("IND closure loop ({})...", total_inds);
      }

      changed = false;
      let mut new_inds = HashSet::new();
      let mut delete_inds: HashMap<_, Vec<_>> = HashMap::new();

      // Perform inference based on FDs
      for inds in self.inds.values() {
        for (i, ind1) in inds.iter().enumerate() {
          // Find all fields which can be inferred from the current FDs
          let mut all_fields = ind1.left_fields.clone().into_iter().collect::<HashSet<_>>();
          let left_table = &self.tables[&ind1.left_table];
          for fd in left_table.fds.values() {
            if fd.lhs.clone().into_iter().collect::<HashSet<_>>().is_subset(&all_fields) {
              all_fields.extend(fd.rhs.clone());
            }
          }

          for (j, ind2) in inds.iter().enumerate() {
            if i == j {
              continue;
            }

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

            // We assume that dependencies which duplicate fields are not helpful
            if new_left.len() != new_right.len() {
              continue;
            }

            // Sort the fields in the INDs
            let permutation = permutation::sort(&new_left[..]);
            let sorted_left = permutation.apply_slice(new_left);
            let sorted_right = permutation.apply_slice(new_right);

            // Construct the new IND
            assert!(ind1.left_table != ind1.right_table);
            let new_ind = IND { left_table: ind1.left_table.clone(),
                                left_fields: sorted_left,
                                right_table: ind1.right_table.clone(),
                                right_fields: sorted_right };
            let ind_key = (ind1.left_table.clone(), ind1.right_table.clone());

            // If the IND doesn't already exist add it and delete old ones
            if !&self.inds[&ind_key].contains(&new_ind) && !new_inds.contains(&new_ind) {
              debug!("Inferred {} via inference using FDs", new_ind);
              new_inds.insert(new_ind);

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

      // Infer new INDs by transitivity
      {
        // Group INDs by table and fields
        let ind_vec: Vec<_> = self.inds.values().flat_map(|inds| inds.clone()).collect();
        let grouped_inds = group_by::group_by(ind_vec.iter(), |ind| {
          (ind.left_table.clone(), ind.left_fields.clone())
        });

        for ind1 in &ind_vec {
          // Check for a matching the RHS (implies a new IND via transitivity)
          let ind_key = &(ind1.right_table.clone(), ind1.right_fields.clone());
          if let Some(other_inds) = grouped_inds.get(&ind_key) {
            for ind2 in other_inds.iter() {
              if ind1.left_table == ind2.right_table {
                continue;
              }

              // Add a new IND for each transitive relation
              let new_ind = IND { left_table: ind1.left_table.clone(),
                                  left_fields: ind1.left_fields.clone(),
                                  right_table: ind2.right_table.clone(),
                                  right_fields: ind2.right_fields.clone() };

              let table_key = (new_ind.left_table.clone(), new_ind.right_table.clone());
              if !self.inds.get(&table_key).contains(&new_ind) && !new_inds.contains(&new_ind) {
                debug!("Inferred {} via transitivity", new_ind);
                new_inds.insert(new_ind);
              }
            }
          }
        }
      }

      if !new_inds.is_empty() || !delete_inds.is_empty() {
        if !delete_inds.is_empty() {
          changed = true;
        }

        // Delete old INDs
        for (tables, delete_indices) in &mut delete_inds {
          let mut inds = self.inds.get_mut(tables.clone());
          delete_indices.sort_by(|a, b| a.cmp(b).reverse());
          delete_indices.dedup();
          for delete_index in delete_indices {
            inds.remove(*delete_index);
          }
        }

        // Add new INDs
        for new_ind in new_inds {
          changed = changed || self.add_ind(new_ind);
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
  fn fd_trivial() {
    let fd = FD {
      lhs: field_set!["foo", "bar"],
      rhs: field_set!["bar"]
    };
    assert!(fd.is_trivial());
  }

  #[test]
  fn fd_reverse() {
    let fd = FD {
      lhs: field_set!["foo"],
      rhs: field_set!["bar"]
    };
    let reverse = fd.reverse();
    assert_eq!(reverse.lhs, fd.rhs);
    assert_eq!(reverse.rhs, fd.lhs);
  }

  #[test]
  fn fd_closure() {
    let mut fds: HashMap<Vec<FieldName>, FD> = collect![
      field_vec!["foo"] => FD {
        lhs: field_set!["foo"],
        rhs: field_set!["bar"]
      },
      field_vec!["bar"] => FD {
        lhs: field_set!["bar"],
        rhs: field_set!["baz"]
      }
    ];
    assert!(fds.closure());

    assert!(fds.values().any(|fd| *fd == FD {
      lhs: field_set!["foo"],
      rhs: field_set!["bar", "baz"]
    }));
    assert!(!fds.closure());
  }

  #[test]
  fn ind_reverse() {
    let ind = IND {
      left_table: TableName::from("foo"), left_fields: field_vec!["bar"],
      right_table: TableName::from("baz"), right_fields: field_vec!["quux"]
    };
    let rev = IND {
      left_table: TableName::from("baz"), left_fields: field_vec!["quux"],
      right_table: TableName::from("foo"), right_fields: field_vec!["bar"]
    };

    assert_eq!(ind.reverse(), rev)
  }

  #[test]
  fn ind_closure_transitive() {
    let t1 = table!("foo", fields! {
      field!("bar", true)
    });
    let t2 = table!("baz", fields! {
      field!("quux", true)
    });
    let t3 = table!("qux", fields! {
      field!("quuz", true)
    });
    let mut schema = schema! {t1, t2, t3};
    add_ind!(schema, "qux", vec!["quuz"], "baz", vec!["quux"]);
    add_ind!(schema, "baz", vec!["quux"], "foo", vec!["bar"]);

    schema.validate();
    schema.ind_closure();
    schema.validate();

    assert!(schema.inds.get(&(TableName::from("qux"), TableName::from("foo"))).len() > 0);
  }

  #[test]
  fn ind_closure_transitive_reverse() {
    let t1 = table!("foo", fields! {
      field!("bar", true)
    });
    let t2 = table!("baz", fields! {
      field!("quux", true)
    });
    let t3 = table!("qux", fields! {
      field!("quuz", true)
    });
    let mut schema = schema! {t1, t2, t3};
    add_ind!(schema, "qux", vec!["quuz"], "baz", vec!["quux"]);
    add_ind!(schema, "foo", vec!["bar"], "baz", vec!["quux"]);

    schema.validate();
    schema.ind_closure();
    schema.validate();

    assert!(schema.inds.get(&(TableName::from("qux"), TableName::from("foo"))).len() == 0);
  }

  #[test]
  fn ind_closure_fd() {
    let mut t1 = table!("foo", fields! {
      field!("bar", true),
      field!("baz")
    });
    add_fd!(t1, vec!["bar"], vec!["baz"]);
    let t2 = table!("quux", fields! {
      field!("qux", true),
      field!("corge")
    });
    let mut schema = schema! {t1, t2};
    add_ind!(schema, "foo", vec!["bar"], "quux", vec!["qux"]);
    add_ind!(schema, "foo", vec!["baz"], "quux", vec!["corge"]);

    schema.validate();
    schema.ind_closure();
    schema.validate();

    let inds = schema.inds.get(&(TableName::from("foo"), TableName::from("quux")));
    assert!(inds.len() == 1);
    assert!(inds[0].left_fields.len() == 2);
  }
}
