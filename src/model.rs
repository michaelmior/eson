use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::usize;

use defaultmap::DefaultHashMap;
use float_ord::FloatOrd;
use indexmap::IndexMap;

use crate::dependencies::{FDClosure, FD, IND};
use crate::symbols::{FieldName, TableName};

/// A schema encapsulating tables and their dependencies
#[derive(Default)]
pub struct Schema {
    /// Tables keyed by their name
    pub tables: HashMap<TableName, Table>,

    /// Inclusion dependencies between tables
    pub inds: DefaultHashMap<(TableName, TableName), Vec<IND>>,
}

impl fmt::Display for Schema {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for table in self.tables.values() {
            writeln!(f, "{}", table)?;
            for fd in table.fds.values() {
                writeln!(f, "  {}", fd)?;
            }
            writeln!(f)?;
        }

        for ind_group in self.inds.values() {
            for ind in ind_group {
                writeln!(f, "{}", ind)?;
            }
        }

        Ok(())
    }
}

impl Schema {
    /// Add a new `IND` to the schema
    pub fn add_ind(&mut self, ind: IND) -> bool {
        let ind_key = (ind.left_table.clone(), ind.right_table.clone());
        let inds = self.inds.get_mut(ind_key);

        if inds.iter().any(|ind2| ind.is_subset(ind2)) {
            false
        } else {
            inds.push(ind);
            true
        }
    }

    #[allow(dead_code)]
    pub fn delete_ind(&mut self, ind: &IND) {
        for inds in self.inds.values_mut() {
            let index = inds.iter().position(|i| i == ind);
            if index.is_some() {
                inds.remove(index.unwrap());
            }
        }
    }

    /// Check if this schema contains a given IND
    pub fn contains_ind(&self, ind: &IND) -> bool {
        self.inds
            .values()
            .any(|inds| inds.iter().any(|i| ind.is_subset(i)))
    }

    /// Copy `IND`s from the table in `src` to the table in `dst`
    pub fn copy_inds(&mut self, src: &TableName, dst: &TableName) {
        let mut new_inds = Vec::new();
        {
            let dst_table = &self.tables[dst];
            for ind_group in self.inds.values() {
                for ind in ind_group {
                    if ind.left_table == *src
                        && ind
                            .left_fields
                            .iter()
                            .any(|f| dst_table.fields.contains_key(f))
                    {
                        let new_ind = IND {
                            left_table: dst.clone(),
                            left_fields: ind.left_fields.clone(),
                            right_table: ind.right_table.clone(),
                            right_fields: ind.right_fields.clone(),
                        };
                        new_inds.push(new_ind);
                    }

                    if ind.right_table == *src
                        && ind
                            .right_fields
                            .iter()
                            .any(|f| dst_table.fields.contains_key(f))
                    {
                        let new_ind = IND {
                            left_table: ind.left_table.clone(),
                            left_fields: ind.left_fields.clone(),
                            right_table: dst.clone(),
                            right_fields: ind.right_fields.clone(),
                        };
                        new_inds.push(new_ind);
                    }
                }
            }
        }

        for new_ind in new_inds {
            self.add_ind(new_ind);
        }

        self.prune_inds();
    }

    /// Prune `IND`s which reference tables which no longer exist
    pub fn prune_inds(&mut self) {
        let tables = self.tables.keys().collect::<HashSet<&TableName>>();
        self.inds
            .retain(|key, _| tables.contains(&key.0) && tables.contains(&key.1));

        for inds in self.inds.values_mut() {
            for ind in inds.iter_mut() {
                // Get the indexes of all fields in each table to keep
                let left_table = &self.tables[&ind.left_table];
                let right_table = &self.tables[&ind.right_table];
                let left_indexes = ind
                    .left_fields
                    .iter()
                    .enumerate()
                    .filter(|&(_, field)| left_table.fields.contains_key(field))
                    .map(|(i, _)| i)
                    .collect::<HashSet<_>>();
                let right_indexes = ind
                    .right_fields
                    .iter()
                    .enumerate()
                    .filter(|&(_, field)| right_table.fields.contains_key(field))
                    .map(|(i, _)| i)
                    .collect::<HashSet<_>>();

                // We can only keep fields which are in both tables
                let retain_indexes = left_indexes
                    .intersection(&right_indexes)
                    .collect::<HashSet<_>>();
                for index in (0..ind.left_fields.len()).rev() {
                    if !retain_indexes.contains(&index) {
                        ind.left_fields.remove(index);
                        ind.right_fields.remove(index);
                    }
                }
            }
        }

        // Remove any INDs which are now empty
        for inds in self.inds.values_mut() {
            inds.retain(|ind| !ind.left_fields.is_empty() && !ind.right_fields.is_empty());
        }
    }

