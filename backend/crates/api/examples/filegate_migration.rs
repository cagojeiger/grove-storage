//! Offline FileGate 0.4.1 resource import; no provider writes or schema upgrades.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use grove_core::{Crypto, EncryptedSecret, ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::types::Json;
use sqlx::{PgConnection, PgPool};

// Explicit columns preserve old values while letting new columns use their defaults.
const TABLES: &[(&str, &str, &str)] = &[
    (
        "storages",
        "storages",
        "id,kind,force_relay,endpoint,public_endpoint,region,bucket,force_path_style,access_key,secret_key_ciphertext,secret_key_nonce,enc_key_id,capacity_bytes,created_at,updated_at",
    ),
    ("clients", "clients", "id,storage_id,created_at"),
    (
        "client_keys",
        "client_native_keys",
        "key_hash,client_id,created_at",
    ),
    (
        "s3_credentials",
        "client_s3_credentials",
        "access_key_id,client_id,secret_key_ciphertext,secret_key_nonce,enc_key_id,created_at",
    ),
    (
        "files",
        "files",
        "id,client_id,state,declared_size,content_type,declared_md5,etag,part_size,created_at,committed_at,deleted_at",
    ),
    (
        "locations",
        "locations",
        "file_id,storage_id,object_key,created_at",
    ),
    (
        "leases",
        "leases",
        "id,file_id,kind,state,expires_at,secret_hash,uploaded_size,uploaded_md5,upload_id,created_at",
    ),
    (
        "lease_parts",
        "lease_parts",
        "lease_id,part_no,state,uploaded_size,uploaded_md5",
    ),
    (
        "s3_keys",
        "s3_object_keys",
        "client_id,key,file_id,created_at,updated_at",
    ),
    (
        "lease_history",
        "lease_history",
        "at,file_id,storage_id,client_id,kind,size",
    ),
    (
        "usage_snapshot",
        "usage_snapshots",
        "day,storage_id,client_id,active_bytes,active_files,observed_at",
    ),
];
const OWNERS: &[&str] = &["s3_uploads", "native_multipart_completions"];
const LEGACY_CHECKSUMS: &[&str] = &[
    "7dd14e2453c3bae518f8626b4cd4acb2e1ecaff49059b141c176ce02ab815dc1eaaa5cbab32b62a61bc8f36d5302673f",
    "978b8fd04a893edb43b747e63b04137a8ee23bd7f35012118cf6e73f0590fb3fc4c94964aaf3bdc256ceb40c8f32df7b",
    "f33da96e44a7774115271fb4777c6f44a8bc34e9963a201fa8b908063890e60ffcd8264b8267336d7d502aa04ca1c318",
    "a622504b4d5a58c2fd1f4d99ab9332fed481d191336059f678e48ce7432724a833e6b5c76eeed592ee2308cbe7cc08bf",
    "02ec9dd2282ae101ad4eb7d73e8497c96bcdf33d84ba2457f5d409473269431820a685b32064f0f08dd5472dce3cf7fb",
    "59427bed767500cffdabec80fdffca99636c5c684f03290381465c7e1112cebd7c6ff404898505d4b09c4e27512abb82",
];

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    format: String,
    captured_at: String,
    tables: BTreeMap<String, Vec<Value>>,
}

fn database_error(error: sqlx::Error) -> anyhow::Error {
    if let Some(error) = error.as_database_error() {
        // PostgreSQL DETAIL may contain an entire credential row.
        anyhow::anyhow!(
            "database rejected operation: SQLSTATE {}, constraint {}",
            error.code().as_deref().unwrap_or("unknown"),
            error.constraint().unwrap_or("none")
        )
    } else {
        anyhow::anyhow!("database operation failed; transaction outcome may be unknown")
    }
}

async fn pool(variable: &str) -> Result<PgPool> {
    let url = std::env::var(variable).with_context(|| format!("{variable} is required"))?;
    grove_db::connect(&url, 1).await.map_err(database_error)
}

fn crypto(prefix: &str) -> Result<Crypto> {
    let root = std::env::var(format!("{prefix}_ENC_ROOT_SECRET"))
        .with_context(|| format!("{prefix}_ENC_ROOT_SECRET is required"))?;
    let key = std::env::var(format!("{prefix}_ENC_KEY_ID")).unwrap_or_else(|_| "v1".into());
    Ok(Crypto::new(&key, &SecretString::from(root))?)
}

