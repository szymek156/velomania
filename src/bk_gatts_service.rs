use std::str::from_utf8;

use anyhow::{anyhow, Result};
use btleplug::api::bleuuid::uuid_from_u16;
use btleplug::api::{Characteristic, Peripheral as _, WriteType};
use btleplug::platform::Peripheral;
use futures::StreamExt;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

pub const SERVICE_NAME: &str = "BK_GATTS";
const _SERVICE_UUID: Uuid = uuid_from_u16(0x00FF);
const FILE_TRANS_UUID: Uuid = uuid_from_u16(0xFF01);
const FILE_LIST_UUID: Uuid = uuid_from_u16(0xFF02);

#[derive(Debug)]
pub struct BkClient {
    pub client: Peripheral,
}

#[derive(Debug)]
pub struct FileInfo {
    id: usize,
    filename: String,
    size: usize,
}

impl BkClient {
    pub async fn list_bc_files(&self) -> Result<Vec<FileInfo>> {
        debug!("services listing");

        let file_list_char = self.get_characteristic(FILE_LIST_UUID)?;
        let raw_response = self.client.read(&file_list_char).await?;
        let response = from_utf8(&raw_response)?;
        info!("Got response {response}");

        // Response is in somewhat CSV format
        // filename1, size
        // filename2, size

        let mut files = vec![];

        for (idx, line) in response.lines().enumerate() {
            let split: Vec<&str> = line.split_terminator(", ").collect();

            debug!("Got split {split:?}");

            match *split.as_slice() {
                [filename, size] => files.push(FileInfo {
                    id: idx,
                    filename: filename.to_string(),
                    size: size.parse()?,
                }),

                _ => {
                    return Err(anyhow!("Invalid split {split:?}"));
                }
            };
        }

        Ok(files)
    }

    pub async fn fetch_file(&self, file: &FileInfo) -> Result<()> {
        let fetch_char = self.get_characteristic(FILE_TRANS_UUID)?;

        // TODO: that could be a struct field?
        let files_char = self.get_characteristic(FILE_LIST_UUID)?;

        self.client.subscribe(&fetch_char).await?;

        // Write the id of the file client wants to fetch.
        // That will trigger stream of indications, with chunks of data
        // TODO: make sure MTU is set to 500 on this side. Now it's working by luck
        let data: [u8; 1] = [file.id as u8];
        self.client
            .write(&files_char, &data, WriteType::WithResponse)
            .await?;

        let mut notifications = self.client.notifications().await?;

        let mut downloaded_file: Vec<u8> = Vec::with_capacity(file.size);

        while let Some(data) = notifications.next().await {
            if data.uuid == FILE_TRANS_UUID {
                debug!("Got file chunk of size {}", data.value.len());
                downloaded_file.extend_from_slice(&data.value);

                // TODO: possible to avoid it? How while loop should change
                if downloaded_file.len() == file.size {
                    break;
                }
            } else {
                warn!("Unexpected notification from uuid {}", data.uuid);
            }
        }

        info!("Unsub...");

        self.client.unsubscribe(&fetch_char).await?;

        info!("Writing the file {}...", file.filename);
        // TODO: spawn task?
        let mut filepath = OpenOptions::new()
            .write(true)
            .create(true)
            .append(false)
            .open(format!("/tmp/{}", file.filename))
            .await?;

        filepath.write_all(&downloaded_file).await?;

        info!("Done!");
        Ok(())
    }

    fn get_characteristic(&self, uuid: Uuid) -> Result<Characteristic> {
        let chars = self.client.characteristics();

        let cmd_char = chars
            .iter()
            .find(|c| c.uuid == uuid)
            .ok_or_else(|| anyhow!("Unable to find characteristic {uuid:?}"))?;

        Ok(cmd_char.clone())
    }
}