    /// Remove INDs which do not represent foreign keys
    pub fn retain_fk_inds(&mut self) {
        for (&(_, ref right_table), ref mut inds) in self.inds.iter_mut() {
            let right_table = &self.tables[right_table];
            inds.retain(|ind| match right_table.fds.get(&ind.left_fields) {
                Some(fd) => ind
                    .right_fields
                    .clone()
                    .into_iter()
                    .collect::<HashSet<_>>()
                    .is_subset(&fd.rhs),
                None => {
                    debug!("Removing {} since it does not represent a foreign key", ind);
                    false
                }
            })
        }
    }

    /// Copy FDs between tables based on inclusion dependencies
    pub fn copy_fds(&mut self) {
        let mut new_fds = Vec::new();

        // Loop over INDs
        for ind_vec in self.inds.values() {
            for ind in ind_vec.iter() {
                let mut left_fields = <HashSet<_>>::new();
                let left_table = self
                    .tables
                    .get(&ind.left_table)
                    .expect(&format!("Table for LHS of IND {} does not exist", ind));
                for field in left_table.fields.keys() {
                    left_fields.insert(field.clone());
                }
                let left_key = left_table
                    .fields
                    .values()
                    .filter(|f| f.key)
                    .map(|f| f.name.clone())
                    .into_iter()
                    .collect::<HashSet<_>>();

                let right_table = self
                    .tables
                    .get(&ind.right_table)
                    .expect(&format!("Table for RHS of IND {} does not exist", ind));
                new_fds.extend(
                    right_table
                        .fds
                        .values()
                        .map(|fd| {
                            let fd_lhs = fd.lhs.clone().into_iter().collect::<HashSet<_>>();
                            let fd_rhs = fd.rhs.clone().into_iter().collect::<HashSet<_>>();

                            // Check that the fields in the LHS of the FD are a subset of the
                            // primary key for the table and that the RHS contains new fields
                            let implies_fd =
                                fd_lhs.is_subset(&left_key) && !fd_rhs.is_disjoint(&left_fields);

                            if implies_fd {
                                let left_vec = fd.lhs.clone().into_iter().collect::<Vec<_>>();
                                let right_vec = fd
                                    .rhs
                                    .clone()
                                    .into_iter()
                                    .filter(|f| left_fields.contains(f))
                                    .collect::<Vec<_>>();
                                Some((ind.left_table.clone(), left_vec, right_vec))
                            } else {
                                None
                            }
                        })
                        .filter(|x| x.is_some())
                        .map(|x| x.unwrap()),
                );
            }
        }

        // Add any new FDs which were found
        for fd in new_fds {
            self.tables.get_mut(&fd.0).unwrap().add_fd(fd.1, fd.2);
        }
    }

    /// Check that all of the `FD`s in the schema are valid
    #[cfg(test)]
    fn validate_fds(&self) {
        for table in self.tables.values() {
            for (key, fd) in table.fds.iter() {
                // Ensure the key in the hash table is correct
                let mut lhs = fd.lhs.iter().map(|f| (*f).clone()).collect::<Vec<_>>();
                lhs.sort();
                assert_eq!(lhs, *key, "FD key {:?} does not match for {}", key, fd);

                // Check that the table contains all the fields
                assert!(
                    fd.lhs.iter().all(|f| table.fields.contains_key(f)),
                    "Missing fields for LHS of {}",
                    fd
                );
                assert!(
                    fd.rhs.iter().all(|f| table.fields.contains_key(f)),
                    "Missing fields for RHS of {}",
                    fd
                );
            }
        }
    }

    /// Check that all of the `IND`s in the schema are valid
    #[cfg(test)]
    fn validate_inds(&self) {
        for (ind_key, inds) in self.inds.iter() {
            for ind in inds {
                assert_eq!(
                    *ind_key,
                    (ind.left_table.clone(), ind.right_table.clone()),
                    "IND key {:?} does not match for {}",
                    ind_key,
                    ind
                );

                // Check that the left table and its fields exist
                let left_table = self.tables.get(&ind.left_table).expect(&format!(
                    "Table {} not found for IND {}",
                    ind.left_table, ind
                ));
                assert!(
                    ind.left_fields
                        .iter()
                        .all(|f| left_table.fields.contains_key(f)),
                    "Missing fields for LHS of {}",
                    ind
                );

                // Check that the right table and its fields exist
                let right_table = self.tables.get(&ind.right_table).expect(&format!(
                    "Table {} not found for IND {}",
                    ind.right_table, ind
                ));
                assert!(
                    ind.right_fields
                        .iter()
                        .all(|f| right_table.fields.contains_key(f)),
                    "Missing fields for RHS of {}",
                    ind
                );
            }
        }
    }

