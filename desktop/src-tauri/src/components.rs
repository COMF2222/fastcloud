//! Reuse CLAP files by content hash, then activate a complete verified set.
//! The manifest is embedded in the signed executable rather than trusted online.
use anyhow::{Context, Result, ensure};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::io::AsyncWriteExt;

const EMBEDDED: &str = include_str!(concat!(env!("OUT_DIR"), "/component-manifest.json"));
const RELEASES: &str = "https://github.com/COMF2222/fastcloud/releases/download";
#[cfg(windows)]
pub const WORKER_PATH: &str = "worker-lite/fastcloud-clap/fastcloud-clap.exe";
#[cfg(not(windows))]
pub const WORKER_PATH: &str = "worker-lite/fastcloud-clap/fastcloud-clap";
const REQUIRED: &[&str] = &[
    WORKER_PATH,
    "model/config.json",
    "model/tokenizer.json",
    "model/onnx/audio_model_quantized.onnx",
    "model/onnx/text_model_quantized.onnx",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FileEntry {
    path: String,
    sha256: String,
    size: u64,
}
#[derive(Clone, Deserialize)]
struct Manifest {
    schema: u32,
    version: String,
    tag: String,
    id: String,
    files: Vec<FileEntry>,
}

fn hash_name(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|ch| ch.is_ascii_digit() || (b'a'..=b'f').contains(&ch))
}

fn safe_path(path: &str) -> bool {
    path.len() <= 240
        && !path.contains(['\\', ':', '*', '?', '"', '<', '>', '|'])
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with(['.', ' '])
                && !part.chars().any(char::is_control)
                && !matches!(
                    part.split('.')
                        .next()
                        .unwrap_or("")
                        .to_ascii_uppercase()
                        .as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                )
        })
        && matches!(
            path.split('/').next(),
            Some("model" | "worker-lite" | "licenses")
        )
}

