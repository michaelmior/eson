#![feature(slice_patterns)]

#![feature(plugin)]
#![plugin(peg_syntax_ext)]

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
    let (tables, fds, inds) = input::input(&input_string).unwrap();
    println!("{}", tables[0].fields.get("id").unwrap().key);
}
