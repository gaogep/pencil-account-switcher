use crate::model::StoreFile;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

pub trait DataDirectory {
    fn data_dir(&self) -> PathBuf;
}

fn private_directory(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub fn atomic_write(path: &Path, data: &[u8], private: bool) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing parent directory"))?;
    fs::create_dir_all(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::PermissionsExt;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(data)?;
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match file.persist(path) {
            Ok(_) => return Ok(()),
            Err(e) => {
                #[cfg(windows)]
                if e.error.raw_os_error() == Some(32) && Instant::now() < deadline {
                    file = e.file;
                    thread::sleep(Duration::from_millis(50));
                    continue;
                }
                let _ = (&deadline, &thread::current());
                return Err(e.error);
            }
        }
    }
}

pub struct FileCredentialStore {
    directory: PathBuf,
}
pub trait CredentialStore: Send + Sync {
    fn save(&self, account: &str, data: &[u8]) -> io::Result<()>;
    fn load(&self, account: &str) -> io::Result<Vec<u8>>;
    fn delete(&self, account: &str) -> io::Result<()>;
}
impl FileCredentialStore {
    pub fn new(platform: &impl DataDirectory) -> Self {
        Self {
            directory: platform.data_dir().join("credentials"),
        }
    }
    fn path(&self, account: &str) -> io::Result<PathBuf> {
        let id = Uuid::parse_str(account)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "凭据账号标识无效"))?;
        Ok(self.directory.join(format!(
            "{}.json",
            id.hyphenated().to_string().to_uppercase()
        )))
    }
    pub fn save(&self, account: &str, data: &[u8]) -> io::Result<()> {
        let path = self.path(account)?;
        private_directory(&self.directory)?;
        atomic_write(&path, data, true)
    }
    pub fn load(&self, account: &str) -> io::Result<Vec<u8>> {
        fs::read(self.path(account)?)
    }
    pub fn delete(&self, account: &str) -> io::Result<()> {
        match fs::remove_file(self.path(account)?) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }
}

impl CredentialStore for FileCredentialStore {
    fn save(&self, account: &str, data: &[u8]) -> io::Result<()> {
        FileCredentialStore::save(self, account, data)
    }
    fn load(&self, account: &str) -> io::Result<Vec<u8>> {
        FileCredentialStore::load(self, account)
    }
    fn delete(&self, account: &str) -> io::Result<()> {
        FileCredentialStore::delete(self, account)
    }
}

pub enum LoadStore {
    Ready(StoreFile),
    Blocked(&'static str),
}
pub struct Store {
    path: PathBuf,
}
impl Store {
    pub fn new(platform: &impl DataDirectory) -> Self {
        Self {
            path: platform.data_dir().join("store.json"),
        }
    }
    pub fn load(&self) -> LoadStore {
        let data = match fs::read(&self.path) {
            Ok(data) => data,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return LoadStore::Ready(StoreFile::default())
            }
            Err(_) => return LoadStore::Blocked("账号列表无法读取，已停止写入以免覆盖原文件"),
        };
        if let Ok(store) = serde_json::from_slice::<StoreFile>(&data) {
            if store.store_version <= 1 {
                return LoadStore::Ready(store);
            }
        }
        let stamp = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%SZ");
        let backup = self
            .path
            .with_file_name(format!("store.corrupt-{stamp}.json"));
        let _ = atomic_write(&backup, &data, false);
        LoadStore::Blocked("账号列表已损坏，已备份且不会覆盖原文件")
    }
    pub fn write(&self, store: &StoreFile) -> io::Result<()> {
        let data = serde_json::to_vec(store).map_err(io::Error::other)?;
        atomic_write(&self.path, &data, false)
    }
}