async fn verify_schema(connection: &mut PgConnection, legacy: bool) -> Result<()> {
    let actual: Vec<(i64, bool, Vec<u8>)> = sqlx::query_as(
        "SELECT version,success,checksum FROM public._sqlx_migrations ORDER BY version",
    )
    .fetch_all(&mut *connection)
    .await
    .map_err(database_error)?;
    let expected: Vec<(i64, Vec<u8>)> = if legacy {
        LEGACY_CHECKSUMS
            .iter()
            .enumerate()
            .map(|(index, checksum)| Ok((index as i64 + 1, hex::decode(checksum)?)))
            .collect::<Result<_>>()?
    } else {
        sqlx::migrate!("../db/migrations")
            .iter()
            .map(|migration| (migration.version, migration.checksum.to_vec()))
            .collect()
    };
    ensure!(
        actual.len() == expected.len()
            && actual.iter().zip(&expected).all(
                |((version, success, checksum), (want_version, want_checksum))| *success
                    && version == want_version
                    && checksum == want_checksum
            ),
        "database migration history differs from the supported baseline"
    );
    Ok(())
}

async fn rows(connection: &mut PgConnection, table: &str, columns: &str) -> Result<Vec<Value>> {
    let query = format!(
        "SELECT coalesce(jsonb_agg(to_jsonb(r)), '[]'::jsonb) FROM (SELECT {columns} FROM public.{table}) r"
    );
    let Json(rows) = sqlx::query_scalar::<_, Json<Vec<Value>>>(&query)
        .fetch_one(&mut *connection)
        .await
        .map_err(database_error)?;
    Ok(rows)
}

impl Snapshot {
    fn table(&self, name: &str) -> Result<&[Value]> {
        self.tables
            .get(name)
            .map(Vec::as_slice)
            .with_context(|| format!("snapshot table missing: {name}"))
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.format == "filegate-0.4.1-v1",
            "unsupported snapshot format"
        );
        ensure!(
            self.tables.len() == TABLES.len() + OWNERS.len(),
            "snapshot table set differs"
        );
        for table in TABLES
            .iter()
            .map(|(source, _, _)| *source)
            .chain(OWNERS.iter().copied())
        {
            ensure!(
                self.tables.contains_key(table),
                "snapshot table missing: {table}"
            );
        }
        for table in OWNERS {
            ensure!(
                self.table(table)?.is_empty(),
                "upload recovery owners remain; drain FileGate uploads first"
            );
        }
        ensure!(
            self.table("storages")?
                .iter()
                .all(|row| row["kind"] == "s3"),
            "FS storage cannot be imported"
        );
        ensure!(
            self.table("files")?
                .iter()
                .all(|row| row["state"] != "pending"),
            "pending files remain; drain FileGate first"
        );
        ensure!(
            self.table("leases")?
                .iter()
                .all(|row| row["state"] != "issued"),
            "issued leases remain; drain FileGate first"
        );
        let files: BTreeMap<&str, &Value> = self
            .table("files")?
            .iter()
            .map(|row| Ok((row["id"].as_str().context("file ID missing")?, row)))
            .collect::<Result<_>>()?;
        let mut locations = BTreeMap::new();
        for row in self.table("locations")? {
            let id = row["file_id"]
                .as_str()
                .context("location file ID missing")?;
            ensure!(
                files.get(id).is_some_and(|file| file["state"] == "active"),
                "cleanup locations remain; drain FileGate first"
            );
            ensure!(
                locations.insert(id, row).is_none(),
                "duplicate file location"
            );
        }
        ensure!(
            files.values().all(|file| file["state"] != "active"
                || locations.contains_key(file["id"].as_str().unwrap_or(""))),
            "active file has no physical location"
        );
        Ok(())
    }

    fn report(&self) {
        println!("Snapshot time: {}", self.captured_at);
        for (table, values) in &self.tables {
            println!("{table}: {} rows", values.len());
        }
    }
}

