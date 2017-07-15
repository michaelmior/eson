#![feature(plugin)]
#![feature(slice_patterns)]

#![plugin(peg_syntax_ext)]

#[cfg(test)]
#[macro_use] extern crate collect_mac;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate string_intern;

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
mod symbols;

use dependencies::{FDClosure, INDClosure};
use model::Schema;
use normalize::Normalizable;
use symbols::TableName;

fn read_file(name: &str) -> Result<String, io::Error> {
  let mut input_file = try!(File::open(name));
  let mut input_string = String::new();
  try!(input_file.read_to_string(&mut input_string));

  Ok(input_string)
}

// Copy FDs between tables based on inclusion dependencies
fn copy_fds(inds: &mut HashMap<(TableName, TableName), Vec<dependencies::IND>>, tables: &mut HashMap<TableName, model::Table>) -> () {
  let mut new_fds = Vec::new();

  // Loop over all FDs
  for ind_vec in inds.values() {
    for ind in ind_vec.iter() {
      let mut left_fields = <HashSet<_>>::new();
      for field in tables.get(&ind.left_table).unwrap().fields.keys() {
        left_fields.insert(field.clone());
      }
      // let left_fields = tables.get(&ind.left_table).unwrap()
      //     .fields.keys().map(|f| *f).into_iter().collect::<HashSet<_>>();
      let left_key = tables.get(&ind.left_table).unwrap()
          .fields.values().filter(|f| f.key).map(|f| f.name.clone()).into_iter().collect::<HashSet<_>>();

      new_fds.extend(tables.get(&ind.right_table).unwrap().fds.values().map(|fd| {
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
    tables.get_mut(&fd.0).unwrap().add_fd(fd.1, fd.2);
  }
}

fn main() {
  env_logger::init().unwrap();

  let filename = format!("examples/{}.txt", env::args().nth(1).unwrap());
  info!("Loading schema {}", filename);
  let input_string = read_file(&filename).unwrap();
  let (table_vec, fd_vec, ind_vec) = input::input(&input_string).unwrap();

  let mut schema = Schema { ..Default::default() };
  // Build a HashMap of parsed Tables
  for table in table_vec.into_iter() {
    schema.tables.insert(table.name.clone(), table);
  }

  // Add the FDs to each table
  info!("Adding FDs");
  for fd in fd_vec.iter() {
    let mut table = schema.tables.get_mut(&fd.0).unwrap();
    table.add_fd(
      fd.1.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>(),
      fd.2.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>()
    );
  }

  // Create a HashMap of INDs from the parsed data
  info!("Adding INDs");
  for ind in ind_vec.iter() {
    let new_ind = dependencies::IND {
      left_table: ind.0.parse().unwrap(),
      left_fields: ind.1.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>(),
      right_table: ind.2.parse().unwrap(),
      right_fields: ind.3.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>()
    };
    schema.add_ind(new_ind);
  }

  let mut changed = true;
  while changed {
    info!("Looping");
    changed = false;
    for table in schema.tables.values_mut() {
      changed = changed || table.fds.closure();
    }
    copy_fds(&mut schema.inds, &mut schema.tables);
    changed = changed || schema.ind_closure();
    changed = changed || schema.normalize();
  }

  println!("{}", schema);
}
