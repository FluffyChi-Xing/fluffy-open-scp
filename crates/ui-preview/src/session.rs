use crate::background::{Asset, Background, TOKEN, invalid};
use base64::{Engine, engine::general_purpose::STANDARD};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_PAGES: usize = 32;
const APPLIED: &str = "applied.jsonl";

#[derive(Serialize, Deserialize)]
struct Receipt {
    page: String,
    ticket: String,
    sha256: String,
    time: f64,
}

#[derive(Default)]
struct Page {
    pending: Option<(u64, String)>,
    applied: Option<(u64, String)>,
}

enum Operation {
    Background,
    Applied,
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_page(value: &str) -> bool {
    (13..=32).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase())
}

fn timestamp() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

pub(crate) struct Session {
    directory: PathBuf,
    token: String,
    background: Background,
    idle: Bytes,
    pages: VecDeque<(String, Page)>,
}

impl Session {
    pub(crate) fn open(directory: &Path) -> io::Result<Self> {
        let token = fs::read_to_string(directory.join(TOKEN))?;
        if !lower_hex(token.trim(), 32) {
            return Err(invalid("Invalid preview token"));
        }
        let mut session = Self {
            directory: directory.to_owned(),
            token: token.trim().to_owned(),
            background: Background::new(directory),
            idle: STANDARD
                .decode(concat!(
                    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNgY",
                    "AAAAAMAASsJTYQAAAAASUVORK5CYII="
                ))
                .map_err(io::Error::other)?
                .into(),
            pages: VecDeque::new(),
        };
        session.recover()?;
        Ok(session)
    }

    fn recover(&mut self) -> io::Result<()> {
        let log = match File::open(self.directory.join(APPLIED)) {
            Ok(log) => log,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        for line in BufReader::new(log).lines() {
            let line = line?;
            if let Ok(r) = serde_json::from_str::<Receipt>(&line)
                && valid_page(&r.page)
                && lower_hex(&r.sha256, 64)
                && let Ok(ticket) = r.ticket.parse::<u64>()
            {
                let mut page = self.take_page(&r.page).unwrap_or_default();
                if page.applied.as_ref().is_none_or(|(last, _)| ticket > *last) {
                    page.applied = Some((ticket, r.sha256));
                }
                self.store_page(&r.page, page);
            }
        }
        Ok(())
    }

    fn take_page(&mut self, id: &str) -> Option<Page> {
        let index = self.pages.iter().position(|(page, _)| page == id)?;
        self.pages.remove(index).map(|(_, page)| page)
    }

    fn store_page(&mut self, id: &str, page: Page) {
        self.pages.push_back((id.to_owned(), page));
        if self.pages.len() > MAX_PAGES {
            self.pages.pop_front();
        }
    }

    fn append(&self, file: &str, record: &impl Serialize) -> io::Result<()> {
        let mut bytes = serde_json::to_vec(record)?;
        bytes.push(b'\n');
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.directory.join(file))?
            .write_all(&bytes)
    }