    /// Ensure all the dependencies are consistent with the tables
    #[cfg(test)]
    pub fn validate(&self) {
        self.validate_fds();
        self.validate_inds();
    }
}

/// A field inside a `Table`
#[derive(Clone, Debug, Eq)]
pub struct Field {
    /// The name of the field
    pub name: FieldName,

    /// Whether this field is a key of its parent `Table`
    pub key: bool,

    /// The cardinality of this field
    pub cardinality: Option<usize>,

    /// The maximum length of values in this field
    pub max_length: Option<usize>,
}

impl PartialEq for Field {
    fn eq(&self, other: &Field) -> bool {
        self.name == other.name
    }
}

impl Hash for Field {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

/// A table, it's field and any intra-table dependencies
#[derive(Debug)]
pub struct Table {
    /// The name of the table
    pub name: TableName,

    /// All `Field`s in the table keyed by the name
    pub fields: IndexMap<FieldName, Field>,

    /// Functional dependencies keyed by their left-hand side
    pub fds: HashMap<Vec<FieldName>, FD>,

    /// The number of rows in this table
    pub row_count: Option<usize>,
}

impl Default for Table {
    fn default() -> Table {
        Table {
            name: TableName::from(""),
            fields: IndexMap::new(),
            fds: HashMap::new(),
            row_count: None,
        }
    }
}

impl PartialEq for Table {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Hash for Table {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl fmt::Display for Table {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut field_names: Vec<_> = self
            .fields
            .values()
            .map(|f| {
                if f.key {
                    let mut key_name = "*".to_owned();
                    key_name.push_str(f.name.as_ref());
                    key_name
                } else {
                    f.name.to_string()
                }
            })
            .collect();
        field_names.sort();
        let fields = field_names.join(", ");
        write!(f, "{}({})", &self.name, &fields)
    }
}

impl Table {
    /// Add an FD for the primary key
    pub fn add_pk_fd(&mut self) {
        let (lhs_names, rhs_names) = {
            let (lhs, rhs): (Vec<_>, Vec<_>) = self.fields.values().partition(|f| f.key);

            (
                lhs.iter().map(|f| f.name.clone()).collect::<Vec<_>>(),
                rhs.iter().map(|f| f.name.clone()).collect::<Vec<_>>(),
            )
        };
        if !lhs_names.is_empty() && !rhs_names.is_empty() {
            self.add_fd(lhs_names, rhs_names);
        }
    }

    /// Calculate field positions for scoring
    fn get_field_positions(&self, fields: &HashSet<FieldName>) -> (f32, f32) {
        let mut indexes = fields
            .iter()
            .map(|f| self.fields.get_full(f).unwrap().0)
            .collect::<Vec<_>>();
        indexes.sort();

        let left = indexes[0] as f32;

        // Count the number of fields between each field by subtracting subsequent indexes
        let between = if indexes.len() == 1 {
            0.0
        } else {
            (&indexes[1..indexes.len()])
                .iter()
                .fold((0, indexes[0]), |(sum, last), index| {
                    (sum + (index - last - 1), last)
                })
                .0 as f32
        };

        (left, between)
    }

    /// Pick a primary key from the set of FDs
    pub fn set_primary_key(&mut self, use_stats: bool) {
        let pk = {
            let mut pks = self
                .fds
                .values()
                .filter(|fd| fd.lhs.len() + fd.rhs.len() == self.fields.len());

            if use_stats {
                pks.max_by_key(|fd| {
                    // Below is taken from https://dx.doi.org/10.5441/002/edbt.2017.31

                    let length_score = 1.0 / fd.lhs.len() as f32;

                    let total_length: usize = fd
                        .lhs
                        .iter()
                        .map(|f| {
                            self.fields[f]
                                .max_length
                                .expect(&format!("No max length for {} in {}", f, self.name))
                        })
                        .sum();
                    let value_score = 1.0 / (f32::max(1.0, total_length as f32 - 7.0) as f32);

                    // Get the position of each field in the table
                    let (left, between) = self.get_field_positions(&fd.lhs);
                    let position_score = 0.5 * (1.0 / (left + 1.0) + 1.0 / (between + 1.0));

                    FloatOrd(length_score + value_score + position_score)
                })
                .expect(&format!("No primary key found for {}", self))
            } else {
                pks.next()
                    .expect(&format!("No primary key found for {}", self))
            }
        };

        for field in self.fields.values_mut() {
            field.key = pk.lhs.contains(&field.name);
        }
    }

