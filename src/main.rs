#![feature(slice_patterns)]

#![feature(plugin)]
#![plugin(peg_syntax_ext)]

use std::collections::HashMap;
use std::io;
use std::io::prelude::*;
use std::fs::File;

mod input;
mod dependencies;
mod model;

fn read_file(name: &str) -> Result<String, io::Error> {
  let mut input_file = try!(File::open(name));
  let mut input_string = String::new();
  try!(input_file.read_to_string(&mut input_string));

  Ok(input_string)
}

fn main() {
    let input_string = read_file("examples/one_to_one.txt").unwrap();
    let (table_vec, fd_vec, ind_vec) = input::input(&input_string).unwrap();

    // Build a HashMap of parsed Tables
    let mut tables = HashMap::with_capacity(table_vec.len());
    for table in table_vec.into_iter() {
      tables.insert(table.name.clone(), table);
    }

    // Add the FDs to each table
    for fd in fd_vec.into_iter() {
      let mut table = tables.get_mut(&fd.0).unwrap();
      table.add_fd(fd.1, fd.2);
    }

    // Create a vector of INDs from the parsed data
    let mut inds = Vec::new();
    for ind in ind_vec.into_iter() {
      let left_table = tables.get(&ind.0).unwrap();
      let right_table = tables.get(&ind.2).unwrap();

      inds.push(dependencies::IND {
        left_table: left_table,
        left_fields: ind.1,
        right_table: right_table,
        right_fields: ind.3
      })
    }
}
