//! SimCityPak 描述符数据库（`database_main.s3db` / `database_user.s3db`）只读访问。
//!
//! 迁移自 C# `SimCityPak\TGIRegistry\`（TGITable 及 TGITables/* 子类）。每个表
//! 均为 `id / name / comments`（FileTypes 另有 viewer 列）；main 库先加载，user
//! 库按 id 覆盖（对齐 C# `TGITable.LoadCache`）。
//!
//! # 用法
//!
//! ```no_run
//! # fn main() -> sc_registry::Result<()> {
//! let registry = sc_registry::Registry::open("data/database_main.s3db")?;
//! let name = registry.property_name(0x0975_695F); // "Model Details"
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{OpenFlags, Connection};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// One descriptor record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub id: u32,
    pub name: String,
    pub comments: String,
}

/// The six descriptor tables of an s3db registry.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    tables: RegistryTables,
}

#[derive(Debug, Clone, Default)]
struct RegistryTables {
    file_types: HashMap<u32, Record>,
    group_types: HashMap<u32, Record>,
    groups: HashMap<u32, Record>,
    instance_types: HashMap<u32, Record>,
    instances: HashMap<u32, Record>,
    properties: HashMap<u32, Record>,
}

impl Registry {
    /// Open one s3db file read-only and load all tables.
    pub fn open(path: impl AsRef<Path>) -> Result<Registry> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let mut tables = RegistryTables::default();
        load_table(&conn, "FileTypes", &mut tables.file_types)?;
        load_table(&conn, "GroupTypes", &mut tables.group_types)?;
        load_table(&conn, "Groups", &mut tables.groups)?;
        load_table(&conn, "InstanceTypes", &mut tables.instance_types)?;
        load_table(&conn, "Instances", &mut tables.instances)?;
        load_table(&conn, "Properties", &mut tables.properties)?;
        Ok(Registry { tables })
    }

    /// Load main + user override (user wins per id), mirroring
    /// `TGITable.LoadCache`.
    pub fn open_with_user(main: impl AsRef<Path>, user: impl AsRef<Path>) -> Result<Registry> {
        let mut registry = Self::open(main)?;
        let user = Self::open(user)?;
        registry.override_with(&user);
        Ok(registry)
    }

    fn override_with(&mut self, other: &Registry) {
        let tables = &mut self.tables;
        let other_tables = &other.tables;
        for (ours, theirs) in [
            (&mut tables.file_types, &other_tables.file_types),
            (&mut tables.group_types, &other_tables.group_types),
            (&mut tables.groups, &other_tables.groups),
            (&mut tables.instance_types, &other_tables.instance_types),
            (&mut tables.instances, &other_tables.instances),
            (&mut tables.properties, &other_tables.properties),
        ] {
            for (id, record) in theirs {
                ours.insert(*id, record.clone());
            }
        }
    }

    pub fn file_types(&self) -> &HashMap<u32, Record> {
        &self.tables.file_types
    }

    pub fn group_types(&self) -> &HashMap<u32, Record> {
        &self.tables.group_types
    }

    pub fn groups(&self) -> &HashMap<u32, Record> {
        &self.tables.groups
    }

    pub fn instance_types(&self) -> &HashMap<u32, Record> {
        &self.tables.instance_types
    }

    pub fn instances(&self) -> &HashMap<u32, Record> {
        &self.tables.instances
    }

    pub fn properties(&self) -> &HashMap<u32, Record> {
        &self.tables.properties
    }

    /// Instance name, hex fallback like C# `TGITable.GetName`.
    pub fn instance_name(&self, id: u32) -> String {
        name_or_hex(&self.tables.instances, id)
    }

    /// Property (descriptor) name, hex fallback.
    pub fn property_name(&self, id: u32) -> String {
        name_or_hex(&self.tables.properties, id)
    }

    /// Type name, hex fallback.
    pub fn type_name(&self, id: u32) -> String {
        name_or_hex(&self.tables.file_types, id)
    }

    /// Group name, hex fallback.
    pub fn group_name(&self, id: u32) -> String {
        name_or_hex(&self.tables.groups, id)
    }
}

fn name_or_hex(table: &HashMap<u32, Record>, id: u32) -> String {
    match table.get(&id) {
        Some(record) if !record.name.is_empty() => record.name.clone(),
        _ => format!("{id:08X}"),
    }
}

fn load_table(conn: &Connection, table: &str, into: &mut HashMap<u32, Record>) -> Result<()> {
    let mut stmt = conn.prepare(&format!("select id, name, comments from {table}"))?;
    let rows = stmt.query_map([], |row| {
        // ids are stored as 64-bit values; mask like the C# reader
        let id: i64 = row.get(0)?;
        let name: String = row.get::<_, Option<String>>(1)?.unwrap_or_default();
        let comments: String = row.get::<_, Option<String>>(2)?.unwrap_or_default();
        Ok(Record {
            id: (id as u64 & 0xFFFF_FFFF) as u32,
            name,
            comments,
        })
    })?;
    for record in rows {
        let record = record?;
        into.insert(record.id, record);
    }
    Ok(())
}
