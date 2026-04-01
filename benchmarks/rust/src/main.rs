use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{sink, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use arrow_array::RecordBatch;
use arrow_ipc::reader::FileReader;
use arrow_select::concat::concat_batches;
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::{json, Value};
use ttoon_core::ir::{ArrowTable, Node};
use ttoon_core::{
    ArrowStreamReader, ArrowStreamWriter, StreamSchema, TjsonArrowStreamReader,
    TjsonArrowStreamWriter, TjsonOptions,
};

const DEFAULT_WARMUPS: u32 = 2;
const DEFAULT_ITERATIONS: u32 = 20;
const RUNNER_SCHEMA_VERSION: u32 = 1;

fn main() {
    let options = parse_args(env::args().skip(1).collect());
    if options.list_cases {
        println!(
            "{}",
            serde_json::to_string_pretty(&list_case_entries(
                options.variant.as_deref(),
                options.shape.as_deref(),
            ))
            .expect("serialize case entries"),
        );
        return;
    }

    let payload = run_benchmarks(&options);
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize runner payload"),
    );
}

#[derive(Debug)]
struct Options {
    dataset_root: PathBuf,
    variant: Option<String>,
    size: Option<String>,
    shape: Option<String>,
    case_name: Option<String>,
    warmups: u32,
    iterations: u32,
    benchmark_release: Option<String>,
    dataset_release: Option<u32>,
    list_cases: bool,
    trace_memory: bool,
}

#[derive(Debug, Serialize)]
struct CaseEntry {
    language: &'static str,
    variant: &'static str,
    shape: &'static str,
    case: &'static str,
}

