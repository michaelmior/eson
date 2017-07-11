#![feature(plugin)]
#![feature(slice_patterns)]

#![plugin(peg_syntax_ext)]

use std::collections::{HashMap, HashSet};
use std::env;
use std::io;
use std::io::prelude::*;
use std::fs::File;

#[macro_use] mod macros;
peg_file! input("input.rustpeg");
mod dependencies;
mod model;
mod normalize;

use dependencies::Closure;
use normalize::Normalizable;

fn read_file(name: &str) -> Result<String, io::Error> {
  let mut input_file = try!(File::open(name));
  let mut input_string = String::new();
  try!(input_file.read_to_string(&mut input_string));

  Ok(input_string)
}

// Copy FDs between tables based on inclusion dependencies
fn copy_fds(inds: &mut HashMap<(&str, &str), Vec<dependencies::IND>>, tables: &mut HashMap<String, model::Table>) -> () {
  let mut new_fds = Vec::new();

  // Loop over all FDs
  for ind_vec in inds.values() {
    for ind in ind_vec.iter() {
      let left_fields = tables.get(ind.left_table).unwrap().fields.keys().map(|k| k.as_str()).into_iter().collect::<HashSet<_>>();
      let left_key = tables.get(ind.left_table).unwrap().fields.values().filter(|f| f.key).map(|f| f.name.as_str()).into_iter().collect::<HashSet<_>>();

      new_fds.extend(tables.get(ind.right_table).unwrap().fds.values().map(|fd| {
        let fd_lhs = fd.lhs.clone().into_iter().collect::<HashSet<_>>();
        let fd_rhs = fd.rhs.clone().into_iter().collect::<HashSet<_>>();

        // Check that the fields in the LHS of the FD are a subset of the
        // primary key for the table and that the RHS contains new fields
        let implies_fd = fd_lhs.is_subset(&left_key) &&
                         !fd_rhs.is_disjoint(&left_fields);

        if implies_fd {
          let left_vec = fd.lhs.clone().into_iter().collect::<Vec<_>>();
          let right_vec = fd.rhs.clone().into_iter().filter(|f| left_fields.contains(f)).collect::<Vec<_>>();
          Some((ind.left_table.clone(), left_vec, right_vec))
        } else {
          None
        }
      }).filter(|x| x.is_some()).map(|x| x.unwrap()));

    }
  }

  // Add any new FDs which were found
  for fd in new_fds {
    tables.get_mut(fd.0).unwrap().add_fd(fd.1, fd.2);
  }
}

fn main() {
  let filename = format!("examples/{}.txt", env::args().nth(1).unwrap());
  let input_string = read_file(&filename).unwrap();
  let (table_vec, fd_vec, ind_vec) = input::input(&input_string).unwrap();

  // Build a HashMap of parsed Tables
  let mut tables = HashMap::with_capacity(table_vec.len());
  for table in table_vec.into_iter() {
    tables.insert(table.name.clone(), table);
  }

  // Add the FDs to each table
  for fd in fd_vec.iter() {
    let mut table = tables.get_mut(&fd.0).unwrap();
    table.add_fd(
      fd.1.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
      fd.2.iter().map(|s| s.as_str()).collect::<Vec<_>>()
    );
  }

  // Create a HashMap of INDs from the parsed data
  let mut inds: HashMap<(&str, &str), Vec<dependencies::IND>> = HashMap::new();
  for ind in ind_vec.iter() {
    let new_ind = dependencies::IND {
      left_table: &ind.0,
      left_fields: ind.1.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
      right_table: &ind.2,
      right_fields: ind.3.iter().map(|s| s.as_str()).collect::<Vec<_>>()
    };

    let ind_key = (ind.0.as_str(), ind.2.as_str());
    if inds.contains_key(&ind_key) {
      let ind_list = inds.get_mut(&ind_key).unwrap();
      ind_list.push(new_ind);
    } else {
      inds.insert(ind_key, vec![new_ind]);
    }
  }

  let mut changed = true;
  while changed {
    changed = false;
    for table in tables.values_mut() {
      changed = changed || table.fds.closure(None);
    }
    copy_fds(&mut inds, &mut tables);
    changed = changed || inds.closure(Some(&mut tables));
    changed = changed || tables.normalize();
  }

  for table in tables.values() {
    println!("{}", table)
  }
}
