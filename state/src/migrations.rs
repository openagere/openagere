use std::borrow::Cow;

use sqlx::migrate::Migrator;

pub(crate) static STATE_MIGRATOR: Migrator = sqlx::migrate!("./migrations");
pub(crate) static LOGS_MIGRATOR: Migrator = sqlx::migrate!("./logs_migrations");

/// Allow an older Agere binary to open a database that has already been
/// migrated by a newer binary running in parallel.
///
/// We intentionally ignore applied migration versions that are newer than the
/// embedded migration set. Known migration versions are still validated by
/// checksum, so this only relaxes the "database is ahead of me" case.
fn runtime_migrator(base: &'static Migrator) -> Migrator {
    Migrator {
        migrations: Cow::Borrowed(base.migrations.as_ref()),
        ignore_missing: true,
        locking: base.locking,
        no_tx: base.no_tx,
    }
}

pub(crate) fn runtime_state_migrator() -> Migrator {
    runtime_migrator(&STATE_MIGRATOR)
}

pub(crate) fn runtime_logs_migrator() -> Migrator {
    runtime_migrator(&LOGS_MIGRATOR)
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    fn migration_sql_files_use_lf_line_endings() {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for dir in ["migrations", "logs_migrations"] {
            for entry in fs::read_dir(manifest_dir.join(dir)).expect("read migrations dir") {
                let path = entry.expect("read migration entry").path();
                if path.extension().and_then(|extension| extension.to_str()) != Some("sql") {
                    continue;
                }
                let migration = fs::read_to_string(&path).expect("read migration file");

                assert!(
                    !migration.contains("\r\n"),
                    "sqlx migration checksums are line-ending sensitive: {}",
                    path.display()
                );
            }
        }
    }
}