#[derive(Debug, Clone)]
struct DatasetBundle {
    variant: String,
    size: String,
    row_count: Option<u64>,
    root: PathBuf,
    meta_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
struct HostEnvironment {
    os: String,
    os_release: Option<String>,
    platform: Option<String>,
    architecture: String,
    cpu_model: String,
    python_version: Option<String>,
    node_version: Option<String>,
    rust_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct BenchmarkStats {
    mean_ms: f64,
    median_ms: f64,
    min_ms: f64,
    max_ms: f64,
    stdev_ms: f64,
}

#[derive(Debug, Clone)]
struct ReleaseMetadata {
    benchmark_release: String,
    dataset_release: u32,
}

#[derive(Debug, Serialize)]
struct BenchmarkResult {
    variant: String,
    shape: String,
    size: String,
    row_count: Option<u64>,
    case: String,
    warmups: u32,
    iterations: u32,
    stats: BenchmarkStats,
    samples_ms: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    memory_trace_kb: Option<Vec<u64>>,
    input_hint: String,
}

#[derive(Debug, Serialize)]
struct RunnerPayload {
    schema_version: u32,
    benchmark_release: String,
    dataset_release: u32,
    language: &'static str,
    generated_at: String,
    filters: Value,
    environment: HostEnvironment,
    dataset_count: usize,
    results: Vec<BenchmarkResult>,
    issues: Vec<String>,
}

enum PreparedCase {
    JsonValue {
        func: Box<dyn Fn() -> Result<(), String>>,
        input_hint: String,
    },
    NodeValue {
        func: Box<dyn Fn() -> Result<(), String>>,
        input_hint: String,
    },
    ArrowValue {
        func: Box<dyn Fn() -> Result<(), String>>,
        input_hint: String,
    },
}

fn parse_args(args: Vec<String>) -> Options {
    let root = workspace_root()
        .join("benchmarks")
        .join("datasets")
        .join("prepared");
    let mut options = Options {
        dataset_root: root,
        variant: None,
        size: None,
        shape: None,
        case_name: None,
        warmups: DEFAULT_WARMUPS,
        iterations: DEFAULT_ITERATIONS,
        benchmark_release: None,
        dataset_release: None,
        list_cases: false,
        trace_memory: false,
    };

    let mut index = 0;
    while index < args.len() {
        let token = &args[index];
        if token == "--list-cases" {
            options.list_cases = true;
            index += 1;
            continue;
        }
        if token == "--trace-memory" {
            options.trace_memory = true;
            index += 1;
            continue;
        }

        let value = args
            .get(index + 1)
            .unwrap_or_else(|| panic!("missing value for argument: {token}"))
            .clone();
        match token.as_str() {
            "--dataset-root" => options.dataset_root = PathBuf::from(value),
            "--variant" => options.variant = Some(value),
            "--size" => options.size = Some(value),
            "--shape" => options.shape = Some(value),
            "--case" => options.case_name = Some(value),
            "--warmups" => options.warmups = value.parse().expect("warmups must be integer"),
            "--iterations" => {
                options.iterations = value.parse().expect("iterations must be integer")
            }
            "--benchmark-release" => options.benchmark_release = Some(value),
            "--dataset-release" => {
                options.dataset_release =
                    Some(value.parse().expect("dataset_release must be integer"))
            }
            _ => panic!("unknown argument: {token}"),
        }
        index += 2;
    }

    options
}

fn run_benchmarks(options: &Options) -> RunnerPayload {
    let release_metadata = load_release_metadata(options).expect("load benchmark release metadata");
    let datasets = discover_datasets(
        &options.dataset_root,
        options.variant.as_deref(),
        options.size.as_deref(),
    );
    let environment = collect_host_environment();
    let mut issues = Vec::new();
    let mut results = Vec::new();

    for bundle in &datasets {
        for (shape, cases) in case_matrix_for_variant(&bundle.variant) {
            if let Some(shape_filter) = options.shape.as_deref() {
                if shape_filter != shape {
                    continue;
                }
            }
            for case_name in cases {
                if let Some(case_filter) = options.case_name.as_deref() {
                    if case_filter != case_name {
                        continue;
                    }
                }
                let prepared = match prepare_case(bundle, shape, case_name, &mut issues) {
                    Some(value) => value,
                    None => continue,
                };

                let (func, input_hint) = match prepared {
                    PreparedCase::JsonValue { func, input_hint }
                    | PreparedCase::NodeValue { func, input_hint }
                    | PreparedCase::ArrowValue { func, input_hint } => (func, input_hint),
                };

                match measure_sync(
                    &*func,
                    options.warmups,
                    options.iterations,
                    options.trace_memory,
                ) {
                    Ok((samples_ms, stats, memory_trace_kb)) => results.push(BenchmarkResult {
                        variant: bundle.variant.clone(),
                        shape: shape.to_string(),
                        size: bundle.size.clone(),
                        row_count: bundle.row_count,
                        case: case_name.to_string(),
                        warmups: options.warmups,
                        iterations: options.iterations,
                        stats,
                        samples_ms,
                        memory_trace_kb,
                        input_hint,
                    }),
                    Err(error) => push_issue(
                        &mut issues,
                        format!(
                            "{}: {} / {} / {} failed: {}",
                            relative_to_workspace(&bundle.meta_path),
                            shape,
                            case_name,
                            bundle.size,
                            error
                        ),
                    ),
                }
            }
        }
    }

    RunnerPayload {
        schema_version: RUNNER_SCHEMA_VERSION,
        benchmark_release: release_metadata.benchmark_release,
        dataset_release: release_metadata.dataset_release,
        language: "rust",
        generated_at: current_timestamp(),
        filters: json!({
            "variant": options.variant,
            "size": options.size,
            "shape": options.shape,
            "case": options.case_name,
            "warmups": options.warmups,
            "iterations": options.iterations,
        }),
        environment,
        dataset_count: datasets.len(),
        results,
        issues,
    }
}

fn load_release_metadata(options: &Options) -> Result<ReleaseMetadata, String> {
    let manifest_root = workspace_root().join("benchmarks").join("manifests");
    let benchmark_manifest = parse_shell_scalars(&manifest_root.join("benchmark_release.sh"))?;
    let dataset_manifest = parse_shell_scalars(&manifest_root.join("datasets.sh"))?;

    let benchmark_release = required_scalar(
        &benchmark_manifest,
        "BENCHMARK_RELEASE",
        &manifest_root.join("benchmark_release.sh"),
    )?;
    let benchmark_dataset_release = required_scalar(
        &benchmark_manifest,
        "BENCHMARK_DATASET_RELEASE",
        &manifest_root.join("benchmark_release.sh"),
    )?;
    let dataset_release = required_scalar(
        &dataset_manifest,
        "DATASET_RELEASE",
        &manifest_root.join("datasets.sh"),
    )?;
    let benchmark_release_major = parse_benchmark_release_major(&benchmark_release)?;
    let dataset_release_number = parse_dataset_release(&dataset_release)?;

    if benchmark_release_major.to_string() != benchmark_dataset_release {
        return Err(format!(
            "BENCHMARK_RELEASE does not match BENCHMARK_DATASET_RELEASE: {} vs {}",
            benchmark_release, benchmark_dataset_release
        ));
    }
    if benchmark_dataset_release != dataset_release {
        return Err(format!(
            "BENCHMARK_DATASET_RELEASE does not match DATASET_RELEASE: {} vs {}",
            benchmark_dataset_release, dataset_release
        ));
    }
    if let Some(cli_benchmark_release) = &options.benchmark_release {
        if cli_benchmark_release != &benchmark_release {
            return Err(format!(
                "CLI benchmark_release does not match authoritative manifest: {} vs {}",
                cli_benchmark_release, benchmark_release
            ));
        }
    }
    if let Some(cli_dataset_release) = options.dataset_release {
        if cli_dataset_release != dataset_release_number {
            return Err(format!(
                "CLI dataset_release does not match authoritative manifest: {} vs {}",
                cli_dataset_release, dataset_release_number
            ));
        }
    }

    Ok(ReleaseMetadata {
        benchmark_release,
        dataset_release: dataset_release_number,
    })
}

fn parse_shell_scalars(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {}", relative_to_workspace(path), error))?;
    let mut assignments = BTreeMap::new();

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("declare -A ") {
            continue;
        }

        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };

        if !key
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
        {
            continue;
        }

        let value = raw_value.trim();
        if value.starts_with('(') {
            continue;
        }

        assignments.insert(key.to_string(), parse_shell_scalar(value)?);
    }

    Ok(assignments)
}

fn parse_shell_scalar(raw_value: &str) -> Result<String, String> {
    if raw_value.len() >= 2 {
        let bytes = raw_value.as_bytes();
        let first = bytes[0] as char;
        let last = bytes[bytes.len() - 1] as char;
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            return Ok(raw_value[1..raw_value.len() - 1].to_string());
        }
    }

    Err(format!("unsupported shell scalar: {}", raw_value))
}

fn required_scalar(
    assignments: &BTreeMap<String, String>,
    key: &str,
    path: &Path,
) -> Result<String, String> {
    assignments.get(key).cloned().ok_or_else(|| {
        format!(
            "{} missing required field: {}",
            relative_to_workspace(path),
            key
        )
    })
}

fn parse_benchmark_release_major(benchmark_release: &str) -> Result<u32, String> {
    let Some((major, minor)) = benchmark_release.split_once('.') else {
        return Err(format!("invalid BENCHMARK_RELEASE: {}", benchmark_release));
    };
    if major.is_empty()
        || minor.is_empty()
        || !major.chars().all(|ch| ch.is_ascii_digit())
        || !minor.chars().all(|ch| ch.is_ascii_digit())
    {
        return Err(format!("invalid BENCHMARK_RELEASE: {}", benchmark_release));
    }
    major.parse::<u32>().map_err(|error| {
        format!(
            "invalid BENCHMARK_RELEASE: {} ({})",
            benchmark_release, error
        )
    })
}

fn parse_dataset_release(dataset_release: &str) -> Result<u32, String> {
    dataset_release
        .parse::<u32>()
        .map_err(|error| format!("invalid DATASET_RELEASE: {} ({})", dataset_release, error))
}

fn list_case_entries(variant: Option<&str>, shape: Option<&str>) -> Vec<CaseEntry> {
    let mut rows = Vec::new();
    for (variant_name, shape_map) in case_matrix() {
        if let Some(variant_filter) = variant {
            if variant_filter != variant_name {
                continue;
            }
        }
        for (shape_name, cases) in shape_map {
            if let Some(shape_filter) = shape {
                if shape_filter != shape_name {
                    continue;
                }
            }
            for case_name in cases {
                rows.push(CaseEntry {
                    language: "rust",
                    variant: variant_name,
                    shape: shape_name,
                    case: case_name,
                });
            }
        }
    }
    rows
}

fn case_matrix() -> Vec<(&'static str, Vec<(&'static str, Vec<&'static str>)>)> {
    vec![
        (
            "js-basic",
            vec![
                (
                    "structure",
                    vec![
                        "json_serialize",
                        "json_deserialize",
                        "tjson_serialize",
                        "tjson_deserialize",
                        "ttoon_serialize",
                        "ttoon_deserialize",
                    ],
                ),
                (
                    "tabular",
                    vec![
                        "arrow_tjson_serialize",
                        "arrow_tjson_deserialize",
                        "arrow_tjson_stream_serialize",
                        "arrow_tjson_stream_deserialize",
                        "arrow_ttoon_serialize",
                        "arrow_ttoon_deserialize",
                        "arrow_ttoon_stream_serialize",
                        "arrow_ttoon_stream_deserialize",
                    ],
                ),
            ],
        ),
        (
            "extended",
            vec![
                ("structure", vec!["ttoon_serialize", "ttoon_deserialize"]),
                (
                    "tabular",
                    vec![
                        "arrow_ttoon_serialize",
                        "arrow_ttoon_deserialize",
                        "arrow_tjson_serialize",
                        "arrow_tjson_deserialize",
                        "arrow_tjson_stream_serialize",
                        "arrow_tjson_stream_deserialize",
                        "arrow_ttoon_stream_serialize",
                        "arrow_ttoon_stream_deserialize",
                    ],
                ),
            ],
        ),
    ]
}

fn case_matrix_for_variant(variant: &str) -> Vec<(&'static str, Vec<&'static str>)> {
    case_matrix()
        .into_iter()
        .find_map(|(variant_name, shapes)| (variant_name == variant).then_some(shapes))
        .unwrap_or_default()
}