async fn export(pool: &PgPool) -> Result<Snapshot> {
    let mut transaction = pool.begin().await.map_err(database_error)?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *transaction)
        .await
        .map_err(database_error)?;
    sqlx::query("SET LOCAL TIME ZONE 'UTC'")
        .execute(&mut *transaction)
        .await
        .map_err(database_error)?;
    verify_schema(&mut transaction, true).await?;
    let captured_at: String = sqlx::query_scalar("SELECT transaction_timestamp()::text")
        .fetch_one(&mut *transaction)
        .await
        .map_err(database_error)?;
    let mut tables = BTreeMap::new();
    for (source, _, columns) in TABLES {
        let columns = if *source == "usage_snapshot" {
            "day,storage_id,client_id,active_bytes,active_files,NULL::timestamptz AS observed_at"
        } else {
            columns
        };
        tables.insert(
            (*source).into(),
            rows(&mut transaction, source, columns).await?,
        );
    }
    for table in OWNERS {
        tables.insert(
            (*table).into(),
            rows(&mut transaction, table, "file_id").await?,
        );
    }
    let snapshot = Snapshot {
        format: "filegate-0.4.1-v1".into(),
        captured_at,
        tables,
    };
    snapshot.validate()?;
    transaction.commit().await.map_err(database_error)?;
    Ok(snapshot)
}

fn bytea(row: &Value, column: &str) -> Result<Vec<u8>> {
    let encoded = row[column]
        .as_str()
        .and_then(|value| value.strip_prefix("\\x"))
        .with_context(|| format!("invalid encrypted field: {column}"))?;
    hex::decode(encoded).with_context(|| format!("invalid encrypted field: {column}"))
}

fn decrypt(crypto: &Crypto, row: &Value, identity: &str) -> Result<SecretString> {
    let key = row["enc_key_id"]
        .as_str()
        .context("encryption key ID missing")?;
    let aad = row[identity]
        .as_str()
        .context("credential identity missing")?;
    Ok(crypto.decrypt(
        key,
        aad,
        &EncryptedSecret {
            ciphertext: bytea(row, "secret_key_ciphertext")?,
            nonce: bytea(row, "secret_key_nonce")?,
        },
    )?)
}

fn reencrypt(values: &mut [Value], source: &Crypto, target: &Crypto, identity: &str) -> Result<()> {
    for row in values {
        let plaintext = decrypt(source, row, identity)?;
        let encrypted = target.encrypt(
            row[identity]
                .as_str()
                .context("credential identity missing")?,
            &plaintext,
        )?;
        row["secret_key_ciphertext"] = json!(format!("\\x{}", hex::encode(encrypted.ciphertext)));
        row["secret_key_nonce"] = json!(format!("\\x{}", hex::encode(encrypted.nonce)));
        row["enc_key_id"] = json!(target.active_key_id());
    }
    Ok(())
}

fn comparable(
    values: &[Value],
    columns: &str,
    crypto: &Crypto,
    identity: Option<&str>,
) -> Result<Vec<String>> {
    let mut result = Vec::with_capacity(values.len());
    for row in values {
        let mut value = serde_json::Map::new();
        for column in columns.split(',') {
            value.insert(
                column.into(),
                row.get(column).context("snapshot column missing")?.clone(),
            );
        }
        if let Some(identity) = identity {
            let plaintext = decrypt(crypto, row, identity)?;
            for column in ["secret_key_ciphertext", "secret_key_nonce", "enc_key_id"] {
                value.remove(column);
            }
            value.insert(
                "secret_fingerprint".into(),
                json!(hex::encode(Sha256::digest(
                    plaintext.expose_secret().as_bytes()
                ))),
            );
        }
        result.push(serde_json::to_string(&value)?);
    }
    result.sort();
    Ok(result)
}

async fn verify(
    connection: &mut PgConnection,
    snapshot: &Snapshot,
    source: &Crypto,
    target: &Crypto,
) -> Result<()> {
    verify_schema(connection, false).await?;
    for (old, new, columns) in TABLES {
        let identity = match *old {
            "storages" => Some("id"),
            "s3_credentials" => Some("access_key_id"),
            _ => None,
        };
        let actual = rows(connection, new, columns).await?;
        ensure!(
            comparable(snapshot.table(old)?, columns, source, identity)?
                == comparable(&actual, columns, target, identity)?,
            "imported rows differ: {new}"
        );
    }
    let uploads: i64 = sqlx::query_scalar("SELECT count(*) FROM public.uploads")
        .fetch_one(&mut *connection)
        .await
        .map_err(database_error)?;
    ensure!(uploads == 0, "target has upload owners");
    Ok(())
}

