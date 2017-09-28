extern crate argparse;
#[cfg(test)]
#[macro_use]
extern crate collect_mac;
extern crate defaultmap;
extern crate float_ord;
extern crate itertools;
#[macro_use]
extern crate log;
extern crate ordermap;
extern crate permutation;
extern crate simple_logging;
extern crate string_intern;

use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::str::FromStr;

use argparse::{ArgumentParser, Store, StoreFalse, StoreOption, StoreTrue};
use log::LogLevelFilter;

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
use normalize::Normalizer;

fn read_file(name: &str) -> Result<String, io::Error> {
  let mut input_file = File::open(name)?;
  let mut input_string = String::new();
  input_file.read_to_string(&mut input_string)?;

  Ok(input_string)
}

struct Options {
  input: String,
  normalize: bool,
  subsume: bool,
  ignore_missing: bool,
  minimize: bool,
  retain_fks: bool,
  use_stats: bool,
  fd_threshold: Option<f32>,
  show_dependencies: bool,
  log_level: String,
}

fn main() {
  let mut options = Options {
    input: "".to_string(),
    normalize: true,
    subsume: true,
    ignore_missing: false,
    minimize: false,
    retain_fks: false,
    use_stats: false,
    fd_threshold: None,
    show_dependencies: false,
    log_level: "Off".to_string(),
  };
  {
    let mut ap = ArgumentParser::new();
    ap.set_description("NoSQL schema renormalization");
    ap.refer(&mut options.input)
      .add_argument("input", Store, "Example to run").required();
    ap.refer(&mut options.normalize)
      .add_option(&["--no-norm"], StoreFalse,
                    "Don't normalize");
    ap.refer(&mut options.subsume)
      .add_option(&["--no-subsume"], StoreFalse,
                    "Don't subsume tables");
    ap.refer(&mut options.ignore_missing)
      .add_option(&["-i", "--ignore-missing"], StoreTrue,
                    "Ignore dependencies with missing tables");
    ap.refer(&mut options.minimize)
      .add_option(&["-m", "--minimize-fds"], StoreTrue,
                    "For FDs which exist in both directions, \
                     select the one with the smallest left-hand side");
    ap.refer(&mut options.retain_fks)
      .add_option(&["-k", "--retain-fks"], StoreTrue,
                    "Keep only INDs representing foreign keys");
    ap.refer(&mut options.use_stats)
      .add_option(&["-s", "--use-stats"], StoreTrue,
                    "Use statistics to guide normalization");
    ap.refer(&mut options.fd_threshold)
      .add_option(&["-t", "--fd-threshold"], StoreOption,
                    "A threshold at which to discard FDs (requires --use-stats)");
    ap.refer(&mut options.show_dependencies)
      .add_option(&["-d", "--show-dependencies"], StoreTrue,
                    "Display the remaining dependencies on completion");
    ap.refer(&mut options.log_level)
      .add_option(&["-l", "--log-level"], Store,
                    "The level of logging to use");
    ap.parse_args_or_exit();
  }

  // Validate arguments
  if options.fd_threshold.is_some() && !options.use_stats {
    writeln!(io::stderr(), "Specifying --fd-threshold requires --use-stats").unwrap();
    ::std::process::exit(1);
  }

  let log_level = LogLevelFilter::from_str(options.log_level.as_str())
    .expect("invalid logging level");
  simple_logging::log_to_stderr(log_level).ok();

  info!("Loading schema {}", options.input);
  let input_string = read_file(&options.input).unwrap();
  let (table_vec, fd_vec, ind_vec, frequencies) = input::input(&input_string).unwrap();

  let mut schema = Schema { ..Default::default() };
  // Build a HashMap of parsed Tables
  for table in table_vec {
    schema.tables.insert(table.name.clone(), table);
  }

  // Copy frequencies to the tables and fields
  for freq in frequencies {
    let table = schema.tables.get_mut(&freq.0)
      .expect(&format!("found stats for unknown table {}", freq.0));
    match freq.1 {
      Some(field_name) => {
        let field = table.fields.get_mut(&field_name)
          .expect(&format!("found stats for unknown field {} on {}", field_name, freq.0));
        field.cardinality = Some(freq.2);
        field.cardinality = Some(freq.2);
        field.max_length = freq.3;
      }
      None => { table.row_count = Some(freq.2) }
    }
  }

  // Add the FDs to each table
  info!("Adding FDs");
  for fd in &fd_vec {
    if options.ignore_missing && !schema.tables.contains_key(&fd.0) {
      continue;
    }

    let table = schema.tables.get_mut(&fd.0)
      .expect(&format!("Missing table {} for FD", fd.0));
    table.add_fd(
      fd.1.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>(),
      fd.2.iter().map(|s| s.parse().unwrap()).collect::<Vec<_>>()
    );
  }

  // Adjust the primary keys using statistics if desired
  if options.use_stats {
    for table in schema.tables.values_mut() {
      table.set_primary_key(true);
    }
  }

  // Create a HashMap of INDs from the parsed data
  info!("Adding INDs");
  for ind in &ind_vec {
    let left_table = ind.0.parse().unwrap();
    let right_table =  ind.2.parse().unwrap();
    if options.ignore_missing &&
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
    if options.minimize {
      table.minimize_fds();
    }
    table.fds.closure();
  }

  if options.retain_fks {
    schema.retain_fk_inds();
  }

  schema.copy_fds();
  schema.ind_closure();

  let normalizer = Normalizer {
    use_stats: options.use_stats,
    fd_threshold: options.fd_threshold
  };

  let mut changed = true;
  while changed {
    info!("Looping");
    changed = false;

    if options.normalize {
      changed = normalizer.normalize(&mut schema) || changed;
    }

    if options.subsume {
      changed = normalizer.subsume(&mut schema) || changed;
    }
  }

  if options.show_dependencies {
    println!("{}", schema);
  } else {
    for table in schema.tables.values() {
      println!("{}", table);
    }
  }
}
