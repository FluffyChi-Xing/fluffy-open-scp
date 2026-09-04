//! Parallel batch export primitives.

use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{Result, TextureOutputFormat, export_texture};
use rw4::DecodedTexture;

/// A completed item notification. Callbacks may arrive in any order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportProgress {
    pub completed: usize,
    pub total: usize,
}

/// One item that failed during a batch export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportFailure {
    pub index: usize,
    pub error: String,
}

/// Results of a batch: successful output remains available even when peers fail.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BatchExport {
    pub outputs: Vec<(usize, Vec<u8>)>,
    pub failures: Vec<ExportFailure>,
}

impl BatchExport {
    pub fn is_success(&self) -> bool {
        self.failures.is_empty()
    }
}

/// Export all items in parallel and aggregate failures instead of aborting early.
pub fn export_batch<T, F>(
    items: &[T],
    worker: F,
    progress: impl Fn(ExportProgress) + Sync,
) -> BatchExport
where
    T: Sync,
    F: Fn(&T) -> Result<Vec<u8>> + Sync + Send,
{
    let total = items.len();
    let done = AtomicUsize::new(0);
    let mut results: Vec<_> = items
        .par_iter()
        .enumerate()
        .map(|(index, item)| {
            let result = worker(item);
            let completed = done.fetch_add(1, Ordering::Relaxed) + 1;
            progress(ExportProgress { completed, total });
            (index, result)
        })
        .collect();
    results.sort_unstable_by_key(|(index, _)| *index);

    let mut batch = BatchExport::default();
    for (index, result) in results {
        match result {
            Ok(bytes) => batch.outputs.push((index, bytes)),
            Err(error) => batch.failures.push(ExportFailure {
                index,
                error: error.to_string(),
            }),
        }
    }
    batch
}

/// Convenience batch operation for already-decoded textures.
pub fn export_textures(
    textures: &[DecodedTexture],
    format: TextureOutputFormat,
    progress: impl Fn(ExportProgress) + Sync,
) -> BatchExport {
    export_batch(
        textures,
        |texture| export_texture(texture, format),
        progress,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_failures_and_reports_every_item() {
        let items = [1u8, 2, 3, 4];
        let events = std::sync::Mutex::new(Vec::new());
        let result = export_batch(
            &items,
            |item| {
                if *item == 2 || *item == 4 {
                    Err(crate::Error::ResourceNotFound(dbpf::ResourceId {
                        type_id: 1,
                        group: 2,
                        instance: u32::from(*item),
                    }))
                } else {
                    Ok(vec![*item])
                }
            },
            |event| events.lock().unwrap().push(event),
        );
        assert_eq!(result.outputs, vec![(0, vec![1]), (2, vec![3])]);
        assert_eq!(
            result.failures.iter().map(|f| f.index).collect::<Vec<_>>(),
            vec![1, 3]
        );
        let events = events.into_inner().unwrap();
        assert_eq!(events.len(), 4);
        assert!(events.iter().all(|event| event.total == 4));
        assert!(events.iter().any(|event| event.completed == 4));
    }
}
