use crate::utils::{dbg_log, DirWalkerEntry};
use crate::{FDir, SimpleDirInfo};
use chrono::{DateTime, Utc};
use drive_v3::objects::{File, UploadType};
use drive_v3::{Credentials, Drive};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub trait CloudProvider {
    fn new() -> Self;

    fn authenticate(&mut self) -> Result<(), String>;

    fn sign_out(&mut self) -> Result<(), String>;

    fn read_dir(&mut self, path: &PathBuf) -> Result<Vec<FDir>, String>;

    fn download(&mut self, from_path: &str, to_path: &str) -> Result<String, String>;

    fn upload(&mut self, from_path: &str, to_path: &str) -> Result<(), String>;

    fn create_dir(&mut self, from_path: &str, to_path: &str) -> Result<(), String>;

    fn search(&mut self, fname: &str) -> Result<Vec<DirWalkerEntry>, String>;

    fn get_item_size(&self, path: &str) -> Result<SimpleDirInfo, String>;

    fn copy_items(&mut self, arr_items: Vec<FDir>, copy_to_path: &str) -> Result<(), String>;
}

pub struct GDrive {
    drive: Option<Drive>,
    path2file: HashMap<String, File>,
}

impl CloudProvider for GDrive {
    fn new() -> Self {
        GDrive {
            drive: None,
            path2file: HashMap::from([("gdrive:".to_string(), GDrive::construct_root_file())]),
        }
    }

    fn authenticate(&mut self) -> Result<(), String> {
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

        self.drive = Some(Drive::new(&credentials));
        self.path2file.clear();
        self.path2file
            .insert("gdrive:".to_string(), GDrive::construct_root_file());

        Ok(())
    }

    fn sign_out(&mut self) -> Result<(), String> {
        fs::remove_file("creds.json").map_err(|e| e.to_string())?;
        self.drive = None;
        Ok(())
    }

    fn read_dir(&mut self, path: &PathBuf) -> Result<Vec<FDir>, String> {
        if self.drive.is_none() {
            self.authenticate()?;
        }

        let file_id = self
            .path2file
            .get(path.to_str().unwrap())
            .unwrap()
            .id
            .clone()
            .unwrap();

        let file_list = self
            .drive
            .as_ref()
            .unwrap()
            .files
            .list()
            .fields("files(name,id,mimeType,size,fileExtension,modifiedTime,parents)")
            .q(&format!("'{}' in parents", file_id))
            .execute()
            .map_err(|e| e.to_string())?;

        let mut files_fdir = Vec::new();
        if let Some(files) = file_list.files {
            for file in files {
                let file_name = file.name.clone().unwrap();
                self.path2file.insert(
                    format!("{}/{}", path.to_str().unwrap(), file_name),
                    file.clone(),
                );

                files_fdir.push(FDir {
                    name: file_name.clone(),
                    path: path
                        .join(&file_name)
                        .to_str()
                        .unwrap()
                        .to_string()
                        .replace("\\", "/"),
                    is_dir: GDrive::is_dir(&file) as i8,
                    size: file.size.unwrap_or("0".to_string()),
                    extension: format!(".{}", file.file_extension.unwrap_or("".to_string())),
                    last_modified: {
                        file.modified_time
                            .unwrap()
                            .parse::<DateTime<Utc>>()
                            .unwrap()
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string()
                    },
                });
            }
        }

        Ok(files_fdir)
    }

    fn download(&mut self, from_path: &str, to_path: &str) -> Result<String, String> {
        if self.drive.is_none() {
            self.authenticate()?;
        }

        let cached_file = self.path2file.get(from_path).unwrap();
        let id = cached_file.id.clone().unwrap();
        let fname = cached_file.name.as_ref().unwrap();
        let saved_path = format!("{}{}", to_path, fname);

        self.drive
            .as_ref()
            .unwrap()
            .files
            .get_media(&id)
            .save_to(&saved_path)
            .execute()
            .map_err(|e| e.to_string())?;

        Ok(saved_path)
    }

    fn upload(&mut self, from_path: &str, to_path: &str) -> Result<(), String> {
        if fs::metadata(from_path).unwrap().is_dir() {
            self.create_dir(from_path, to_path)?;

            let children = fs::read_dir(from_path)
                .map_err(|e| e.to_string())?
                .map(|entry| entry.unwrap().path())
                .collect::<Vec<_>>();

            for child_path in children
                .iter()
                .filter(|p| fs::metadata(p).unwrap().is_dir())
            {
                let child_path_str = child_path.to_str().unwrap().replace("\\", "/");
                let new_to_path = format!("{}/{}", to_path, from_path.split('/').last().unwrap());

                self.upload(&child_path_str, &new_to_path)?;
            }

            for child_path in children
                .iter()
                .filter(|p| fs::metadata(p).unwrap().is_file())
            {
                let child_path_str = child_path.to_str().unwrap().replace("\\", "/");
                let new_to_path = format!("{}/{}", to_path, from_path.split('/').last().unwrap());

                self.upload(&child_path_str, &new_to_path)?;
            }

            return Ok(());
        }

        if self.drive.is_none() {
            self.authenticate()?;
        }

        let md = File {
            name: Some(from_path.split('/').last().unwrap().to_string()),
            parents: Some(vec![self
                .path2file
                .get(to_path)
                .unwrap()
                .id
                .clone()
                .unwrap()]),
            ..Default::default()
        };

        let new_file = if fs::metadata(from_path).unwrap().len() > 5 * 1024 * 1024 {
            dbg_log(format!(
                "Starting resumable upload of {} to {}",
                from_path, to_path
            ));

            self.drive
                .as_ref()
                .unwrap()
                .files
                .create()
                .upload_type(UploadType::Resumable)
                .callback(|total_bytes, uploaded_bytes| {
                    println!(
                        "Uploaded {} bytes, out of a total of {}.",
                        uploaded_bytes, total_bytes
                    );
                })
                .metadata(md)
                .content_source(from_path)
                .execute()
                .map_err(|e| e.to_string())?
        } else {
            dbg_log(format!(
                "Starting standard upload of {} to {}",
                from_path, to_path
            ));

            self.drive
                .as_ref()
                .unwrap()
                .files
                .create()
                .upload_type(UploadType::Multipart)
                .metadata(md)
                .content_source(from_path)
                .execute()
                .map_err(|e| e.to_string())?
        };

        self.path2file.insert(
            format!("{}/{}", to_path, new_file.name.as_ref().unwrap()),
            new_file,
        );

        Ok(())
    }

