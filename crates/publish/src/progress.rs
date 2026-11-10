//! Progress events of a single image upload, shared by both clients and
//! meant to be forwarded by the executor (Phase 5.5) to GUI/CLI/MCP and the
//! upload log.

/// Phase of one image upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadStep {
    /// Apple: asset reservation created (`appScreenshots` POST).
    Reserve,
    /// Bytes are being transferred (Apple: per upload operation; Google:
    /// the whole image in one request).
    Transfer,
    /// Apple: reservation committed with checksum.
    Commit,
    /// Apple: waiting for `assetDeliveryState` to become `COMPLETE`.
    Processing,
    /// The image is uploaded (and processed, for Apple).
    Done,
}

/// One progress update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    /// Current phase.
    pub step: UploadStep,
    /// Store id of the asset once known (Apple screenshot id, Google image id).
    pub asset_id: Option<String>,
    /// Bytes confirmed by the store so far.
    pub bytes_sent: u64,
    /// File size in bytes.
    pub bytes_total: u64,
    /// `(parts done, parts total)` while transferring in parts.
    pub parts: Option<(usize, usize)>,
    /// Store state while processing (`UPLOAD_COMPLETE`, …).
    pub state: Option<String>,
}

impl Progress {
    pub(crate) fn new(step: UploadStep, bytes_total: u64) -> Self {
        Self {
            step,
            asset_id: None,
            bytes_sent: 0,
            bytes_total,
            parts: None,
            state: None,
        }
    }
}

/// Progress callback. Called on the calling task, never concurrently.
pub type ProgressFn<'a> = &'a (dyn Fn(&Progress) + Send + Sync);

/// A progress callback that ignores all updates.
pub fn ignore_progress(_: &Progress) {}
