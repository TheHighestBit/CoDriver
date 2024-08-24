use crate::utils::{dbg_log, DirWalkerEntry};
use crate::FDir;
use drive_v3::objects::File;
use drive_v3::{Credentials, Drive};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub trait CloudProvider {
    fn new() -> Result<Self, String>
    where
        Self: Sized;

    fn authenticate() -> Result<Credentials, String>;

    fn read_dir(&mut self, path: &PathBuf) -> Result<Vec<FDir>, String>;

    fn download_file(&self, path: &str) -> Result<String, String>;

    fn search_for(&self, fname: &str) -> Result<Vec<DirWalkerEntry>, String>;
}

pub struct GDrive {
    drive: Drive,
    cache: HashMap<String, File>,
}

impl CloudProvider for GDrive {
    fn new() -> Result<Self, String> {
        let credentials = GDrive::authenticate()?;

        let mut root_file = File::new();
        root_file.name = Some("gdrive:".to_string());
        root_file.id = Some("root".to_string());

        Ok(GDrive {
            drive: Drive::new(&credentials),
            cache: HashMap::from([("gdrive:".to_string(), root_file)]),
        })
    }

    fn authenticate() -> Result<Credentials, String> {
        // TODO - Think about how to store these securely
        let client_secrets_path = "client_secret.json";
        let scopes: Vec<&str> = vec!["https://www.googleapis.com/auth/drive"];

        let creds_path = "creds.json";
        let credentials = match fs::metadata(creds_path) {
            Ok(_) => {
                let mut credentials =
                    Credentials::from_file(&creds_path, &scopes).map_err(|e| e.to_string())?;

                if !credentials.are_valid() {
                    credentials.refresh().map_err(|e| e.to_string())?;

                    credentials.store(&creds_path).map_err(|e| e.to_string())?;
                }

                credentials
            }
            Err(_) => {
                let credentials =
                    Credentials::from_client_secrets_file(&client_secrets_path, &scopes)
                        .map_err(|e| e.to_string())?;

                credentials.store("creds.json").map_err(|e| e.to_string())?;

                credentials
            }
        };

        Ok(credentials)
    }

    fn read_dir(&mut self, path: &PathBuf) -> Result<Vec<FDir>, String> {
        let file_name = path.file_name().unwrap();
        let file_id = self
            .cache
            .get(file_name.to_str().unwrap())
            .unwrap()
            .id
            .clone()
            .unwrap();

        let file_list = self
            .drive
            .files
            .list()
            .fields("files(name,id,mimeType,size,fullFileExtension,modifiedTime)")
            .q(&format!("'{}' in parents", file_id))
            .execute()
            .map_err(|e| e.to_string())?;

        let mut files_fdir = Vec::new();
        if let Some(files) = file_list.files {
            for file in files {
                let file_name = file.name.clone().unwrap();
                self.cache.insert(file_name.clone(), file.clone());

                files_fdir.push(FDir {
                    name: file_name.clone(),
                    path: path
                        .join(&file_name)
                        .to_str()
                        .unwrap()
                        .to_string()
                        .replace("\\", "/"),
                    is_dir: (file.mime_type.unwrap() == "application/vnd.google-apps.folder") as i8,
                    size: file.size.unwrap_or("0".to_string()),
                    extension: file.full_file_extension.unwrap_or("".to_string()),
                    last_modified: file.modified_time.unwrap(),
                });
            }
        }

        Ok(files_fdir)
    }

    fn download_file(&self, path: &str) -> Result<String, String> {
        let slash_index = path.rfind('/').unwrap();
        let file_name = &path[slash_index + 1..];

        let cached_file = self.cache.get(file_name).unwrap();
        let id = cached_file.id.clone().unwrap();
        let saved_path = format!("{}{}", std::env::temp_dir().to_str().unwrap(), file_name);

        dbg_log(format!("Downloading {} to {}", file_name, saved_path));

        self.drive
            .files
            .get_media(&id)
            .save_to(&saved_path)
            .execute()
            .map_err(|e| e.to_string())?;

        Ok(saved_path)
    }

    fn search_for(&self, fname: &str) -> Result<Vec<DirWalkerEntry>, String> {
        let file_list = self
            .drive
            .files
            .list()
            .fields("files(name,id,mimeType,size,fullFileExtension,modifiedTime)")
            .q(&format!("name contains '{}'", &fname))
            .execute()
            .map_err(|e| e.to_string())?;

        let mut search_result = Vec::new();
        if let Some(files) = file_list.files {
            for file in files {
                let is_file = file.mime_type.unwrap() != "application/vnd.google-apps.folder";
                search_result.push(DirWalkerEntry {
                    name: file.name.clone().unwrap(),
                    path: file.name.clone().unwrap(),
                    depth: 0,
                    is_dir: !is_file,
                    is_file,
                    size: file.size.unwrap_or("0".to_string()).parse().unwrap(),
                    extension: file.full_file_extension.unwrap_or("".to_string()),
                    last_modified: file.modified_time.unwrap(),
                })
            }
        }

        Ok(search_result)
    }
}