    fn create_dir(&mut self, from_path: &str, to_path: &str) -> Result<(), String> {
        if self.drive.is_none() {
            self.authenticate()?;
        }

        let dir_name = from_path.split('/').last().unwrap();

        dbg_log(format!("Creating directory {}/{}", to_path, &dir_name,));

        let md = File {
            name: Some(dir_name.to_string()),
            parents: Some(vec![self
                .path2file
                .get(to_path)
                .unwrap()
                .id
                .clone()
                .unwrap()]),
            mime_type: Some("application/vnd.google-apps.folder".to_string()),
            ..Default::default()
        };

        let new_dir = self
            .drive
            .as_ref()
            .unwrap()
            .files
            .create()
            .upload_type(UploadType::Multipart)
            .metadata(md)
            .execute()
            .map_err(|e| e.to_string())?;

        self.path2file
            .insert(format!("{}/{}", to_path, &dir_name), new_dir);

        Ok(())
    }

    fn search(&mut self, fname: &str) -> Result<Vec<DirWalkerEntry>, String> {
        if self.drive.is_none() {
            self.authenticate()?;
        }

        let file_list = self
            .drive
            .as_ref()
            .unwrap()
            .files
            .list()
            .fields("files(name,id,mimeType,size,fullFileExtension,modifiedTime)")
            .q(&format!("name contains '{}'", &fname))
            .execute()
            .map_err(|e| e.to_string())?;

        let mut search_result = Vec::new();
        if let Some(files) = file_list.files {
            for file in files {
                let file_path = format!("gdrive:/{}", file.name.clone().unwrap());
                self.path2file.insert(file_path.clone(), file.clone());

                let is_file = !GDrive::is_dir(&file);
                search_result.push(DirWalkerEntry {
                    name: file.name.clone().unwrap(),
                    path: file_path,
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

    fn get_item_size(&self, path: &str) -> Result<SimpleDirInfo, String> {
        let cached_item = self.path2file.get(path).ok_or("item not in cache")?;

        if Self::is_dir(cached_item) {
            let mut total_items: u64 = 0;
            let size = self.calc_size(cached_item, &mut total_items);

            Ok(SimpleDirInfo {
                size,
                count_elements: total_items,
            })
        } else {
            Ok(SimpleDirInfo {
                size: cached_item
                    .size
                    .as_ref()
                    .unwrap_or(&"0".to_string())
                    .parse()
                    .unwrap(),
                count_elements: 1,
            })
        }
    }

    fn copy_items(&mut self, arr_items: Vec<FDir>, copy_to_path: &str) -> Result<(), String> {
        for item in arr_items {
            self.copy(&item.path, copy_to_path)?;
        }

        Ok(())
    }
}

impl GDrive {
    pub fn is_authenticated(&self) -> Result<(), String> {
        if self.drive.is_none() {
            return Err("Drive not authenticated".to_string());
        }

        Ok(())
    }

    fn construct_root_file() -> File {
        let mut root_file = File::new();
        root_file.name = Some("gdrive:".to_string());
        root_file.id = Some("root".to_string());

        root_file
    }

    fn is_dir(file: &File) -> bool {
        file.mime_type.as_ref().unwrap() == "application/vnd.google-apps.folder"
    }

    fn calc_size(&self, file: &File, total_items: &mut u64) -> u64 {
        *total_items += 1;
        if GDrive::is_dir(file) {
            let mut size = 0;

            let children = self
                .drive
                .as_ref()
                .unwrap()
                .files
                .list()
                .fields("files(id,size,mimeType)")
                .q(&format!("'{}' in parents", file.id.as_ref().unwrap()))
                .execute()
                .unwrap()
                .files
                .unwrap();

            for child in children {
                size += self.calc_size(&child, total_items);
            }

            size
        } else {
            file.size.as_ref().unwrap().parse().unwrap()
        }
    }

    pub fn copy(&mut self, from: &str, to: &str) -> Result<(), String> {
        if self.drive.is_none() {
            self.authenticate()?;
        }

        if to.starts_with("gdrive:") {
            if from.starts_with("gdrive:") {
                // Gdrive to Gdrive copy
                todo!();
            } else {
                // Local to Gdrive copy
                self.upload(from, to)?;
            }
        } else {
            // Gdrive to Local copy
            let item = self.path2file.get(from).ok_or("item not in cache")?;
            let item_name = item.name.clone().unwrap();

            if fs::metadata(to).is_err() {
                fs::create_dir_all(to).map_err(|e| e.to_string())?;
            }

            if Self::is_dir(item) {
                let children = self.read_dir(&PathBuf::from(&from))?;

                self.copy_items(children, format!("{}/{}", to, item_name).as_str())?;
            } else {
                self.download(from, &format!("{}/", to))?;
            }
        }

        Ok(())
    }
}