fn discover_datasets(
    dataset_root: &Path,
    variant_filter: Option<&str>,
    size_filter: Option<&str>,
) -> Vec<DatasetBundle> {
    let mut rows = Vec::new();
    if !dataset_root.exists() {
        return rows;
    }

    let variants = match fs::read_dir(dataset_root) {
        Ok(entries) => entries,
        Err(_) => return rows,
    };

    for variant_entry in variants.flatten() {
        let variant_name = variant_entry.file_name().to_string_lossy().to_string();
        if let Some(filter) = variant_filter {
            if filter != variant_name {
                continue;
            }
        }
        let variant_path = variant_entry.path();
        if !variant_path.is_dir() {
            continue;
        }

        let sizes = match fs::read_dir(&variant_path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for size_entry in sizes.flatten() {
            let size_name = size_entry.file_name().to_string_lossy().to_string();
            if let Some(filter) = size_filter {
                if filter != size_name {
                    continue;
                }
            }
            let root = size_entry.path();
            let meta_path = root.join("meta.json");
            if !meta_path.is_file() {
                continue;
            }

            let row_count = fs::read_to_string(&meta_path)
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .and_then(|payload| payload.get("row_count").and_then(Value::as_u64));

            rows.push(DatasetBundle {
                variant: variant_name.clone(),
                size: size_name,
                row_count,
                root,
                meta_path,
            });
        }
    }

    rows
}

fn prepare_case(
    bundle: &DatasetBundle,
    shape: &str,
    case_name: &str,
    issues: &mut Vec<String>,
) -> Option<PreparedCase> {
    if case_name.starts_with("json_") {
        if shape != "structure" {
            return None;
        }

        let source_path = bundle.file_for("structure", "source.json");
        if !source_path.is_file() {
            push_issue(
                issues,
                format!(
                    "{}: missing structure/source.json, skipping {}",
                    relative_to_workspace(&bundle.meta_path),
                    case_name
                ),
            );
            return None;
        }

        if case_name == "json_serialize" {
            let value = match load_json_value(&source_path) {
                Ok(value) => value,
                Err(error) => {
                    push_issue(
                        issues,
                        format!(
                            "{}: failed to load {}: {}",
                            relative_to_workspace(&bundle.meta_path),
                            relative_to_workspace(&source_path),
                            error
                        ),
                    );
                    return None;
                }
            };
            return Some(PreparedCase::JsonValue {
                func: Box::new(move || {
                    serde_json::to_string(&value)
                        .map(|_| ())
                        .map_err(|error| format!("serde_json::to_string failed: {error}"))
                }),
                input_hint: relative_to_workspace(&source_path),
            });
        }

        let text = match load_text(&source_path) {
            Ok(text) => text,
            Err(error) => {
                push_issue(
                    issues,
                    format!(
                        "{}: failed to load {}: {}",
                        relative_to_workspace(&bundle.meta_path),
                        relative_to_workspace(&source_path),
                        error
                    ),
                );
                return None;
            }
        };
        return Some(PreparedCase::JsonValue {
            func: Box::new(move || {
                serde_json::from_str::<Value>(&text)
                    .map(|_| ())
                    .map_err(|error| format!("serde_json::from_str failed: {error}"))
            }),
            input_hint: relative_to_workspace(&source_path),
        });
    }

    if shape == "structure" {
        let source_path = bundle.file_for("structure", "source.json");
        let tjson_path = bundle.file_for("structure", "tjson.txt");
        let ttoon_path = bundle.file_for("structure", "ttoon.txt");

        match case_name {
            "tjson_serialize" => {
                let node = match load_object_source(&source_path) {
                    Ok(node) => node,
                    Err(error) => {
                        push_issue(
                            issues,
                            format!(
                                "{}: failed to load {}: {}",
                                relative_to_workspace(&bundle.meta_path),
                                relative_to_workspace(&source_path),
                                error
                            ),
                        );
                        return None;
                    }
                };
                Some(PreparedCase::NodeValue {
                    func: Box::new(move || {
                        ttoon_core::to_tjson(&node, None)
                            .map(|_| ())
                            .map_err(|error| format!("to_tjson failed: {}", error.message))
                    }),
                    input_hint: relative_to_workspace(&source_path),
                })
            }
            "tjson_deserialize" => {
                let text = match load_text(&tjson_path) {
                    Ok(text) => text,
                    Err(error) => {
                        push_issue(
                            issues,
                            format!(
                                "{}: failed to load {}: {}",
                                relative_to_workspace(&bundle.meta_path),
                                relative_to_workspace(&tjson_path),
                                error
                            ),
                        );
                        return None;
                    }
                };
                Some(PreparedCase::NodeValue {
                    func: Box::new(move || {
                        ttoon_core::from_ttoon(&text)
                            .map(|_| ())
                            .map_err(|error| format!("from_ttoon failed: {}", error.message))
                    }),
                    input_hint: relative_to_workspace(&tjson_path),
                })
            }
            "ttoon_serialize" => {
                let node = match load_object_source(&source_path) {
                    Ok(node) => node,
                    Err(error) => {
                        push_issue(
                            issues,
                            format!(
                                "{}: failed to load {}: {}",
                                relative_to_workspace(&bundle.meta_path),
                                relative_to_workspace(&source_path),
                                error
                            ),
                        );
                        return None;
                    }
                };
                Some(PreparedCase::NodeValue {
                    func: Box::new(move || {
                        ttoon_core::to_ttoon(&node, None)
                            .map(|_| ())
                            .map_err(|error| format!("to_ttoon failed: {}", error.message))
                    }),
                    input_hint: relative_to_workspace(&source_path),
                })
            }
            "ttoon_deserialize" => {
                let text = match load_text(&ttoon_path) {
                    Ok(text) => text,
                    Err(error) => {
                        push_issue(
                            issues,
                            format!(
                                "{}: failed to load {}: {}",
                                relative_to_workspace(&bundle.meta_path),
                                relative_to_workspace(&ttoon_path),
                                error
                            ),
                        );
                        return None;
                    }
                };
                Some(PreparedCase::NodeValue {
                    func: Box::new(move || {
                        ttoon_core::from_ttoon(&text)
                            .map(|_| ())
                            .map_err(|error| format!("from_ttoon failed: {}", error.message))
                    }),
                    input_hint: relative_to_workspace(&ttoon_path),
                })
            }
            _ => None,
        }
    } else {
        let source_arrow_path = bundle.file_for("tabular", "source.arrow");
        let table_tjson_path = bundle.file_for("tabular", "tjson.txt");
        let table_ttoon_path = bundle.file_for("tabular", "ttoon.txt");

        match case_name {
            "arrow_ttoon_stream_serialize" => {
                let table = match load_arrow_table(&source_arrow_path) {
                    Ok(table) => table,
                    Err(error) => {
                        push_issue(
                            issues,
                            format!(
                                "{}: failed to load {}: {}",
                                relative_to_workspace(&bundle.meta_path),
                                relative_to_workspace(&source_arrow_path),
                                error
                            ),
                        );
                        return None;
                    }
                };
                let stream_schema = match StreamSchema::from_arrow_schema(table.schema.as_ref()) {
                    Ok(schema) => schema,
                    Err(error) => {
                        push_issue(
                            issues,
                            format!(
                                "{}: failed to build StreamSchema from {}: {}",
                                relative_to_workspace(&bundle.meta_path),
                                relative_to_workspace(&source_arrow_path),
                                error.message
                            ),
                        );
                        return None;
                    }
                };
                Some(PreparedCase::ArrowValue {
                    func: Box::new(move || {
                        let mut output = sink();
                        let mut writer = ArrowStreamWriter::new(
                            &mut output,
                            stream_schema.clone(),
                            ttoon_core::TtoonOptions::default(),
                        )
                        .map_err(|error| {
                            format!("ArrowStreamWriter::new failed: {}", error.message)
                        })?;
                        for batch in &table.batches {
                            writer.write_batch(batch).map_err(|error| {
                                format!("write_batch failed: {}", error.message)
                            })?;
                        }
                        let _ = writer
                            .close()
                            .map_err(|error| format!("close failed: {}", error.message))?;
                        Ok(())
                    }),
                    input_hint: relative_to_workspace(&source_arrow_path),
                })
            }
            "arrow_tjson_stream_serialize" => {
                let table = match load_arrow_table(&source_arrow_path) {
                    Ok(table) => table,
                    Err(error) => {
                        push_issue(
                            issues,
                            format!(
                                "{}: failed to load {}: {}",
                                relative_to_workspace(&bundle.meta_path),
                                relative_to_workspace(&source_arrow_path),
                                error
                            ),
                        );
                        return None;
                    }
                };
                let stream_schema = match StreamSchema::from_arrow_schema(table.schema.as_ref()) {
                    Ok(schema) => schema,
                    Err(error) => {
                        push_issue(
                            issues,
                            format!(
                                "{}: failed to build StreamSchema from {}: {}",
                                relative_to_workspace(&bundle.meta_path),
                                relative_to_workspace(&source_arrow_path),
                                error.message
                            ),
                        );
                        return None;
                    }
                };
                Some(PreparedCase::ArrowValue {
                    func: Box::new(move || {
                        let mut output = sink();
                        let mut writer = TjsonArrowStreamWriter::new(
                            &mut output,
                            stream_schema.clone(),
                            TjsonOptions::default(),
                        )
                        .map_err(|error| {
                            format!("TjsonArrowStreamWriter::new failed: {}", error.message)
                        })?;
                        for batch in &table.batches {
                            writer.write_batch(batch).map_err(|error| {
                                format!("write_batch failed: {}", error.message)
                            })?;
                        }
                        let _ = writer
                            .close()
                            .map_err(|error| format!("close failed: {}", error.message))?;
                        Ok(())
                    }),
                    input_hint: relative_to_workspace(&source_arrow_path),
                })
            }
            "arrow_ttoon_stream_deserialize" => {
                let table = match load_arrow_table(&source_arrow_path) {
                    Ok(table) => table,
                    Err(error) => {
                        push_issue(
                            issues,
                            format!(
                                "{}: failed to load {}: {}",
                                relative_to_workspace(&bundle.meta_path),
                                relative_to_workspace(&source_arrow_path),
                                error
                            ),
                        );
                        return None;
                    }
                };
                let stream_schema = match StreamSchema::from_arrow_schema(table.schema.as_ref()) {
                    Ok(schema) => schema,
                    Err(error) => {
                        push_issue(
                            issues,
                            format!(
                                "{}: failed to build StreamSchema from {}: {}",
                                relative_to_workspace(&bundle.meta_path),
                                relative_to_workspace(&source_arrow_path),
                                error.message
                            ),
                        );
                        return None;
                    }
                };
                let input_path = table_ttoon_path.clone();
                Some(PreparedCase::ArrowValue {
                    func: Box::new(move || {
                        let file = File::open(&input_path)
                            .map_err(|error| format!("open stream input failed: {error}"))?;
                        let mut reader = ArrowStreamReader::new(
                            BufReader::new(file),
                            stream_schema.clone(),
                            1024,
                        )
                        .map_err(|error| {
                            format!("ArrowStreamReader::new failed: {}", error.message)
                        })?;
                        let mut row_count = 0usize;
                        while let Some(batch) = reader.next() {
                            let batch = batch.map_err(|error| {
                                format!("stream read failed: {}", error.message)
                            })?;
                            row_count += batch.num_rows();
                        }
                        let _ = row_count;
                        Ok(())
                    }),
                    input_hint: relative_to_workspace(&table_ttoon_path),
                })
            }
            "arrow_tjson_stream_deserialize" => {
                let table = match load_arrow_table(&source_arrow_path) {
                    Ok(table) => table,
                    Err(error) => {
                        push_issue(
                            issues,
                            format!(
                                "{}: failed to load {}: {}",
                                relative_to_workspace(&bundle.meta_path),
                                relative_to_workspace(&source_arrow_path),
                                error
                            ),
                        );
                        return None;
                    }
                };
                let stream_schema = match StreamSchema::from_arrow_schema(table.schema.as_ref()) {
                    Ok(schema) => schema,
                    Err(error) => {
                        push_issue(
                            issues,
                            format!(
                                "{}: failed to build StreamSchema from {}: {}",
                                relative_to_workspace(&bundle.meta_path),
                                relative_to_workspace(&source_arrow_path),
                                error.message
                            ),
                        );
                        return None;
                    }
                };
                let input_path = table_tjson_path.clone();
                Some(PreparedCase::ArrowValue {
                    func: Box::new(move || {
                        let file = File::open(&input_path)
                            .map_err(|error| format!("open stream input failed: {error}"))?;
                        let mut reader = TjsonArrowStreamReader::new(
                            BufReader::new(file),
                            stream_schema.clone(),
                            1024,
                        )
                        .map_err(|error| {
                            format!("TjsonArrowStreamReader::new failed: {}", error.message)
                        })?;
                        let mut row_count = 0usize;
                        while let Some(batch) = reader.next() {
                            let batch = batch.map_err(|error| {
                                format!("stream read failed: {}", error.message)
                            })?;
                            row_count += batch.num_rows();
                        }
                        let _ = row_count;
                        Ok(())
                    }),
                    input_hint: relative_to_workspace(&table_tjson_path),
                })
            }
            "arrow_tjson_serialize" => {
                let table = match load_arrow_table(&source_arrow_path) {
                    Ok(table) => table,
                    Err(error) => {
                        push_issue(
                            issues,
                            format!(
                                "{}: failed to load {}: {}",
                                relative_to_workspace(&bundle.meta_path),
                                relative_to_workspace(&source_arrow_path),
                                error
                            ),
                        );
                        return None;
                    }
                };
                Some(PreparedCase::ArrowValue {
                    func: Box::new(move || {
                        ttoon_core::arrow_to_tjson(&table, None)
                            .map(|_| ())
                            .map_err(|error| format!("arrow_to_tjson failed: {}", error.message))
                    }),
                    input_hint: relative_to_workspace(&source_arrow_path),
                })
            }
            "arrow_tjson_deserialize" => {
                let text = match load_text(&table_tjson_path) {
                    Ok(text) => text,
                    Err(error) => {
                        push_issue(
                            issues,
                            format!(
                                "{}: failed to load {}: {}",
                                relative_to_workspace(&bundle.meta_path),
                                relative_to_workspace(&table_tjson_path),
                                error
                            ),
                        );
                        return None;
                    }
                };
                Some(PreparedCase::ArrowValue {
                    func: Box::new(move || {
                        ttoon_core::read_arrow(&text)
                            .map(|_| ())
                            .map_err(|error| format!("read_arrow failed: {}", error.message))
                    }),
                    input_hint: relative_to_workspace(&table_tjson_path),
                })
            }
            "arrow_ttoon_serialize" => {
                let table = match load_arrow_table(&source_arrow_path) {
                    Ok(table) => table,
                    Err(error) => {
                        push_issue(
                            issues,
                            format!(
                                "{}: failed to load {}: {}",
                                relative_to_workspace(&bundle.meta_path),
                                relative_to_workspace(&source_arrow_path),
                                error
                            ),
                        );
                        return None;
                    }
                };
                Some(PreparedCase::ArrowValue {
                    func: Box::new(move || {
                        ttoon_core::arrow_to_ttoon(&table, None)
                            .map(|_| ())
                            .map_err(|error| format!("arrow_to_ttoon failed: {}", error.message))
                    }),
                    input_hint: relative_to_workspace(&source_arrow_path),
                })
            }
            "arrow_ttoon_deserialize" => {
                let text = match load_text(&table_ttoon_path) {
                    Ok(text) => text,
                    Err(error) => {
                        push_issue(
                            issues,
                            format!(
                                "{}: failed to load {}: {}",
                                relative_to_workspace(&bundle.meta_path),
                                relative_to_workspace(&table_ttoon_path),
                                error
                            ),
                        );
                        return None;
                    }
                };
                Some(PreparedCase::ArrowValue {
                    func: Box::new(move || {
                        ttoon_core::read_arrow(&text)
                            .map(|_| ())
                            .map_err(|error| format!("read_arrow failed: {}", error.message))
                    }),
                    input_hint: relative_to_workspace(&table_ttoon_path),
                })
            }
            _ => None,
        }
    }
}

fn measure_sync(
    func: &dyn Fn() -> Result<(), String>,
    warmups: u32,
    iterations: u32,
    trace_memory: bool,
) -> Result<(Vec<f64>, BenchmarkStats, Option<Vec<u64>>), String> {
    if iterations == 0 {
        return Err("iterations must be > 0".to_string());
    }

    for _ in 0..warmups {
        func()?;
    }

    let mut samples_ms = Vec::with_capacity(iterations as usize);
    let mut memory_trace_kb = trace_memory.then(|| Vec::with_capacity(iterations as usize));
    for _ in 0..iterations {
        let start = Instant::now();
        func()?;
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        samples_ms.push(elapsed_ms);
        if let Some(trace) = memory_trace_kb.as_mut() {
            trace.push(current_rss_kb()?);
        }
    }

    Ok((
        samples_ms.clone(),
        summarize_samples(&samples_ms),
        memory_trace_kb,
    ))
}

fn summarize_samples(samples_ms: &[f64]) -> BenchmarkStats {
    let mut sorted = samples_ms.to_vec();
    sorted.sort_by(|left, right| left.partial_cmp(right).unwrap());

    let mean_ms = samples_ms.iter().sum::<f64>() / samples_ms.len() as f64;
    let median_ms = if sorted.len() % 2 == 0 {
        let upper = sorted.len() / 2;
        (sorted[upper - 1] + sorted[upper]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    };
    let min_ms = *sorted.first().unwrap_or(&0.0);
    let max_ms = *sorted.last().unwrap_or(&0.0);
    let stdev_ms = if samples_ms.len() > 1 {
        let variance = samples_ms
            .iter()
            .map(|sample| {
                let delta = sample - mean_ms;
                delta * delta
            })
            .sum::<f64>()
            / (samples_ms.len() as f64 - 1.0);
        variance.sqrt()
    } else {
        0.0
    };

    BenchmarkStats {
        mean_ms,
        median_ms,
        min_ms,
        max_ms,
        stdev_ms,
    }
}

fn load_json_value(path: &Path) -> Result<Value, String> {
    let text = load_text(path)?;
    serde_json::from_str(&text).map_err(|error| format!("invalid JSON: {error}"))
}

fn load_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| error.to_string())
}