    /// Preserve the URL and image response contract used by the deployed page.
    pub(crate) fn handle(&mut self, path: &str) -> io::Result<Option<Bytes>> {
        let mut parts = path.strip_prefix('/').unwrap_or_default().split('/');
        let (Some(token), Some(operation), Some(id), Some(sequence), None) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            return Ok(None);
        };
        let Ok(ticket) = sequence.parse::<u64>() else {
            return Ok(None);
        };
        if token != self.token
            || !valid_page(id)
            || sequence.starts_with('0')
            || sequence.len() > 10
            || !sequence.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Ok(None);
        }
        let operation = match operation {
            "background" => Operation::Background,
            "applied" => Operation::Applied,
            _ => return Ok(None),
        };

        let found = self.take_page(id);
        if found.is_none() {
            if matches!(operation, Operation::Applied) {
                return Ok(Some(self.idle.clone()));
            }
            self.append(
                "connected.jsonl",
                &serde_json::json!({"page": id, "time": timestamp()}),
            )?;
        }
        let mut page = found.unwrap_or_default();
        let body = match operation {
            Operation::Background => Ok(self.prepare(&mut page, ticket)),
            Operation::Applied => self.acknowledge(&mut page, id, ticket).map(|()| None),
        };
        self.store_page(id, page);
        Ok(Some(body?.unwrap_or_else(|| self.idle.clone())))
    }

    fn prepare(&mut self, page: &mut Page, ticket: u64) -> Option<Bytes> {
        let newer_than =
            |entry: &Option<(u64, String)>| entry.as_ref().is_none_or(|(id, _)| ticket > *id);
        if newer_than(&page.applied)
            && newer_than(&page.pending)
            && let Some(Asset { sha256, bytes }) = self.background.current()
            && page
                .applied
                .as_ref()
                .is_none_or(|(_, previous)| *previous != sha256)
        {
            page.pending = Some((ticket, sha256));
            return Some(bytes);
        }
        None
    }

    fn acknowledge(&self, page: &mut Page, id: &str, ticket: u64) -> io::Result<()> {
        if let Some((pending, sha256)) = page.pending.as_ref()
            && *pending == ticket
        {
            let r = Receipt {
                page: id.to_owned(),
                ticket: ticket.to_string(),
                sha256: sha256.clone(),
                time: timestamp(),
            };
            self.append(APPLIED, &r)?;
            page.applied = page.pending.take();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{background::random_hex, init, publish};
    use image::{Rgba, RgbaImage};

    const PAGE: &str = "1795712401054example";

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn create() -> Self {
            let path =
                std::env::temp_dir().join(format!("openscp-ui-preview-{}", random_hex().unwrap()));
            init(&path).unwrap();
            Self(path)
        }

        fn candidate(&self, red: u8) -> String {
            let path = self.0.join("candidate.png");
            RgbaImage::from_pixel(32, 32, Rgba([red, 0, 0, 255]))
                .save(&path)
                .unwrap();
            publish(&self.0, &path).unwrap()
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            for file in [
                TOKEN,
                "candidate.png",
                "background.png",
                "connected.jsonl",
                APPLIED,
            ] {
                let _ = fs::remove_file(self.0.join(file));
            }
            let _ = fs::remove_dir(&self.0);
        }
    }

    fn call(session: &mut Session, operation: &str, ticket: u64) -> Bytes {
        let path = format!("/{}/{operation}/{PAGE}/{ticket}", session.token);
        session.handle(&path).unwrap().unwrap()
    }

    #[test]
    fn applies_two_images_on_one_page_across_a_backend_restart() {
        let disk = TestDirectory::create();
        let first = disk.candidate(3);
        let mut session = Session::open(&disk.0).unwrap();
        assert!(
            session
                .handle(&format!("/{}/background/{PAGE}/1", "a"))
                .unwrap()
                .is_none()
        );
        assert_ne!(call(&mut session, "background", 1), session.idle);
        call(&mut session, "applied", 2);
        assert!(!disk.0.join(APPLIED).exists());
        call(&mut session, "applied", 1);
        call(&mut session, "applied", 1);
        let mut session = Session::open(&disk.0).unwrap();
        assert_eq!(call(&mut session, "background", 2), session.idle);
        let second = disk.candidate(7);
        assert_ne!(call(&mut session, "background", 3), session.idle);
        call(&mut session, "applied", 3);
        let log = fs::read_to_string(disk.0.join(APPLIED)).unwrap();
        let receipts: Vec<Receipt> = log
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(receipts.len(), 2);
        assert_eq!(receipts[0].sha256, first);
        assert_eq!(receipts[1].sha256, second);
        assert_eq!(receipts[0].page, receipts[1].page);
        assert_eq!(
            fs::read_to_string(disk.0.join("connected.jsonl"))
                .unwrap()
                .lines()
                .count(),
            1
        );
    }

    #[test]
    fn rejects_corrupt_files_and_keeps_the_last_good_picture() {
        let disk = TestDirectory::create();
        disk.candidate(3);
        let mut session = Session::open(&disk.0).unwrap();
        let prior = call(&mut session, "background", 1);
        let path = disk.0.join("candidate.png");
        fs::write(&path, "invalid").unwrap();
        assert!(publish(&disk.0, &path).is_err());
        assert_eq!(fs::read(disk.0.join("background.png")).unwrap(), prior);
        fs::write(disk.0.join("background.png"), "invalid").unwrap();
        assert_eq!(call(&mut session, "background", 2), prior);
    }
}
