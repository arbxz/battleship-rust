// audio.rs — Thin wrapper around rodio for playing WAV sound effects.
// Sounds are loaded from the `assets/` folder relative to the executable.
// All playback is non-blocking (fires on a background thread via rodio).

use std::io::BufReader;
use std::path::PathBuf;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

/// Holds the audio output stream and handle.
/// The `_stream` must be kept alive for the lifetime of playback.
pub struct Audio {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    assets_dir: PathBuf,
}

/// Sound effect identifiers mapped to WAV filenames.
pub enum Sfx {
    Select,
    Confirm,
    Cancel,
    Hit,
    Sunk,
}

impl Sfx {
    fn filename(&self) -> &str {
        match self {
            Sfx::Select => "Select 1.wav",
            Sfx::Confirm => "Confirm 1.wav",
            Sfx::Cancel => "Cancel 1.wav",
            Sfx::Hit => "Hit damage 1.wav",
            Sfx::Sunk => "Balloon Pop 1.wav",
        }
    }
}

impl Audio {
    /// Try to initialise audio. Returns None if audio output is unavailable.
    pub fn new() -> Option<Self> {
        let (stream, handle) = OutputStream::try_default().ok()?;

        // Locate assets/ relative to the executable (works for cargo run and installed binaries)
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));

        // Try: next to exe → ../../assets (cargo target/debug) → cwd/assets
        let candidates = [
            exe_dir.as_ref().map(|d| d.join("assets")),
            exe_dir.as_ref().map(|d| d.join("../../assets")),
            Some(PathBuf::from("assets")),
        ];

        let assets_dir = candidates
            .into_iter()
            .flatten()
            .find(|p| p.is_dir())
            .unwrap_or_else(|| PathBuf::from("assets"));

        Some(Audio {
            _stream: stream,
            handle,
            assets_dir,
        })
    }

    /// Play a sound effect. Silently does nothing if the file is missing.
    pub fn play(&self, sfx: Sfx) {
        let path = self.assets_dir.join(sfx.filename());
        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(_) => return,
        };
        let source = match Decoder::new(BufReader::new(file)) {
            Ok(s) => s,
            Err(_) => return,
        };
        if let Ok(sink) = Sink::try_new(&self.handle) {
            sink.append(source);
            sink.detach(); // play in background, don't block
        }
    }
}