async fn import(
    pool: &PgPool,
    snapshot: &Snapshot,
    source: &Crypto,
    target: &Crypto,
) -> Result<()> {
    snapshot.validate()?;
    let mut transaction = pool.begin().await.map_err(database_error)?;
    sqlx::query("SET LOCAL TIME ZONE 'UTC'")
        .execute(&mut *transaction)
        .await
        .map_err(database_error)?;
    sqlx::query("SET LOCAL lock_timeout = '5s'")
        .execute(&mut *transaction)
        .await
        .map_err(database_error)?;
    let tables = TABLES
        .iter()
        .map(|(_, table, _)| format!("public.{table}"))
        .chain(std::iter::once("public.uploads".into()))
        .collect::<Vec<_>>()
        .join(",");
    sqlx::query(&format!("LOCK TABLE {tables} IN ACCESS EXCLUSIVE MODE"))
        .execute(&mut *transaction)
        .await
        .map_err(database_error)?;
    let writers: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid()")
        .fetch_one(&mut *transaction).await.map_err(database_error)?;
    ensure!(
        writers == 0,
        "target database has other connections; stop Grove before import"
    );
    verify_schema(&mut transaction, false).await?;
    for (_, table, _) in TABLES {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM public.{table}"))
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error)?;
        ensure!(count == 0, "target resource table is not empty: {table}");
    }
    let uploads: i64 = sqlx::query_scalar("SELECT count(*) FROM public.uploads")
        .fetch_one(&mut *transaction)
        .await
        .map_err(database_error)?;
    ensure!(uploads == 0, "target uploads table is not empty");
    for (old, new, columns) in TABLES {
        let mut values = snapshot.table(old)?.to_vec();
        match *old {
            "storages" => reencrypt(&mut values, source, target, "id")?,
            "s3_credentials" => reencrypt(&mut values, source, target, "access_key_id")?,
            _ => {}
        }
        let query = format!(
            "INSERT INTO public.{new}({columns}) SELECT {columns} FROM jsonb_populate_recordset(NULL::public.{new}, $1) r"
        );
        sqlx::query(&query)
            .bind(Json(values))
            .execute(&mut *transaction)
            .await
            .map_err(database_error)?;
    }
    verify(&mut transaction, snapshot, source, target).await?;
    transaction.commit().await.map_err(database_error)?;
    Ok(())
}

fn save(snapshot: &Snapshot, path: &Path) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .context("cannot create new private snapshot file")?;
    file.write_all(&serde_json::to_vec(snapshot)?)
        .context("snapshot write failed")?;
    file.sync_all().context("snapshot sync failed")?;
    Ok(())
}

async fn run() -> Result<()> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [command, path] = arguments.as_slice() else {
        bail!("usage: filegate_migration export|import|verify <snapshot.json>");
    };
    if command == "export" {
        let pool = pool("FILEGATE_DATABASE_URL").await?;
        let snapshot = export(&pool).await?;
        save(&snapshot, Path::new(path))?;
        snapshot.report();
    } else {
        ensure!(
            command == "import" || command == "verify",
            "unknown migration command"
        );
        let snapshot: Snapshot =
            serde_json::from_slice(&std::fs::read(path).context("cannot read snapshot")?)
                .map_err(|_| anyhow::anyhow!("snapshot is not valid migration JSON"))?;
        snapshot.validate()?;
        let source = crypto("FILEGATE")?;
        let target = crypto("GROVE")?;
        let pool = pool("GROVE_DATABASE_URL").await?;
        if command == "import" {
            import(&pool, &snapshot, &source, &target).await?;
        } else {
            let mut transaction = pool.begin().await.map_err(database_error)?;
            sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
                .execute(&mut *transaction)
                .await
                .map_err(database_error)?;
            sqlx::query("SET LOCAL TIME ZONE 'UTC'")
                .execute(&mut *transaction)
                .await
                .map_err(database_error)?;
            verify(&mut transaction, &snapshot, &source, &target).await?;
            transaction.commit().await.map_err(database_error)?;
        }
        snapshot.report();
        println!("PASS {command}: resource rows and decrypted credentials match");
    }
    Ok(())
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("filegate_migration: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
#[path = "filegate_migration/tests.rs"]
mod tests;
