// Source: https://github.com/tfachmann/typst-as-library/blob/main/Cargo.toml
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use clap::ValueEnum;
use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::term;
use typst::diag::{FileError, FileResult, PackageError, PackageResult, Severity, SourceDiagnostic};
use typst::ecow::eco_format;
use typst::foundations::{Bytes, Datetime, Dict, IntoValue};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, Lines, Source, Span, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, WorldExt};
use typst_kit::fonts::{FontSearcher, FontSlot};

/// Main interface that determines the environment for Typst.
pub struct TypstWrapperWorld {
    /// Root path to which files will be resolved.
    root: PathBuf,

    // Current working directory.
    workdir: PathBuf,

    /// The content of a source.
    pub source: Source,

    /// The standard library.
    library: LazyHash<Library>,

    /// Metadata about all known fonts.
    book: LazyHash<FontBook>,

    /// Metadata about all known fonts.
    fonts: Vec<FontSlot>,

    /// Map of all known files.
    files: Arc<Mutex<HashMap<FileId, FileEntry>>>,

    /// Cache directory (e.g. where packages are downloaded to).
    cache_directory: PathBuf,

    /// http agent to download packages.
    http: reqwest::blocking::Client,

    /// Datetime.
    time: time::OffsetDateTime,
}

impl TypstWrapperWorld {
    pub fn new(root: String, source: String, inputs: &Vec<(String, String)>) -> Self {
        let root = PathBuf::from(root);
        let fonts = FontSearcher::new().include_system_fonts(true).search();

        let inputs: Dict = inputs
            .iter()
            .map(|(k, v)| (k.as_str().into(), v.as_str().into_value()))
            .collect();
        let library = Library::builder().with_inputs(inputs).build();

        Self {
            library: LazyHash::new(library),
            book: LazyHash::new(fonts.book),
            root,
            workdir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            fonts: fonts.fonts,
            source: Source::new(
                FileId::new(None, VirtualPath::new("main.typ")),
                source.into(),
            ),
            time: time::OffsetDateTime::now_utc(),
            cache_directory: std::env::var_os("CACHE_DIRECTORY")
                .map(|os_path| os_path.into())
                .unwrap_or(std::env::temp_dir()),
            http: reqwest::blocking::Client::new(),
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// A File that will be stored in the HashMap.
#[derive(Clone, Debug)]
struct FileEntry {
    bytes: Bytes,
    source: Option<Source>,
}

impl FileEntry {
    fn new(bytes: Vec<u8>, source: Option<Source>) -> Self {
        Self {
            bytes: Bytes::new(bytes),
            source,
        }
    }

    fn source(&mut self, id: FileId) -> FileResult<Source> {
        let source = if let Some(source) = &self.source {
            source
        } else {
            let contents = std::str::from_utf8(&self.bytes).map_err(|_| FileError::InvalidUtf8)?;
            let contents = contents.trim_start_matches('\u{feff}');
            let source = Source::new(id, contents.into());
            self.source.insert(source)
        };
        Ok(source.clone())
    }
}

impl TypstWrapperWorld {
    /// Helper to handle file requests.
    ///
    /// Requests will be either in packages or a local file.
    fn file(&self, id: FileId) -> FileResult<FileEntry> {
        let mut files = self.files.lock().map_err(|_| FileError::AccessDenied)?;
        if let Some(entry) = files.get(&id) {
            return Ok(entry.clone());
        }
        let path = if let Some(package) = id.package() {
            let package_dir = self.download_package(package)?;
            id.vpath().resolve(&package_dir)
        } else {
            id.vpath().resolve(&self.root)
        }
        .ok_or(FileError::AccessDenied)?;

        let content = std::fs::read(&path).map_err(|error| FileError::from_io(error, &path))?;
        Ok(files
            .entry(id)
            .or_insert(FileEntry::new(content, None))
            .clone())
    }

    /// Downloads the package and returns the system path of the unpacked package.
    fn download_package(&self, package: &PackageSpec) -> PackageResult<PathBuf> {
        let package_subdir = format!("{}/{}/{}", package.namespace, package.name, package.version);
        let path = self.cache_directory.join(package_subdir);

        if path.exists() {
            return Ok(path);
        }

        eprintln!("downloading {package}");
        let url = format!(
            "https://packages.typst.org/{}/{}-{}.tar.gz",
            package.namespace, package.name, package.version,
        );

        let mut response = retry(|| {
            let response = self
                .http
                .get(&url)
                .send()
                .map_err(|error| eco_format!("{error}"))?;

            let status = response.status();
            if !status.is_success() {
                return Err(eco_format!(
                    "response returned unsuccessful status code {status}",
                ));
            }

            Ok(response)
        })
        .map_err(|error| PackageError::NetworkFailed(Some(error)))?;

        let mut compressed_archive = Vec::new();
        response
            .read_to_end(&mut compressed_archive)
            .map_err(|error| PackageError::NetworkFailed(Some(eco_format!("{error}"))))?;
        let raw_archive = zune_inflate::DeflateDecoder::new(&compressed_archive)
            .decode_gzip()
            .map_err(|error| PackageError::MalformedArchive(Some(eco_format!("{error}"))))?;
        let mut archive = tar::Archive::new(raw_archive.as_slice());
        archive.unpack(&path).map_err(|error| {
            _ = std::fs::remove_dir_all(&path);
            PackageError::MalformedArchive(Some(eco_format!("{error}")))
        })?;

        Ok(path)
    }
}

/// This is the interface we have to implement such that `typst` can compile it.
///
/// I have tried to keep it as minimal as possible
impl typst::World for TypstWrapperWorld {
    /// Standard library.
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    /// Metadata about all known Books.
    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    /// Accessing the main source file.
    fn main(&self) -> FileId {
        self.source.id()
    }

    /// Accessing a specified source file (based on `FileId`).
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.source.id() {
            Ok(self.source.clone())
        } else {
            self.file(id)?.source(id)
        }
    }

    /// Accessing a specified file (non-file).
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.file(id).map(|file| file.bytes.clone())
    }

    /// Accessing a specified font per index of font book.
    fn font(&self, id: usize) -> Option<Font> {
        self.fonts[id].get()
    }

    /// Get the current date.
    ///
    /// Optionally, an offset in hours is given.
    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        let offset = offset.unwrap_or(0);
        let offset = time::UtcOffset::from_hms(offset.try_into().ok()?, 0, 0).ok()?;
        let time = self.time.checked_to_offset(offset)?;
        Some(Datetime::Date(time.date()))
    }
}