    /// Add a new `FD` to this table
    pub fn add_fd(&mut self, mut lhs: Vec<FieldName>, mut rhs: Vec<FieldName>) {
        lhs.sort();
        lhs.dedup();

        // Merge this FD with others having the same LHS
        let key = &lhs.to_vec();
        if self.fds.contains_key(key) {
            let old_fd = self.fds.remove(key).unwrap();
            rhs.extend(old_fd.rhs.into_iter());
        }

        let left_set = lhs.into_iter().collect::<HashSet<_>>();
        let right_set = rhs.into_iter().collect::<HashSet<_>>();

        self.fds.insert(
            key.clone(),
            FD {
                lhs: left_set,
                rhs: right_set,
            },
        );
        self.fds.closure();
    }

    /// Check if this table contains a given FD
    #[allow(dead_code)]
    pub fn contains_fd(&self, fd: &FD) -> bool {
        let mut key = fd.lhs.clone().into_iter().collect::<Vec<_>>();
        key.sort();

        match self.fds.get(&key) {
            Some(table_fd) => fd.rhs.is_subset(&table_fd.rhs),
            None => false,
        }
    }

    /// Copy `FD`s from another given `Table`
    pub fn copy_fds(&mut self, other: &Table) {
        for fd in other.fds.values() {
            let new_lhs = fd
                .lhs
                .clone()
                .into_iter()
                .filter(|f| self.fields.contains_key(f))
                .collect::<Vec<_>>();
            let new_rhs = fd
                .rhs
                .clone()
                .into_iter()
                .filter(|f| self.fields.contains_key(f))
                .collect::<Vec<_>>();
            if !new_lhs.is_empty() && !new_rhs.is_empty() {
                self.add_fd(new_lhs, new_rhs);
            }
        }
    }

    /// Produce all fields marked as a key
    pub fn key_fields(&self) -> HashSet<FieldName> {
        self.fields
            .values()
            .filter(|f| f.key)
            .map(|f| f.name.clone())
            .collect::<HashSet<_>>()
    }

    /// Check if a set of fields is a superkey for this table
    pub fn is_superkey(&self, fields: &HashSet<FieldName>) -> bool {
        self.key_fields().is_subset(fields)
    }

    /// Check if this table is in BCNF according to its functional dependencies
    pub fn is_bcnf(&self, skip_keys: bool, fd_threshold: Option<f32>) -> bool {
        self.violating_fd(skip_keys, fd_threshold).is_none()
    }

    /// Find a functional dependency which violates BCNF
    pub fn violating_fd(&self, use_stats: bool, fd_threshold: Option<f32>) -> Option<&FD> {
        let mut violators = self
            .fds
            .values()
            .filter(|fd| !fd.is_trivial() && !self.is_superkey(&fd.lhs));

        if use_stats {
            let vfd = violators
                .filter(|fd| fd.lhs.len() + fd.rhs.len() < self.fields.len())
                .map(|fd| {
                    // Below is taken from https://dx.doi.org/10.5441/002/edbt.2017.31
                    let length_score = 0.5
                        * (1.0 / fd.lhs.len() as f32
                            + 1.0 / (fd.rhs.len() as f32) / (self.fields.len() as f32 - 2.0));

                    let total_length: usize = fd
                        .lhs
                        .iter()
                        .map(|f| {
                            self.fields[f]
                                .max_length
                                .expect(&format!("No max length for {} in {}", f, self.name))
                        })
                        .sum();
                    let value_score = 1.0 / (f32::max(1.0, total_length as f32 - 7.0) as f32);

                    let (_, left_between) = self.get_field_positions(&fd.lhs);
                    let (_, right_between) = self.get_field_positions(&fd.rhs);
                    let position_score =
                        0.5 * (1.0 / (left_between + 1.0) + 1.0 / (right_between + 1.0));

                    // TODO: Add duplication score

                    (fd, FloatOrd(length_score + value_score + position_score))
                })
                .max_by_key(|&(_, score)| score);
            match vfd {
                Some((fd, score)) => {
                    if fd_threshold.is_none() || score > FloatOrd(fd_threshold.unwrap()) {
                        Some(fd)
                    } else {
                        None
                    }
                }
                None => None,
            }
        } else {
            violators.next()
        }
    }

