#![feature(slice_patterns)]

extern crate argparse;
#[cfg(test)]
#[macro_use]
extern crate collect_mac;
extern crate defaultmap;
extern crate env_logger;
extern crate itertools;
#[macro_use]
extern crate log;
extern crate permutation;
extern crate string_intern;

use std::fs::File;
use std::io;
use std::io::prelude::*;

use argparse::{ArgumentParser, Store, StoreFalse, StoreTrue};

#[macro_use]
mod macros;
mod dependencies;
mod model;
mod normalize;
mod symbols;

mod input {
  include!(concat!(env!("OUT_DIR"), "/input.rs"));
}

use dependencies::{FDClosure, INDClosure};
use model::Schema;
use normalize::Normalizable;

fn read_file(name: &str) -> Result<String, io::Error> {
  let mut input_file = File::open(name)?;
  let mut input_string = String::new();
  input_file.read_to_string(&mut input_string)?;

  Ok(input_string)
}

fn main() {
  env_logger::init().unwrap();

  let mut input = "".to_string();
  let mut normalize = true;
  let mut subsume = true;
  let mut ignore_missing = false;
  let mut minimize = false;
  let mut retain_fks = false;
  {
    let mut ap = ArgumentParser::new();
    ap.set_description("NoSQL schema renormalization");
    ap.refer(&mut input)
      .add_argument("input", Store, "Example to run").required();
    ap.refer(&mut normalize)
      .add_option(&["--no-norm"], StoreFalse,
                    "Don't normalize");
    ap.refer(&mut subsume)
      .add_option(&["--no-subsume"], StoreFalse,
                    "Don't subsume tables");
    ap.refer(&mut ignore_missing)
      .add_option(&["-i", "--ignore-missing"], StoreTrue,
                    "Ignore dependencies with missing tables");
    ap.refer(&mut minimize)
      .add_option(&["-m", "--minimize-fds"], StoreTrue,
                    "For FDs which exist in both directions, \
                     select the one with the smallest left-hand side");
    ap.refer(&mut retain_fks)
      .add_option(&["-k", "--retain-fks"], StoreTrue,
                    "Keep only INDs representing foreign keys");
    ap.parse_args_or_exit();
  }

  let filename = format!("examples/{}.txt", input);
  info!("Loading schema {}", filename);
  let input_string = read_file(&filename).unwrap();
  let (table_vec, fd_vec, ind_vec) = input::input(&input_string).unwrap();

  let mut schema = Schema { ..Default::default() };
  // Build a HashMap of parsed Tables
  for table in table_vec {
    schema.tables.insert(table.name.clone(), table);
  }

  // Add the FDs to each table
  info!("Adding FDs");
  for fd in &fd_vec {
    if ignore_missing && !schema.tables.contains_key(&fd.0) {
      continue;
    }

    let mut table = schema.tables.get_mut(&fd.0)
      .expect(&format!("Missing table {} for FD", fd.0));
    table.add_fd(
      fd.1.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>(),
      fd.2.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>()
    );
  }

  // Create a HashMap of INDs from the parsed data
  info!("Adding INDs");
  for ind in &ind_vec {
    let left_table = ind.0.parse().unwrap();
    let right_table =  ind.2.parse().unwrap();
    if ignore_missing &&
        !(schema.tables.contains_key(&left_table) &&
          schema.tables.contains_key(&right_table)) {
      continue;
    }

    let lhs = ind.1.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>();
    let permutation = permutation::sort(&lhs[..]);
    let rhs = ind.3.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>();

    let new_ind = dependencies::IND {
      left_table: left_table,
      left_fields: permutation.apply_slice(&lhs[..]),
      right_table: right_table,
      right_fields: permutation.apply_slice(&rhs[..])
    };
    schema.add_ind(new_ind);
  }

  for table in schema.tables.values_mut() {
    if minimize {
      table.minimize_fds();
    }
    table.fds.closure();
  }

  if retain_fks {
    schema.retain_fk_inds();
  }

  schema.copy_fds();
  schema.ind_closure();

  if normalize {
    schema.normalize();
  }

  if subsume {
    schema.subsume();
  }

  println!("{}", schema);
}