impl Manifest {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.schema == 1 && hash_name(&self.id),
            "Invalid component manifest"
        );
        ensure!(
            !self.version.is_empty()
                && self.version.len() <= 40
                && self
                    .version
                    .bytes()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, b'.' | b'-')),
            "Invalid component release"
        );
        ensure!(
            !self.files.is_empty() && self.files.len() <= 2000,
            "Invalid component file count"
        );
        let canonical = format!("v{}", self.version);
        let compact = self
            .version
            .rsplit_once('-')
            .filter(|(_, suffix)| suffix.len() == 1 && suffix.as_bytes()[0].is_ascii_lowercase())
            .map_or_else(
                || canonical.clone(),
                |(base, suffix)| format!("v{base}{suffix}"),
            );
        ensure!(
            self.tag == canonical || self.tag == compact,
            "Component tag differs from version"
        );
        let mut paths = HashSet::new();
        let mut total = 0_u64;
        for entry in &self.files {
            ensure!(
                hash_name(&entry.sha256) && entry.size <= 256 * 1024 * 1024,
                "Invalid component checksum or size"
            );
            ensure!(
                safe_path(&entry.path) && paths.insert(entry.path.to_ascii_lowercase()),
                "Unsafe component path"
            );
            total = total
                .checked_add(entry.size)
                .context("Component size overflow")?;
        }
        ensure!(total <= 2 * 1024 * 1024 * 1024, "Components are too large");
        ensure!(
            format!("{:x}", Sha256::digest(serde_json::to_vec(&self.files)?)) == self.id,
            "Component manifest checksum mismatch"
        );
        for required in REQUIRED {
            ensure!(
                self.files
                    .iter()
                    .any(|entry| entry.path == *required && entry.size > 0),
                "Incomplete CLAP component manifest"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: &'static str,
    pub completed_files: usize,
    pub total_files: usize,
    pub downloaded_bytes: u64,
    pub error: Option<String>,
}

pub struct Manager {
    manifest: Option<Manifest>,
    root: PathBuf,
    bundled: PathBuf,
    status: Mutex<Status>,
    active: Mutex<Option<PathBuf>>,
    gate: tokio::sync::Mutex<()>,
}

impl Manager {
    pub fn new(root: PathBuf, bundled: PathBuf) -> Result<Arc<Self>> {
        let manifest: Option<Manifest> = serde_json::from_str(EMBEDDED)?;
        if let Some(manifest) = &manifest {
            manifest.validate()?;
            ensure!(
                manifest.version == env!("CARGO_PKG_VERSION"),
                "Component release differs from application"
            );
        }
        Ok(Self::with_manifest(root, bundled, manifest))
    }

    fn with_manifest(root: PathBuf, bundled: PathBuf, manifest: Option<Manifest>) -> Arc<Self> {
        let count = manifest.as_ref().map_or(0, |value| value.files.len());
        Arc::new(Self {
            manifest,
            root,
            bundled,
            status: Mutex::new(Status {
                state: if count == 0 {
                    "unavailable"
                } else {
                    "checking"
                },
                completed_files: 0,
                total_files: count,
                downloaded_bytes: 0,
                error: None,
            }),
            active: Mutex::new(None),
            gate: tokio::sync::Mutex::new(()),
        })
    }

    pub fn managed(&self) -> bool {
        self.manifest.is_some()
    }
    pub fn status(&self) -> Status {
        self.status.lock().clone()
    }
    pub fn active(&self) -> Option<PathBuf> {
        self.active.lock().clone()
    }

    pub async fn prepare(self: &Arc<Self>) -> Result<()> {
        let client = reqwest::Client::builder()
            .https_only(true)
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(900))
            .user_agent("Fastcloud components")
            .build()?;
        self.prepare_from(&client, RELEASES).await
    }

    pub async fn preserve_before_update(self: &Arc<Self>) -> Result<()> {
        // A light install has no bundled files to preserve. Its optional model
        // download must not delay an unrelated update of the client itself.
        if self.manifest.is_none() || !self.bundled.join(REQUIRED[0]).is_file() {
            return Ok(());
        }
        self.prepare().await
    }

    async fn prepare_from(
        self: &Arc<Self>,
        client: &reqwest::Client,
        releases: &str,
    ) -> Result<()> {
        let Some(manifest) = &self.manifest else {
            return Ok(());
        };
        let _guard = self.gate.lock().await;
        if self.active.lock().is_some() {
            return Ok(());
        }
        {
            let mut status = self.status.lock();
            status.state = "checking";
            status.completed_files = 0;
            status.downloaded_bytes = 0;
            status.error = None;
        }
        match self.prepare_set(manifest, client, releases).await {
            Ok(path) => {
                *self.active.lock() = Some(path);
                self.status.lock().state = "ready";
                #[cfg(not(debug_assertions))]
                {
                    // A verified managed set no longer needs duplicate installer
                    // files. Development builds keep their source resources intact.
                    let bundled = self.bundled.clone();
                    let files = manifest.files.clone();
                    match tokio::task::spawn_blocking(move || cleanup_bundled(&bundled, &files))
                        .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => {
                            log::warn!("Could not remove migrated CLAP files: {error:#}")
                        }
                        Err(error) => log::warn!("CLAP migration cleanup task: {error}"),
                    }
                }
                Ok(())
            }
            Err(error) => {
                let mut status = self.status.lock();
                status.state = "error";
                status.error = Some(format!("{error:#}"));
                Err(error)
            }
        }
    }

    async fn prepare_set(
        self: &Arc<Self>,
        manifest: &Manifest,
        client: &reqwest::Client,
        releases: &str,
    ) -> Result<PathBuf> {
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            safe_dir(&root)?;
            safe_dir(&root.join("blobs"))?;
            safe_dir(&root.join("sets"))?;
            Ok(())
        })
        .await??;
        // Identical files at different paths must share one download and one writer.
        let mut seen = HashSet::new();
        let unique: Vec<_> = manifest
            .files
            .iter()
            .filter(|entry| seen.insert(&entry.sha256))
            .collect();
        for batch in unique.chunks(4) {
            let mut tasks = tokio::task::JoinSet::new();
            for entry in batch {
                let manager = self.clone();
                let client = client.clone();
                let entry = (**entry).clone();
                let count = manifest
                    .files
                    .iter()
                    .filter(|file| file.sha256 == entry.sha256)
                    .count();
                let url = format!("{releases}/{}/clap-{}.gz", manifest.tag, entry.sha256);
                tasks.spawn(async move {
                    manager
                        .ensure_file(&entry, &client, &url)
                        .await
                        .map(|()| count)
                });
            }
            // Drain all transfers before failing, so a retry cannot race old writers.
            let mut failure = None;
            while let Some(result) = tasks.join_next().await {
                match result {
                    Ok(Ok(count)) => self.status.lock().completed_files += count,
                    Ok(Err(error)) => {
                        failure.get_or_insert(error);
                    }
                    Err(error) => {
                        failure.get_or_insert(anyhow::Error::from(error));
                    }
                }
            }
            if let Some(error) = failure {
                return Err(error);
            }
        }
        let root = self.root.clone();
        let manifest = manifest.clone();
        tokio::task::spawn_blocking(move || {
            let path = materialize(&root, &manifest)?;
            if let Err(error) = prune(&root, &manifest.id) {
                log::warn!("Could not remove obsolete CLAP components: {error:#}");
            }
            Ok(path)
        })
        .await?
    }

    async fn ensure_file(
        self: &Arc<Self>,
        entry: &FileEntry,
        client: &reqwest::Client,
        url: &str,
    ) -> Result<()> {
        let root = self.root.clone();
        let bundled = self.bundled.clone();
        let owned = entry.clone();
        if tokio::task::spawn_blocking(move || reuse_file(&root, &bundled, &owned)).await?? {
            return Ok(());
        }
        self.status.lock().state = "downloading";
        let compressed = self
            .root
            .join("blobs")
            .join(format!("{}.download", entry.sha256));
        for attempt in 0..3 {
            let result = async {
                let mut response = client.get(url).send().await?.error_for_status()?;
                let mut file = tokio::fs::File::create(&compressed).await?;
                let mut downloaded = 0_u64;
                while let Some(chunk) = response.chunk().await? {
                    downloaded += chunk.len() as u64;
                    ensure!(
                        downloaded <= entry.size + 65536,
                        "Component download is larger than expected"
                    );
                    file.write_all(&chunk).await?;
                    self.status.lock().downloaded_bytes += chunk.len() as u64;
                }
                file.flush().await?;
                drop(file);
                let root = self.root.clone();
                let entry = entry.clone();
                let path = compressed.clone();
                tokio::task::spawn_blocking(move || unpack(&root, &path, &entry)).await??;
                Ok::<_, anyhow::Error>(())
            }
            .await;
            let _ = tokio::fs::remove_file(&compressed).await;
            let retry = result
                .as_ref()
                .err()
                .and_then(|error| error.downcast_ref::<reqwest::Error>())
                .is_some_and(|error| {
                    error.is_connect()
                        || error.is_timeout()
                        || error.is_body()
                        || error.is_request()
                        || error.status().is_some_and(|status| {
                            status.is_server_error() || status.as_u16() == 429
                        })
                });
            if result.is_ok() || !retry || attempt == 2 {
                return result.with_context(|| format!("Could not prepare {}", entry.path));
            }
            tokio::time::sleep(Duration::from_millis(500 * (attempt + 1))).await;
        }
        anyhow::bail!("Component download attempts exhausted")
    }
}