fn retry<T, E>(mut f: impl FnMut() -> Result<T, E>) -> Result<T, E> {
    if let Ok(ok) = f() {
        Ok(ok)
    } else {
        f()
    }
}

// Printing diagnostics
// Source: https://github.com/typst/typst/blob/0da0165954e027ba48f7ba4a03e3b7b5b35ea8f6/crates/typst-cli/src/args.rs#L585
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, ValueEnum)]
pub enum DiagnosticFormat {
    #[default]
    Human,
    Short,
}

// Source: https://github.com/typst/typst/blob/0da0165954e027ba48f7ba4a03e3b7b5b35ea8f6/crates/typst-cli/src/compile.rs#L674
impl<'a> TypstWrapperWorld {
    fn lookup(&'a self, id: FileId) -> CodespanResult<Lines<String>> {
        if id == self.source.id() {
            return Ok(self.source.lines().to_owned());
        }
        Ok(self
            .file(id)
            .map_err(|_| codespan_reporting::files::Error::FileMissing)?
            .source(id)
            .map_err(|_| codespan_reporting::files::Error::FileMissing)?
            .lines()
            .to_owned())
    }
}
type CodespanResult<T> = Result<T, CodespanError>;
type CodespanError = codespan_reporting::files::Error;
impl<'a> codespan_reporting::files::Files<'a> for TypstWrapperWorld {
    type FileId = FileId;
    type Name = String;
    type Source = Lines<String>;

