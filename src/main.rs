#![feature(slice_patterns)]

extern crate argparse;
#[cfg(test)]
#[macro_use]
extern crate collect_mac;
extern crate env_logger;
extern crate itertools;
#[macro_use]
extern crate log;
extern crate permutation;
extern crate string_intern;

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io;
use std::io::prelude::*;

use argparse::{ArgumentParser, Store, StoreFalse};

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
use symbols::TableName;

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
    let mut table = schema.tables.get_mut(&fd.0).unwrap();
    table.add_fd(
      fd.1.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>(),
      fd.2.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>()
    );
  }

  // Create a HashMap of INDs from the parsed data
  info!("Adding INDs");
  for ind in &ind_vec {
    let lhs = ind.1.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>();
    let permutation = permutation::sort(&lhs[..]);

    let rhs = ind.3.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>();

    let new_ind = dependencies::IND {
      left_table: ind.0.parse().unwrap(),
      left_fields: permutation.apply_slice(&lhs[..]),
      right_table: ind.2.parse().unwrap(),
      right_fields: permutation.apply_slice(&rhs[..])
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
    schema.copy_fds();

    changed = changed || schema.ind_closure();

    if normalize {
      changed = changed || schema.normalize();
    }

    if subsume {
      changed = changed || schema.subsume();
    }
  }

  println!("{}", schema);
}
