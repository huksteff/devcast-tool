//! Schema versions. `project.json` carries `schema_version`; older files are upgraded step by step
//! on load, newer ones are refused. Add a migration together with bumping
//! [`dc_core::project::SCHEMA_VERSION`].

use dc_core::project::SCHEMA_VERSION;
use serde_json::Value;

/// Upgrades a project JSON by one version in place.
pub type Migration = fn(&mut Value) -> Result<(), String>;

/// `MIGRATIONS[i]` upgrades version `i + 1` to `i + 2`. Empty while the schema is at version 1.
pub const MIGRATIONS: &[Migration] = &[];

#[derive(Debug, PartialEq, Eq)]
pub enum MigrateError {
    MissingVersion,
    TooNew { found: u32 },
    Failed { from: u32, message: String },
}

/// Brings `value` to the current schema version.
pub fn migrate(value: Value) -> Result<Value, MigrateError> {
    migrate_with(value, MIGRATIONS, SCHEMA_VERSION)
}

fn migrate_with(
    mut value: Value,
    migrations: &[Migration],
    target: u32,
) -> Result<Value, MigrateError> {
    let found = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v >= 1)
        .ok_or(MigrateError::MissingVersion)?;
    if found > target {
        return Err(MigrateError::TooNew { found });
    }
    for version in found..target {
        let step = migrations
            .get(version as usize - 1)
            .ok_or_else(|| MigrateError::Failed {
                from: version,
                message: "нет миграции".to_owned(),
            })?;
        step(&mut value).map_err(|message| MigrateError::Failed {
            from: version,
            message,
        })?;
        if let Some(object) = value.as_object_mut() {
            object.insert("schema_version".to_owned(), Value::from(version + 1));
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_version_has_a_migration() {
        assert_eq!(MIGRATIONS.len(), SCHEMA_VERSION as usize - 1);
    }

    fn rename_title(value: &mut Value) -> Result<(), String> {
        let object = value.as_object_mut().ok_or("не объект")?;
        let title = object.remove("title").ok_or("нет title")?;
        object.insert("name".to_owned(), title);
        Ok(())
    }

    fn add_flag(value: &mut Value) -> Result<(), String> {
        value
            .as_object_mut()
            .ok_or("не объект")?
            .insert("flag".to_owned(), Value::Bool(true));
        Ok(())
    }

    #[test]
    fn runs_the_chain_from_the_found_version() {
        let chain: &[Migration] = &[rename_title, add_flag];
        let old = serde_json::json!({"schema_version": 1, "title": "x"});
        let new = migrate_with(old, chain, 3).unwrap();
        assert_eq!(
            new,
            serde_json::json!({"schema_version": 3, "name": "x", "flag": true})
        );
        let mid = serde_json::json!({"schema_version": 2, "name": "y"});
        assert_eq!(
            migrate_with(mid, chain, 3).unwrap()["flag"],
            Value::Bool(true)
        );
    }

    #[test]
    fn refuses_newer_and_missing_versions() {
        let chain: &[Migration] = &[];
        assert_eq!(
            migrate_with(serde_json::json!({"schema_version": 5}), chain, 1),
            Err(MigrateError::TooNew { found: 5 })
        );
        assert_eq!(
            migrate_with(serde_json::json!({}), chain, 1),
            Err(MigrateError::MissingVersion)
        );
    }

    #[test]
    fn reports_failing_step() {
        let chain: &[Migration] = &[rename_title];
        let error = migrate_with(serde_json::json!({"schema_version": 1}), chain, 2).unwrap_err();
        assert_eq!(
            error,
            MigrateError::Failed {
                from: 1,
                message: "нет title".to_owned()
            }
        );
    }
}