fn safe_dir(path: &Path) -> Result<()> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "Invalid component directory"
        );
    } else {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

fn valid_file(path: &Path, entry: &FileEntry) -> Result<bool> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(false);
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != entry.size {
        return Ok(false);
    }
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 65536];
    loop {
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        digest.update(&buffer[..size]);
    }
    Ok(format!("{:x}", digest.finalize()) == entry.sha256)
}

fn reuse_file(root: &Path, bundled: &Path, entry: &FileEntry) -> Result<bool> {
    let blob = root.join("blobs").join(&entry.sha256);
    if valid_file(&blob, entry)? {
        return Ok(true);
    }
    if blob.exists() {
        fs::remove_file(&blob)?;
    }
    let source = bundled.join(&entry.path);
    if !valid_file(&source, entry)? {
        return Ok(false);
    }
    // An installer may overwrite its resources; do not hardlink from its directory.
    let temporary = blob.with_extension("copy");
    fs::copy(source, &temporary)?;
    ensure!(
        valid_file(&temporary, entry)?,
        "Bundled component changed during migration"
    );
    fs::rename(temporary, blob)?;
    Ok(true)
}

fn unpack(root: &Path, compressed: &Path, entry: &FileEntry) -> Result<()> {
    let blob = root.join("blobs").join(&entry.sha256);
    let temporary = blob.with_extension("part");
    let result = (|| -> Result<()> {
        let decoder = flate2::read::GzDecoder::new(fs::File::open(compressed)?);
        let mut limited = decoder.take(entry.size + 1);
        let mut output = fs::File::create(&temporary)?;
        let size = std::io::copy(&mut limited, &mut output)?;
        output.flush()?;
        output.sync_all()?;
        drop(output);
        ensure!(
            size == entry.size && valid_file(&temporary, entry)?,
            "Component checksum mismatch"
        );
        fs::rename(&temporary, &blob)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn make_worker_executable(root: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let worker = root.join(WORKER_PATH);
        let permissions = fs::metadata(&worker)?.permissions();
        if permissions.mode() & 0o111 != 0o111 {
            fs::set_permissions(worker, fs::Permissions::from_mode(0o755))?;
        }
    }
    #[cfg(not(unix))]
    let _ = root;
    Ok(())
}

fn materialize(root: &Path, manifest: &Manifest) -> Result<PathBuf> {
    let sets = root.join("sets");
    let target = sets.join(&manifest.id);
    if target.is_dir()
        && !fs::symlink_metadata(&target)?.file_type().is_symlink()
        && manifest
            .files
            .iter()
            .all(|entry| valid_file(&target.join(&entry.path), entry).unwrap_or(false))
    {
        make_worker_executable(&target)?;
        return Ok(target);
    }
    let staging = sets.join(format!("{}.staging", manifest.id));
    // Directory names come only from the validated manifest digest.
    if staging.exists() {
        ensure!(
            !fs::symlink_metadata(&staging)?.file_type().is_symlink(),
            "Invalid staging directory"
        );
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir(&staging)?;
    for entry in &manifest.files {
        let destination = staging.join(&entry.path);
        fs::create_dir_all(destination.parent().context("Component parent directory")?)?;
        let blob = root.join("blobs").join(&entry.sha256);
        if fs::hard_link(&blob, &destination).is_err() {
            fs::copy(&blob, &destination)?;
        }
    }
    make_worker_executable(&staging)?;
    fs::write(
        staging.join("manifest.json"),
        serde_json::to_vec(&manifest.files)?,
    )?;
    if target.exists() {
        ensure!(
            !fs::symlink_metadata(&target)?.file_type().is_symlink(),
            "Invalid component set"
        );
        fs::remove_dir_all(&target)?;
    }
    fs::rename(&staging, &target)?;
    Ok(target)
}

fn prune(root: &Path, current: &str) -> Result<()> {
    let mut sets = Vec::new();
    for entry in fs::read_dir(root.join("sets"))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !hash_name(&name) || !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        let file = path.join("manifest.json");
        if !file.is_file() || fs::metadata(&file)?.len() > 512 * 1024 {
            continue;
        }
        let files: Vec<FileEntry> = serde_json::from_slice(&fs::read(&file)?)?;
        if files.len() > 2000
            || files
                .iter()
                .any(|entry| !safe_path(&entry.path) || !hash_name(&entry.sha256))
            || format!("{:x}", Sha256::digest(serde_json::to_vec(&files)?)) != name
        {
            continue;
        }
        sets.push((name, path, entry.metadata()?.modified()?, files));
    }
    ensure!(
        sets.iter().any(|(id, _, _, _)| id == current),
        "Current component set is missing"
    );
    sets.sort_by(|a, b| b.2.cmp(&a.2));
    let previous = sets
        .iter()
        .find(|(id, _, _, _)| id != current)
        .map(|item| item.0.clone());
    let mut live = HashSet::new();
    for (id, path, _, files) in sets {
        if id == current || previous.as_deref() == Some(id.as_str()) {
            live.extend(files.into_iter().map(|entry| entry.sha256));
        } else {
            // Only directories with a verified manifest and digest name belong to us.
            fs::remove_dir_all(path)?;
        }
    }
    for entry in fs::read_dir(root.join("blobs"))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_file() && hash_name(&name) && !live.contains(&name) {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn cleanup_bundled(bundled: &Path, files: &[FileEntry]) -> Result<()> {
    if !bundled.is_dir() || fs::symlink_metadata(bundled)?.file_type().is_symlink() {
        return Ok(());
    }
    let mut directories = HashSet::new();
    for entry in files {
        ensure!(safe_path(&entry.path), "Unsafe migration path");
        let path = bundled.join(&entry.path);
        let mut parent = path.parent();
        let mut parents = Vec::new();
        let mut linked = false;
        while let Some(folder) = parent {
            if folder == bundled {
                break;
            }
            ensure!(
                folder.starts_with(bundled),
                "Migration path escaped installation"
            );
            if let Ok(metadata) = fs::symlink_metadata(folder)
                && metadata.file_type().is_symlink()
            {
                linked = true;
                break;
            }
            parents.push(folder.to_path_buf());
            parent = folder.parent();
        }
        if !linked && valid_file(&path, entry)? {
            fs::remove_file(path)?;
            directories.extend(parents);
        }
    }
    let mut directories: Vec<_> = directories.into_iter().collect();
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        let _ = fs::remove_dir(directory);
    }
    let _ = fs::remove_dir(bundled);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::HashMap,
        net::TcpListener,
        sync::atomic::{AtomicBool, Ordering},
    };

    fn manifest(version: &str) -> (Manifest, HashMap<String, Vec<u8>>) {
        let mut contents = HashMap::new();
        let mut files = Vec::new();
        for (index, path) in REQUIRED.iter().enumerate() {
            let bytes = format!("Synthetic CLAP resource {index}").into_bytes();
            files.push(FileEntry {
                path: (*path).into(),
                size: bytes.len() as u64,
                sha256: format!("{:x}", Sha256::digest(&bytes)),
            });
            contents.insert((*path).to_owned(), bytes);
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let id = format!("{:x}", Sha256::digest(serde_json::to_vec(&files).unwrap()));
        let manifest = Manifest {
            schema: 1,
            version: version.into(),
            tag: format!("v{version}"),
            id,
            files,
        };
        manifest.validate().unwrap();
        (manifest, contents)
    }

    fn bundle(root: &Path, contents: &HashMap<String, Vec<u8>>) {
        for (path, bytes) in contents {
            let target = root.join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, bytes).unwrap();
        }
    }

    fn change(manifest: &mut Manifest, path: &str, bytes: &[u8]) {
        let entry = manifest
            .files
            .iter_mut()
            .find(|entry| entry.path == path)
            .unwrap();
        entry.sha256 = format!("{:x}", Sha256::digest(bytes));
        entry.size = bytes.len() as u64;
        manifest.id = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&manifest.files).unwrap())
        );
        manifest.validate().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn native_worker_permissions_are_restored_after_download_and_cache_reuse() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let worker = root.path().join(WORKER_PATH);
        fs::create_dir_all(worker.parent().unwrap()).unwrap();
        fs::write(&worker, b"native worker").unwrap();
        fs::set_permissions(&worker, fs::Permissions::from_mode(0o644)).unwrap();
        make_worker_executable(root.path()).unwrap();
        assert_eq!(fs::metadata(&worker).unwrap().permissions().mode() & 0o111, 0o111);
        fs::set_permissions(&worker, fs::Permissions::from_mode(0o644)).unwrap();
        make_worker_executable(root.path()).unwrap();
        assert_eq!(fs::metadata(&worker).unwrap().permissions().mode() & 0o111, 0o111);
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    struct Server {
        base: String,
        bodies: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        requests: Arc<Mutex<Vec<String>>>,
        failures: Arc<Mutex<HashMap<String, usize>>>,
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Server {
        fn new() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            listener.set_nonblocking(true).unwrap();
            let bodies = Arc::new(Mutex::new(HashMap::<String, Vec<u8>>::new()));
            let requests = Arc::new(Mutex::new(Vec::new()));
            let stop = Arc::new(AtomicBool::new(false));
            let failures = Arc::new(Mutex::new(HashMap::<String, usize>::new()));
            let fail_responses = failures.clone();
            let served = bodies.clone();
            let recorded = requests.clone();
            let stopped = stop.clone();
            let thread = std::thread::spawn(move || {
                while !stopped.load(Ordering::Relaxed) {
                    let Ok((mut stream, _)) = listener.accept() else {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    };
                    // Windows accepted sockets can inherit the listener's
                    // nonblocking mode. The fixture reads complete requests.
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(10)))
                        .unwrap();
                    let mut header = Vec::new();
                    while header.len() < 8192
                        && !header.windows(4).any(|value| value == b"\r\n\r\n")
                    {
                        let mut chunk = [0; 1024];
                        match stream.read(&mut chunk) {
                            Ok(0) | Err(_) => break,
                            Ok(size) => header.extend_from_slice(&chunk[..size]),
                        }
                    }
                    if !header.windows(4).any(|value| value == b"\r\n\r\n") {
                        continue;
                    }
                    let request = String::from_utf8_lossy(&header);
                    let Some(path) = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                    else {
                        continue;
                    };
                    let path = path.to_owned();
                    recorded.lock().push(path.clone());
                    let body = served.lock().get(&path).cloned();
                    let unavailable =
                        fail_responses
                            .lock()
                            .get_mut(&path)
                            .is_some_and(|remaining| {
                                if *remaining == 0 {
                                    false
                                } else {
                                    *remaining -= 1;
                                    true
                                }
                            });
                    let status = if unavailable {
                        "503 Service Unavailable"
                    } else if body.is_some() {
                        "200 OK"
                    } else {
                        "404 Not Found"
                    };
                    let body = body.unwrap_or_default();
                    if write!(
                        stream,
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .is_err()
                    {
                        continue;
                    }
                    let _ = stream.write_all(&body);
                }
            });
            Self {
                base,
                bodies,
                requests,
                failures,
                stop,
                thread: Some(thread),
            }
        }

        fn put(&self, version: &str, hash: &str, bytes: Vec<u8>) {
            self.bodies
                .lock()
                .insert(format!("/v{version}/clap-{hash}.gz"), bytes);
        }
    }

    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(thread) = self.thread.take() {
                thread.join().unwrap();
            }
        }
    }

    #[test]
    fn embedded_manifest_matches_application() {
        let manifest: Option<Manifest> = serde_json::from_str(EMBEDDED).unwrap();
        if let Some(manifest) = manifest {
            manifest.validate().unwrap();
            assert_eq!(manifest.version, env!("CARGO_PKG_VERSION"));
        }
    }

    #[test]
    fn manifest_rejects_escape_paths_devices_duplicates_and_tampering() {
        for path in [
            "../outside.exe",
            "model/../../outside",
            "C:/evil.exe",
            "model\\evil.exe",
            "model/CON.txt",
            "model/file.",
            "model//file",
            "model/evil:stream",
        ] {
            let (mut manifest, _) = manifest("0.2.8");
            manifest.files[0].path = path.into();
            assert!(manifest.validate().is_err(), "{path}");
        }
        let (mut manifest, _) = manifest("0.2.8");
        manifest.files.push(manifest.files[0].clone());
        assert!(manifest.validate().is_err());
        let (mut manifest, _) = super::tests::manifest("0.2.8");
        manifest.files[0].size += 1;
        assert!(manifest.validate().is_err());
    }

    #[tokio::test]
    async fn bundled_resources_are_migrated_without_network_and_reused_after_reinstall() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("managed");
        let bundled = directory.path().join("installation");
        let (manifest, contents) = manifest("0.2.8");
        bundle(&bundled, &contents);
        let manager = Manager::with_manifest(root.clone(), bundled.clone(), Some(manifest.clone()));
        let client = reqwest::Client::new();
        manager
            .prepare_from(&client, "http://127.0.0.1:9")
            .await
            .unwrap();
        assert_eq!(manager.status().state, "ready");
        assert_eq!(manager.status().downloaded_bytes, 0);
        // An installer changes/removes its files without corrupting managed files.
        fs::write(
            bundled.join(&manifest.files[0].path),
            b"overwritten by installer",
        )
        .unwrap();
        fs::remove_dir_all(&bundled).unwrap();
        let mut next = manifest.clone();
        next.version = "0.2.9".into();
        next.tag = "v0.2.9".into();
        let restarted = Manager::with_manifest(root, bundled, Some(next));
        restarted
            .prepare_from(&client, "http://127.0.0.1:9")
            .await
            .unwrap();
        assert_eq!(restarted.status().downloaded_bytes, 0);
        for entry in manifest.files {
            assert!(valid_file(&restarted.active().unwrap().join(&entry.path), &entry).unwrap());
        }
    }

    #[tokio::test]
    async fn only_the_changed_file_is_fetched_and_failed_updates_can_resume() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("managed");
        let bundled = directory.path().join("installation");
        let (manifest, contents) = manifest("0.2.8");
        bundle(&bundled, &contents);
        let manager = Manager::with_manifest(root.clone(), bundled.clone(), Some(manifest.clone()));
        let client = reqwest::Client::new();
        manager
            .prepare_from(&client, "http://127.0.0.1:9")
            .await
            .unwrap();
        let old_active = manager.active().unwrap();
        fs::remove_dir_all(&bundled).unwrap();
        let mut next = manifest.clone();
        next.version = "0.2.9".into();
        next.tag = "v0.2.9".into();
        let new_bytes = b"New configuration data";
        change(&mut next, "model/config.json", new_bytes);
        let changed = next
            .files
            .iter()
            .find(|entry| entry.path == "model/config.json")
            .unwrap()
            .clone();
        let server = Server::new();
        server.put("0.2.9", &changed.sha256, gzip(b"Corrupt configuration!"));
        let updater = Manager::with_manifest(root.clone(), bundled, Some(next.clone()));
        assert!(updater.prepare_from(&client, &server.base).await.is_err());
        assert_eq!(updater.status().state, "error");
        assert!(updater.active().is_none());
        assert!(!root.join("blobs").join(&changed.sha256).exists());
        assert!(!root.join("sets").join(&next.id).exists());
        for entry in &manifest.files {
            assert!(valid_file(&old_active.join(&entry.path), entry).unwrap());
        }
        server.put("0.2.9", &changed.sha256, gzip(new_bytes));
        updater.prepare_from(&client, &server.base).await.unwrap();
        assert_eq!(updater.status().state, "ready");
        assert_eq!(server.requests.lock().len(), 2);
        for entry in &next.files {
            assert!(valid_file(&updater.active().unwrap().join(&entry.path), entry).unwrap());
        }
    }

    #[tokio::test]
    async fn fresh_install_downloads_identical_contents_once_for_multiple_paths() {
        let directory = tempfile::tempdir().unwrap();
        let (mut manifest, contents) = manifest("0.2.8");
        let mut duplicate = manifest.files[0].clone();
        duplicate.path = "licenses/duplicate.txt".into();
        manifest.files.push(duplicate);
        manifest.files.sort_by(|a, b| a.path.cmp(&b.path));
        manifest.id = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&manifest.files).unwrap())
        );
        manifest.validate().unwrap();
        let server = Server::new();
        for (path, bytes) in &contents {
            let entry = manifest
                .files
                .iter()
                .find(|entry| entry.path == *path)
                .unwrap();
            server.put("0.2.8", &entry.sha256, gzip(bytes));
        }
        let manager = Manager::with_manifest(
            directory.path().join("managed"),
            directory.path().join("missing"),
            Some(manifest.clone()),
        );
        manager
            .prepare_from(&reqwest::Client::new(), &server.base)
            .await
            .unwrap();
        assert_eq!(server.requests.lock().len(), contents.len());
        assert_eq!(manager.status().completed_files, manifest.files.len());
        for entry in &manifest.files {
            assert!(valid_file(&manager.active().unwrap().join(&entry.path), entry).unwrap());
        }
    }

    #[tokio::test]
    async fn transient_download_errors_retry_but_corrupt_files_do_not() {
        let directory = tempfile::tempdir().unwrap();
        let (manifest, contents) = manifest("0.2.8");
        let bundled = directory.path().join("installed");
        bundle(&bundled, &contents);
        let entry = manifest.files[0].clone();
        fs::remove_file(bundled.join(&entry.path)).unwrap();
        let server = Server::new();
        server.put("0.2.8", &entry.sha256, gzip(&contents[&entry.path]));
        server
            .failures
            .lock()
            .insert(format!("/v0.2.8/clap-{}.gz", entry.sha256), 1);
        let manager =
            Manager::with_manifest(directory.path().join("managed"), bundled, Some(manifest));
        manager
            .prepare_from(&reqwest::Client::new(), &server.base)
            .await
            .unwrap();
        assert_eq!(server.requests.lock().len(), 2);
        assert_eq!(manager.status().state, "ready");
    }

    #[tokio::test]
    async fn lettered_release_downloads_use_the_actual_tag() {
        let directory = tempfile::tempdir().unwrap();
        let (mut manifest, contents) = manifest("0.2.1-a");
        manifest.tag = "v0.2.1a".into();
        manifest.validate().unwrap();
        let bundled = directory.path().join("installed");
        bundle(&bundled, &contents);
        let entry = manifest.files[0].clone();
        fs::remove_file(bundled.join(&entry.path)).unwrap();
        let server = Server::new();
        server.put("0.2.1a", &entry.sha256, gzip(&contents[&entry.path]));
        let manager =
            Manager::with_manifest(directory.path().join("managed"), bundled, Some(manifest));
        manager
            .prepare_from(&reqwest::Client::new(), &server.base)
            .await
            .unwrap();
        assert!(server.requests.lock()[0].starts_with("/v0.2.1a/"));
    }

    #[tokio::test]
    async fn client_updates_do_not_wait_for_missing_optional_model_downloads() {
        let directory = tempfile::tempdir().unwrap();
        let (manifest, _) = manifest("0.2.8");
        let manager = Manager::with_manifest(
            directory.path().join("managed"),
            directory.path().join("no-bundled-resources"),
            Some(manifest),
        );
        manager.preserve_before_update().await.unwrap();
        assert!(manager.active().is_none());
        assert!(!directory.path().join("managed").exists());
    }

    #[test]
    fn decompression_is_bounded_and_incomplete_gzip_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::create_dir(root.join("blobs")).unwrap();
        let (manifest, contents) = manifest("0.2.8");
        let entry = &manifest.files[0];
        let compressed = root.join("transfer.gz");
        fs::write(&compressed, gzip(&vec![b'x'; entry.size as usize + 10_000])).unwrap();
        assert!(unpack(root, &compressed, entry).is_err());
        let mut truncated = gzip(&contents[&entry.path]);
        truncated.truncate(truncated.len() - 6);
        fs::write(&compressed, truncated).unwrap();
        assert!(unpack(root, &compressed, entry).is_err());
        assert!(!root.join("blobs").join(&entry.sha256).exists());
    }

    #[test]
    fn migration_cleanup_preserves_custom_files_and_changed_source_files() {
        let directory = tempfile::tempdir().unwrap();
        let bundled = directory.path().join("installed");
        let (manifest, contents) = manifest("0.2.8");
        bundle(&bundled, &contents);
        fs::write(
            bundled.join("model/config.json"),
            b"custom changed configuration",
        )
        .unwrap();
        fs::write(bundled.join("model/custom.txt"), b"user file").unwrap();
        cleanup_bundled(&bundled, &manifest.files).unwrap();
        assert!(bundled.join("model/config.json").is_file());
        assert!(bundled.join("model/custom.txt").is_file());
        assert!(!bundled.join("model/tokenizer.json").exists());
        assert!(
            !bundled
                .join(WORKER_PATH)
                .exists()
        );
    }

    #[test]
    fn obsolete_sets_are_removed_without_deleting_current_previous_or_unknown_files() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::create_dir(root.join("blobs")).unwrap();
        fs::create_dir(root.join("sets")).unwrap();
        let (first, contents) = manifest("0.2.8");
        for entry in &first.files {
            fs::write(
                root.join("blobs").join(&entry.sha256),
                &contents[&entry.path],
            )
            .unwrap();
        }
        let first_path = materialize(root, &first).unwrap();
        let mut second = first.clone();
        change(&mut second, "model/config.json", b"second configuration");
        let mut third = second.clone();
        change(&mut third, "model/config.json", b"third configuration");
        for (set, bytes) in [
            (&second, b"second configuration".as_slice()),
            (&third, b"third configuration".as_slice()),
        ] {
            let entry = set
                .files
                .iter()
                .find(|file| file.path == "model/config.json")
                .unwrap();
            fs::write(root.join("blobs").join(&entry.sha256), bytes).unwrap();
        }
        std::thread::sleep(Duration::from_millis(15));
        let second_path = materialize(root, &second).unwrap();
        std::thread::sleep(Duration::from_millis(15));
        let third_path = materialize(root, &third).unwrap();
        fs::create_dir(root.join("sets/my-files")).unwrap();
        fs::write(root.join("blobs/keep.txt"), b"not a component").unwrap();
        prune(root, &third.id).unwrap();
        assert!(!first_path.exists());
        assert!(second_path.is_dir() && third_path.is_dir());
        assert!(root.join("sets/my-files").is_dir() && root.join("blobs/keep.txt").is_file());
        for entry in &third.files {
            assert!(valid_file(&third_path.join(&entry.path), entry).unwrap());
        }
    }
}