fn load_object_source(path: &Path) -> Result<Node, String> {
    let value = load_json_value(path)?;
    json_value_to_node(&value)
}

fn load_arrow_table(path: &Path) -> Result<ArrowTable, String> {
    let file = File::open(path).map_err(|error| format!("open arrow file failed: {error}"))?;
    let mut reader = FileReader::try_new(file, None)
        .map_err(|error| format!("open arrow ipc failed: {error}"))?;
    let schema = reader.schema();
    let mut batches = Vec::new();
    for batch in &mut reader {
        let batch = batch.map_err(|error| format!("read arrow record batch failed: {error}"))?;
        batches.push(batch);
    }

    let records = if batches.is_empty() {
        RecordBatch::new_empty(schema.clone())
    } else if batches.len() == 1 {
        batches.remove(0)
    } else {
        concat_batches(&schema, &batches)
            .map_err(|error| format!("concat arrow record batches failed: {error}"))?
    };

    Ok(ArrowTable {
        schema,
        batches: vec![records],
    })
}

fn json_value_to_node(value: &Value) -> Result<Node, String> {
    match value {
        Value::Null => Ok(Node::Null),
        Value::Bool(value) => Ok(Node::Bool(*value)),
        Value::Number(value) => {
            if let Some(number) = value.as_i64() {
                Ok(Node::Int(number))
            } else if let Some(number) = value.as_f64() {
                Ok(Node::Float(number))
            } else {
                Err(format!("unsupported JSON number: {value}"))
            }
        }
        Value::String(value) => Ok(Node::String(value.clone())),
        Value::Array(items) => {
            let mut nodes = Vec::with_capacity(items.len());
            for item in items {
                nodes.push(json_value_to_node(item)?);
            }
            Ok(Node::List(nodes))
        }
        Value::Object(map) => {
            if let Some(node) = typed_wrapper_to_node(map)? {
                return Ok(node);
            }
            let mut entries = IndexMap::new();
            for (key, item) in map {
                entries.insert(key.clone(), json_value_to_node(item)?);
            }
            Ok(Node::Object(entries))
        }
    }
}