    fn name(&'a self, id: FileId) -> CodespanResult<Self::Name> {
        let vpath = id.vpath();
        Ok(if let Some(package) = id.package() {
            format!("{package}{}", vpath.as_rooted_path().display())
        } else {
            // Try to express the path relative to the working directory.
            vpath
                .resolve(self.root.as_path())
                .and_then(|abs| pathdiff::diff_paths(abs, self.workdir.as_path()))
                .as_deref()
                .unwrap_or_else(|| vpath.as_rootless_path())
                .to_string_lossy()
                .into()
        })
    }

    fn source(&'a self, id: FileId) -> CodespanResult<Self::Source> {
        Ok(self.lookup(id)?)
    }

    fn line_index(&'a self, id: FileId, given: usize) -> CodespanResult<usize> {
        let source_lines = self.lookup(id)?;
        source_lines
            .byte_to_line(given)
            .ok_or_else(|| CodespanError::IndexTooLarge {
                given,
                max: source_lines.len_bytes(),
            })
    }

    fn line_range(&'a self, id: FileId, given: usize) -> CodespanResult<std::ops::Range<usize>> {
        let source_lines = self.lookup(id)?;
        source_lines
            .line_to_range(given)
            .ok_or_else(|| CodespanError::LineTooLarge {
                given,
                max: source_lines.len_lines(),
            })
    }

    fn column_number(&'a self, id: FileId, _: usize, given: usize) -> CodespanResult<usize> {
        let source_lines = self.lookup(id)?;
        source_lines.byte_to_column(given).ok_or_else(|| {
            let max = source_lines.len_bytes();
            if given <= max {
                CodespanError::InvalidCharBoundary { given }
            } else {
                CodespanError::IndexTooLarge { given, max }
            }
        })
    }
}

// Source: https://github.com/typst/typst/blob/dd1e6e94f73db6a257a5ac34a6320e00410a2534/crates/typst-cli/src/compile.rs#L617 (v14.0.0)
pub fn render_diagnostics(
    world: &TypstWrapperWorld,
    errors: &[SourceDiagnostic],
    warnings: &[SourceDiagnostic],
    diagnostic_format: DiagnosticFormat,
) -> Result<String, codespan_reporting::files::Error> {
    let mut writer = term::termcolor::Ansi::new(Vec::new());

    let mut config = term::Config {
        tab_width: 2,
        ..Default::default()
    };
    if diagnostic_format == DiagnosticFormat::Short {
        config.display_style = term::DisplayStyle::Short;
    }

    for diagnostic in warnings.iter().chain(errors) {
        let diag = match diagnostic.severity {
            Severity::Error => Diagnostic::error(),
            Severity::Warning => Diagnostic::warning(),
        }
        .with_message(diagnostic.message.clone())
        .with_notes({
            let r = diagnostic
                .hints
                .iter()
                .map(|e| (eco_format!("hint: {e}")).into())
                .collect();
            r
        })
        .with_labels(label(world, diagnostic.span).into_iter().collect());

        term::emit_to_write_style(&mut writer, &config, world, &diag)?;

        // Stacktrace-like helper diagnostics.
        for point in &diagnostic.trace {
            let message = point.v.to_string();
            let help = Diagnostic::help()
                .with_message(message)
                .with_labels(label(world, point.span).into_iter().collect());

            term::emit_to_write_style(&mut writer, &config, world, &help)?;
        }
    }

    let output = String::from_utf8(writer.into_inner()).unwrap_or_default();
    Ok(output)
}

/// Create a label for a span.
fn label(world: &TypstWrapperWorld, span: Span) -> Option<Label<FileId>> {
    Some(Label::primary(span.id()?, world.range(span)?))
}