    /// Prune `FD`s which reference fields which no longer exist
    pub fn prune_fds(&mut self) {
        let fields = self.fields.keys().collect::<HashSet<_>>();
        for fd in self.fds.values_mut() {
            fd.lhs.retain(|f| fields.contains(&f));
            fd.rhs.retain(|f| fields.contains(&f));
        }

        self.fds
            .retain(|_, fd| !fd.lhs.is_empty() && !fd.rhs.is_empty());
    }

    /// Minimize the set of functional dependencies such that
    /// each `FD` `A->B` is removed if the `FD` `B->A` also
    /// exists and `|B| < |A|`
    pub fn minimize_fds(&mut self) {
        let mut remove_fds = Vec::new();

        for fd in self.fds.values() {
            let reverse = fd.reverse();
            let rhs = fd.rhs.clone().into_iter().collect::<Vec<_>>();
            if self.fds.contains_key(&rhs)
                && self.fds[&rhs] == reverse
                && fd.lhs.len() > reverse.lhs.len()
            {
                let mut key = fd.lhs.clone().into_iter().collect::<Vec<_>>();
                debug!("Removing {} due to minimization", fd);
                key.sort();
                remove_fds.push(key);
            }
        }

        for fd in remove_fds {
            self.fds.remove(&fd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_equal_by_name() {
        let t1 = table!("foo");
        let t2 = table!("foo");
        assert_eq!(t1, t2)
    }

    #[test]
    fn table_format_string() {
        let t = table!(
            "foo",
            fields! {
              field!("bar", true),
              field!("baz")
            }
        );
        assert_eq!(format!("{}", t), "foo(*bar, baz)")
    }

    #[test]
    fn table_add_pk_fd() {
        let mut t = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar")
            }
        );

        t.add_pk_fd();

        let fd = FD {
            lhs: field_set!["foo"],
            rhs: field_set!["bar"],
        };

        assert!(t.contains_fd(&fd))
    }

    #[test]
    fn table_is_bcnf_yes() {
        let mut t = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar")
            }
        );
        add_fd!(t, vec!["foo"], vec!["bar"]);
        assert!(t.is_bcnf(false, None))
    }