fn typed_wrapper_to_node(map: &serde_json::Map<String, Value>) -> Result<Option<Node>, String> {
    if map.len() != 2 || !map.contains_key("$kind") || !map.contains_key("value") {
        return Ok(None);
    }

    let kind = map
        .get("$kind")
        .and_then(Value::as_str)
        .ok_or_else(|| "typed wrapper $kind must be string".to_string())?;
    let raw = map
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| "typed wrapper value must be string".to_string())?;

    let node = match kind {
        "decimal" => {
            let value = if raw.ends_with('m') {
                raw.to_string()
            } else {
                format!("{raw}m")
            };
            Node::Decimal(value)
        }
        "uuid" => Node::Uuid(raw.to_string()),
        "date" => Node::Date(raw.to_string()),
        "time" => Node::Time(raw.to_string()),
        "datetime" => Node::DateTime(raw.to_string()),
        _ => return Err(format!("unknown native source wrapper kind: {kind}")),
    };
    Ok(Some(node))
}

fn collect_host_environment() -> HostEnvironment {
    HostEnvironment {
        os: env::consts::OS.to_string(),
        os_release: command_output(&["uname", "-r"]),
        platform: command_output(&["uname", "-a"]),
        architecture: env::consts::ARCH.to_string(),
        cpu_model: detect_cpu_model(),
        python_version: command_output(&["python3", "--version"])
            .or_else(|| command_output(&["python", "--version"])),
        node_version: command_output(&["node", "--version"]),
        rust_version: command_output(&["rustc", "--version"]),
    }
}

