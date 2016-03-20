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

    // Create a HashMap of INDs from the parsed data
    let mut inds: HashMap<_, Vec<dependencies::IND>> = HashMap::new();
    for ind in ind_vec.into_iter() {
      let left_table = tables.get(&ind.0).unwrap();
      let right_table = tables.get(&ind.2).unwrap();

      let new_ind = dependencies::IND {
        left_table: left_table,
        left_fields: ind.1,
        right_table: right_table,
        right_fields: ind.3
      };

      let ind_key = (ind.0.clone(), ind.2.clone());
      if inds.contains_key(&ind_key) {
        let ind_list = inds.get_mut(&ind_key).unwrap();
        ind_list.push(new_ind);
      } else {
        inds.insert(ind_key, vec![new_ind]);
      }
    }
}
