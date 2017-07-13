use string_intern::{Validator, Symbol};

pub struct FieldNameSymbol;

impl Validator for FieldNameSymbol {
  type Err = ::std::string::ParseError;
  fn validate_symbol(_: &str) -> Result<(), Self::Err> {
    Ok(())
  }
}

pub type FieldName = Symbol<FieldNameSymbol>;

pub struct TableNameSymbol;

impl Validator for TableNameSymbol {
  type Err = ::std::string::ParseError;
  fn validate_symbol(_: &str) -> Result<(), Self::Err> {
    Ok(())
  }
}

pub type TableName = Symbol<TableNameSymbol>;