    #[test]
    fn table_violating_fd_no_stats() {
        let mut t = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar")
            }
        );
        add_fd!(t, vec!["bar"], vec!["foo"]);
        let fd = FD {
            lhs: field_set!["bar"],
            rhs: field_set!["foo"],
        };
        assert_eq!(t.violating_fd(false, None).unwrap(), &fd)
    }

    #[test]
    fn table_no_violating_fd() {
        let mut t = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar")
            }
        );
        add_fd!(t, vec!["foo"], vec!["bar"]);
        assert!(t.violating_fd(false, None).is_none())
    }

    #[test]
    fn table_violating_fd_length() {
        let mut t = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar"),
              field!("baz"),
              field!("quux")
            }
        );
        add_fd!(t, vec!["bar", "baz"], vec!["quux"]);
        add_fd!(t, vec!["bar"], vec!["baz", "quux"]);

        assert_eq!(t.violating_fd(true, None).unwrap().lhs.len(), 1);
    }

    #[test]
    fn table_violating_fd_value() {
        let mut t = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar", false, 1, usize::MAX),
              field!("baz"),
              field!("quux")
            }
        );
        add_fd!(t, vec!["baz"], vec!["bar", "quux"]);
        add_fd!(t, vec!["bar"], vec!["baz", "quux"]);

        let lhs = &t.violating_fd(true, None).unwrap().lhs;
        assert_eq!(*lhs.iter().next().unwrap(), FieldName::from("baz"));
    }

    #[test]
    fn table_violating_fd_position() {
        let mut t = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar"),
              field!("baz"),
              field!("quux"),
              field!("qux"),
              field!("corge"),
              field!("grault"),
              field!("garply")
            }
        );
        add_fd!(t, vec!["bar"], vec!["baz", "quux"]);
        add_fd!(t, vec!["qux"], vec!["corge", "garply"]);

        let lhs = &t.violating_fd(true, None).unwrap().lhs;
        assert_eq!(*lhs.iter().next().unwrap(), FieldName::from("bar"));
    }

    #[test]
    fn prune_fds() {
        let mut t = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar")
            }
        );
        add_fd!(t, vec!["quux"], vec!["qux"]);
        assert!(t.fds.len() == 2);

        t.prune_fds();

        assert!(t.fds.len() == 1);
    }

    #[test]
    fn minimize_fds() {
        let mut t = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar"),
              field!("baz")
            }
        );
        add_fd!(t, vec!["foo"], vec!["bar", "baz"]);
        add_fd!(t, vec!["bar", "baz"], vec!["foo"]);
        t.minimize_fds();

        let minimized = FD {
            lhs: field_set!["foo"],
            rhs: field_set!["bar", "baz"],
        };
        assert_eq!(t.fds.values().collect::<Vec<_>>(), vec![&minimized]);
    }

    #[test]
    fn table_is_bcnf_no() {
        let mut t = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar"),
              field!("baz")
            }
        );
        add_fd!(t, vec!["foo"], vec!["bar"]);
        add_fd!(t, vec!["bar"], vec!["baz"]);
        assert!(!t.is_bcnf(false, None))
    }

    #[test]
    fn table_key_fields() {
        let t = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar")
            }
        );
        let key_fields = t.key_fields();
        assert!(key_fields.contains("foo"));
        assert!(!key_fields.contains("bar"));
    }

    #[test]
    #[should_panic]
    fn table_set_primary_key_invalid() {
        let mut t = table!(
            "foo",
            fields! {
              field!("foo"),
              field!("bar"),
              field!("baz")
            }
        );
        add_fd!(t, vec!["foo"], vec!["bar"]);

        t.set_primary_key(false);
    }

    #[test]
    fn table_set_primary_key_no_stats() {
        let mut t = table!(
            "foo",
            fields! {
              field!("foo"),
              field!("bar")
            }
        );
        add_fd!(t, vec!["foo"], vec!["bar"]);

        t.set_primary_key(false);

        assert_has_key!(t, field_vec!["foo"])
    }

    #[test]
    fn table_set_primary_key_length() {
        let mut t = table!(
            "foo",
            fields! {
              field!("foo"),
              field!("bar"),
              field!("baz")
            }
        );
        add_fd!(t, vec!["foo", "bar"], vec!["baz"]);
        add_fd!(t, vec!["baz"], vec!["foo", "bar"]);

        t.set_primary_key(true);

        assert_has_key!(t, field_vec!["baz"])
    }

    #[test]
    fn table_set_primary_key_value() {
        let mut t = table!(
            "foo",
            fields! {
              field!("foo", false, 1, usize::MAX),
              field!("bar"),
              field!("baz")
            }
        );
        add_fd!(t, vec!["foo"], vec!["bar", "baz"]);
        add_fd!(t, vec!["baz"], vec!["foo", "bar"]);

        t.set_primary_key(true);

        assert_has_key!(t, field_vec!["baz"])
    }

    #[test]
    fn table_set_primary_key_position() {
        let mut t = table!(
            "foo",
            fields! {
              field!("foo"),
              field!("bar"),
              field!("baz")
            }
        );

        add_fd!(t, vec!["foo"], vec!["bar", "baz"]);
        add_fd!(t, vec!["baz"], vec!["foo", "bar"]);

        t.set_primary_key(true);

        assert_has_key!(t, field_vec!["foo"])
    }

    #[test]
    fn table_is_superkey_yes() {
        let t = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar")
            }
        );
        let key = collect![as HashSet<_>: FieldName::from("foo"), FieldName::from("bar")];
        assert!(t.is_superkey(&key))
    }

    #[test]
    fn table_is_superkey_no() {
        let t = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar")
            }
        );
        let key = collect![as HashSet<_>: FieldName::from("bar")];
        assert!(!t.is_superkey(&key))
    }

    #[test]
    fn table_contains_fd() {
        let mut t1 = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar")
            }
        );
        add_fd!(t1, vec!["foo"], vec!["bar"]);
        let fd = FD {
            lhs: field_set!["foo"],
            rhs: field_set!["bar"],
        };

        assert!(t1.contains_fd(&fd))
    }

    #[test]
    fn table_contains_fd_no() {
        let mut t1 = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar")
            }
        );
        add_fd!(t1, vec!["foo"], vec!["bar"]);
        let fd = FD {
            lhs: field_set!["bar"],
            rhs: field_set!["foo"],
        };

        assert!(!t1.contains_fd(&fd))
    }

    #[test]
    fn table_copy_fds() {
        let mut t1 = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar"),
              field!("baz")
            }
        );
        let mut t2 = table!(
            "foo",
            fields! {
              field!("foo", true),
              field!("bar")
            }
        );
        add_fd!(t1, vec!["foo"], vec!["bar"]);
        add_fd!(t1, vec!["foo"], vec!["baz"]);
        t2.copy_fds(&t1);

        let copied_fd = FD {
            lhs: field_set!["foo"],
            rhs: field_set!["bar"],
        };
        let copied_fds = t2.fds.values().collect::<Vec<_>>();
        assert_eq!(vec![&copied_fd], copied_fds)
    }

    #[test]
    fn schema_add_ind_subset() {
        let t1 = table!(
            "foo",
            fields! {
              field!("bar"),
              field!("qux")
            }
        );
        let t2 = table!(
            "baz",
            fields! {
              field!("quux"),
              field!("corge")
            }
        );
        let mut schema = schema! {t1, t2};
        add_ind!(
            schema,
            "foo",
            vec!["bar", "qux"],
            "baz",
            vec!["quux", "corge"]
        );
        add_ind!(schema, "foo", vec!["bar"], "baz", vec!["quux"]);

        assert!(schema.inds.values().map(|inds| inds.len()).sum::<usize>() == 1usize)
    }

    #[test]
    fn schema_contains_ind_subset() {
        let t1 = table!(
            "foo",
            fields! {
              field!("bar"),
              field!("qux")
            }
        );
        let t2 = table!(
            "baz",
            fields! {
              field!("quux"),
              field!("corge")
            }
        );
        let mut schema = schema! {t1, t2};
        add_ind!(
            schema,
            "foo",
            vec!["bar", "qux"],
            "baz",
            vec!["quux", "corge"]
        );

        let ind = IND {
            left_table: TableName::from("foo"),
            left_fields: field_vec!["bar"],
            right_table: TableName::from("baz"),
            right_fields: field_vec!["quux"],
        };
        assert!(schema.contains_ind(&ind))
    }

    #[test]
    fn schema_contains_ind() {
        let t1 = table!(
            "foo",
            fields! {
              field!("bar", true)
            }
        );
        let t2 = table!(
            "baz",
            fields! {
              field!("quux", true)
            }
        );
        let mut schema = schema! {t1, t2};
        add_ind!(schema, "foo", vec!["bar"], "baz", vec!["quux"]);

        let ind = IND {
            left_table: TableName::from("foo"),
            left_fields: vec![FieldName::from("bar")],
            right_table: TableName::from("baz"),
            right_fields: vec![FieldName::from("quux")],
        };
        assert!(schema.contains_ind(&ind))
    }

    #[test]
    fn schema_copy_inds() {
        let t1 = table!(
            "foo",
            fields! {
              field!("bar", true),
              field!("baz")
            }
        );
        let t2 = table!(
            "quux",
            fields! {
              field!("bar", true),
              field!("baz")
            }
        );
        let t3 = table!(
            "corge",
            fields! {
              field!("grault", true),
              field!("garply")
            }
        );
        let mut schema = schema! {t1, t2, t3};
        add_ind!(
            schema,
            "quux",
            vec!["bar", "baz"],
            "corge",
            vec!["grault", "garply"]
        );

        schema.validate();
        schema.copy_inds(&TableName::from("quux"), &TableName::from("foo"));
        schema.validate();

        let inds = &schema.inds[&(TableName::from("foo"), TableName::from("corge"))];
        assert_eq!(inds.len(), 1);

        let ind = &inds[0];
        assert_eq!(ind.left_fields, field_vec!["bar", "baz"]);
        assert_eq!(ind.right_fields, field_vec!["grault", "garply"]);
    }

    #[test]
    fn schema_copy_inds_partial() {
        let t1 = table!(
            "foo",
            fields! {
              field!("bar", true)
            }
        );
        let t2 = table!(
            "quux",
            fields! {
              field!("bar", true),
              field!("baz")
            }
        );
        let t3 = table!(
            "corge",
            fields! {
              field!("grault", true),
              field!("garply")
            }
        );
        let mut schema = schema! {t1, t2, t3};
        add_ind!(
            schema,
            "quux",
            vec!["bar", "baz"],
            "corge",
            vec!["grault", "garply"]
        );

        schema.validate();
        schema.copy_inds(&TableName::from("quux"), &TableName::from("foo"));
        schema.validate();

        let inds = &schema.inds[&(TableName::from("foo"), TableName::from("corge"))];
        assert_eq!(inds.len(), 1);

        let ind = &inds[0];
        assert_eq!(ind.left_fields, field_vec!["bar"]);
        assert_eq!(ind.right_fields, field_vec!["grault"]);
    }

    #[test]
    fn schema_prune_inds_yes() {
        let t = table!(
            "foo",
            fields! {
              field!("bar", true)
            }
        );
        let mut schema = schema! {t};
        add_ind!(schema, "foo", vec!["bar"], "baz", vec!["quux"]);

        // !schema.validate();
        schema.prune_inds();
        schema.validate();

        assert_eq!(schema.inds.len(), 0)
    }

    #[test]
    fn schema_prune_inds_no() {
        let t1 = table!(
            "foo",
            fields! {
              field!("bar", true)
            }
        );
        let t2 = table!(
            "baz",
            fields! {
              field!("quux", true)
            }
        );
        let mut schema = schema! {t1, t2};
        add_ind!(schema, "foo", vec!["bar"], "baz", vec!["quux"]);

        schema.validate();
        schema.prune_inds();
        schema.validate();

        assert_eq!(schema.inds.len(), 1)
    }

    #[test]
    fn schema_prune_inds_fields() {
        let t1 = table!(
            "foo",
            fields! {
              field!("bar", true)
            }
        );
        let t2 = table!(
            "qux",
            fields! {
              field!("quux", true)
            }
        );

        let mut schema = schema! {t1, t2};
        add_ind!(
            schema,
            "foo",
            vec!["bar", "baz"],
            "qux",
            vec!["quux", "corge"]
        );

        // !schema.validate();
        schema.prune_inds();
        schema.validate();

        let ind = schema.inds.values().next().unwrap().iter().next().unwrap();

        assert_eq!(ind.left_fields.len(), 1);
        assert_eq!(
            ind.left_fields.iter().next().unwrap(),
            &FieldName::from("bar")
        );

        assert_eq!(ind.right_fields.len(), 1);
        assert_eq!(
            ind.right_fields.iter().next().unwrap(),
            &FieldName::from("quux")
        );
    }

    #[test]
    fn schema_prune_inds_fields_one_side() {
        let t1 = table!(
            "foo",
            fields! {
              field!("bar", true)
            }
        );
        let t2 = table!(
            "qux",
            fields! {
              field!("quux", true),
              field!("corge")
            }
        );

        let mut schema = schema! {t1, t2};
        add_ind!(
            schema,
            "foo",
            vec!["bar", "baz"],
            "qux",
            vec!["quux", "corge"]
        );

        // !schema.validate();
        schema.prune_inds();
        schema.validate();

        let ind = schema.inds.values().next().unwrap().iter().next().unwrap();

        assert_eq!(ind.left_fields.len(), 1);
        assert_eq!(
            ind.left_fields.iter().next().unwrap(),
            &FieldName::from("bar")
        );

        assert_eq!(ind.right_fields.len(), 1);
        assert_eq!(
            ind.right_fields.iter().next().unwrap(),
            &FieldName::from("quux")
        );
    }

    #[test]
    fn retain_fk_inds_no() {
        let t1 = table!(
            "foo",
            fields! {
              field!("bar", true),
              field!("qux")
            }
        );
        let t2 = table!(
            "quux",
            fields! {
              field!("corge", true),
              field!("garply")
            }
        );

        let mut schema = schema! {t1, t2};
        add_ind!(schema, "foo", vec!["qux"], "quux", vec!["garply"]);

        schema.retain_fk_inds();

        assert!(schema.inds.values().all(|inds| inds.is_empty()))
    }

    #[test]
    fn retain_fk_inds_yes() {
        let t1 = table!(
            "foo",
            fields! {
              field!("bar", true),
              field!("qux")
            }
        );
        let mut t2 = table!(
            "quux",
            fields! {
              field!("corge", true),
              field!("garply")
            }
        );
        add_fd!(t2, vec!["corge"], vec!["garply"]);

        let mut schema = schema! {t1, t2};
        add_ind!(
            schema,
            "foo",
            vec!["bar", "qux"],
            "quux",
            vec!["corge", "garply"]
        );

        schema.retain_fk_inds();

        assert!(schema.inds.values().all(|inds| inds.is_empty()))
    }
}
