# eson

[![Build Status](https://travis-ci.org/michaelmior/eson.svg?branch=master)](https://travis-ci.org/michaelmior/eson)
[![Build status](https://ci.appveyor.com/api/projects/status/ovsa4d9klcextjju?svg=true)](https://ci.appveyor.com/project/michaelmior/eson)

`eson` is a work in progress tool to extract a normalized schema from a denormalized relational schema.
The hope is that it can be useful for understanding and managing schemas of NoSQL applications.

## Installation

If you are a Rust user, you can install eson with `cargo install eson`.
Otherwise, you can download a Linux, Windows, or Mac binary from the [latest release](https://github.com/michaelmior/eson/releases/latest).

## Input format

Example input files are available in the `examples` directory.
Input files to `eson` are split into four different sections.
The first specifies the denormalized input relations in the following format:

```
users(*user_id, first_name, last_name)
```

Fields marked with a `*` compose the primary key of that relation.
The second section specifies functional dependencies for each table.
The table name is given first, followed by the left and right-hand sides of the dependency.

```
users user_id -> first_name, last_name
```

Inclusion dependencies are specified in a similar manner as in the examples below:

```
employees user_id <= users user_id
users user_id <= employees user_id
```

There are two shortcuts which can be used in this section.
Firstly, if the inclusion dependency applies in both directions, then `==` can be used instead of separately specifying two dependencies.
Second, if the fields on the right-hand side are the same as those on the left, `...` can be used to replace the fields on the right.
Employing both of these shortcuts, the two dependencies above can be written as:

```
employees user_id == users ...
```

The final section is optional and specifies statistics on tables and columns when using a heuristics-based approach for ordering functional dependencies (the `--use-stats` option).
Statistics for a relation simply list the total number of entries in the relation.
Statistics for a column list the number of unique values as well as the maximum length.

```
users 1000
users user_id 1000 1
```