impl DatasetBundle {
    fn file_for(&self, shape: &str, filename: &str) -> PathBuf {
        self.root.join(shape).join(filename)
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn relative_to_workspace(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

fn current_timestamp() -> String {
    command_output(&["date", "-u", "+%Y-%m-%dT%H:%M:%SZ"]).unwrap_or_else(|| "unknown".to_string())
}

fn detect_cpu_model() -> String {
    if cfg!(target_os = "macos") {
        return command_output(&["sysctl", "-n", "machdep.cpu.brand_string"])
            .unwrap_or_else(|| "unknown".to_string());
    }
    if cfg!(target_os = "linux") {
        if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
            for line in cpuinfo.lines() {
                if let Some(value) = line.strip_prefix("model name\t: ") {
                    return value.to_string();
                }
            }
        }
    }
    "unknown".to_string()
}

fn command_output(command: &[&str]) -> Option<String> {
    let (program, args) = command.split_first()?;
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let stderr = String::from_utf8(output.stderr).ok()?;
    let text = if stdout.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn current_rss_kb() -> Result<u64, String> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    let result = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if result != 0 {
        return Err(format!(
            "read current rss failed: {}",
            std::io::Error::last_os_error()
        ));
    }

    let usage = unsafe { usage.assume_init() };
    let rss = usage.ru_maxrss as u64;

    #[cfg(target_os = "macos")]
    {
        Ok(rss / 1024)
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(rss)
    }
}

fn push_issue(issues: &mut Vec<String>, issue: String) {
    if !issues.iter().any(|item| item == &issue) {
        issues.push(issue);
    }
}
