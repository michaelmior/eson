#![feature(plugin)]
#![plugin(peg_syntax_ext)]

mod cql;
mod dependencies;
mod model;

fn main() {
    let result = cql::create("CREATE TABLE foo (a int);");
    match result {
      Err(e) => println!("{}", e),
      Ok(table) => println!("{}", table.fields.get("a").unwrap().key)
    };
}
