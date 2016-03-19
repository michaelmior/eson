use model::{Table};

use std::collections::{HashMap, HashSet};

pub trait Closure {
  fn closure(&mut self) -> ();
}

#[derive(PartialEq, Eq)]
pub struct FD {
  pub lhs: HashSet<String>,
  pub rhs: HashSet<String>,
}

impl Closure for HashMap<Vec<String>, FD> {
  fn closure(&mut self) -> () {
    let mut changed = true;

    while changed {
      changed = false;
      let mut new_fds = Vec::new();

      for fd1 in self.values() {
        for fd2 in self.values() {
          // Check if a new FD can be inferred via transitivity
          if fd1 == fd2 || !fd1.rhs.is_subset(&fd2.lhs) { continue; }

          let mut lhs_copy = fd1.lhs.clone().into_iter().collect::<Vec<String>>();
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
          let mut lhs_copy = new_fd.lhs.clone().into_iter().collect::<Vec<String>>();
          lhs_copy.sort();

          changed = true;
          self.insert(lhs_copy, new_fd);
        }
      }
    }
  }
}

pub struct IND<'a> {
  pub left_table: &'a Table,
  pub left_fields: Vec<String>,
  pub right_table: &'a Table,
  pub right_fields: Vec<String>,
}

impl<'a> IND<'a> {
  fn reverse(&self) -> IND {
    IND { left_table: self.right_table, left_fields: self.right_fields.clone(),
          right_table: self.left_table, right_fields: self.left_fields.clone() }
  }
}
