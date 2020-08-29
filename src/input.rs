extern crate peg;

use std::str;

use indexmap::IndexMap;

use super::model::{Field, Table};


peg::parser!{
  pub grammar input() for str {
    rule space()
      = quiet!{[' '| '\t' | '\r' | '\n']+}

    rule identifier() -> String
      = id:$(['A'..='Z' | 'a'..='z' | '_']+['A'..='Z' | 'a'..='z' | '0'..='9' | '_']*) { id.to_string() }

    rule identifiers() -> Vec<String>
      = i:identifier() **<1,> (space()? "," space()?) { i }

    rule integer() -> usize
      = i:$(['0'..='9']+) { i.parse().unwrap() }

    rule table() -> String
      = table:identifier() { table }

    rule field_define() -> Field
      = key:"*"? name:identifier() {
          let parsed_name = name.parse().expect(&format!("Invalid field name {}", name));
          Field {
            name: parsed_name,
            key: key.is_some(),
            cardinality: None,
            max_length: None
          }
        }

    rule field_defines() -> Vec<Field>
      = field_define() ** (space()? "," space()?)

    rule create() -> Table
      = table:table() space()?
        "(" space()? fields:field_defines() space()? ")" {
          let mut field_map = IndexMap::new();
          for field in fields {
            field_map.insert(field.name.clone(), field);
          }

          let parsed_name = table.parse().expect(&format!("Invalid table name {}", table));
          let mut t = Table {
            name: parsed_name,
            fields: field_map,
            ..Default::default()
          };
          t.add_pk_fd();

          t
        }

    rule func_dep() -> (String, Vec<String>, Vec<String>)
      = table:identifier() space() lhs:identifiers() space() "->"
        space() rhs:identifiers() { (table, lhs, rhs) }

    rule inc_dir() -> String
      = dir:$("<=" / "==") { dir.to_string() }

    rule inc_dep() -> Vec<(String, Vec<String>, String, Vec<String>)>
      = left_table:identifier() space() left_fields:identifiers()
        space() dir:inc_dir() space()
        right_table:identifier() space() maybe_right_fields:(ids:identifiers() { Some(ids) } / "..." { None })  {
          let right_fields = match maybe_right_fields {
            Some(fields) => fields,
            None => left_fields.clone()
          };

          let mut inds: Vec<_> = Vec::new();
          if dir == "==" {
            inds.push((right_table.clone(), right_fields.clone(), left_table.clone(), left_fields.clone()));
          }
          inds.push((left_table, left_fields, right_table, right_fields));

          inds
        }

    rule table_frequency() -> (String, Option<String>, usize, Option<usize>)
      = table:identifier() space() count:integer() {
        (table, None, count, None)
      }

    rule column_frequency() -> (String, Option<String>, usize, Option<usize>)
      = table:identifier() space() column:identifier() space() count:integer() space() max_length:integer() {
        (table, Some(column), count, Some(max_length))
      }

    rule frequency() -> (String, Option<String>, usize, Option<usize>)
      = table_frequency() / column_frequency()

    pub rule input() -> (Vec<Table>, Vec<(String, Vec<String>, Vec<String>)>,
              Vec<(String, Vec<String>, String, Vec<String>)>,
              Vec<(String, Option<String>, usize, Option<usize>)>)
      = tables:(create() **<1,> "\n") "\n"*
        func_deps:(func_dep() ** "\n") "\n"*
        inc_deps:(inc_dep() ** "\n") "\n"*
        frequencies:((frequency() ** "\n"))? "\n"* {
          (tables, func_deps, inc_deps.into_iter().flat_map(|i| i).collect::<Vec<_>>(), frequencies.unwrap_or(Vec::new()))
        }
  }
}
