//! Benchmark: old Node-based read_arrow vs new direct path (tjson_arrow).
//!
//! Measures timing and heap allocation using a tracking global allocator.
//! Tracks total bytes allocated (monotonically increasing → no underflow).
//!
//! Run with:
//!   cargo test -p ttoon-core --test bench_direct_path -- --ignored --nocapture --test-threads=1

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{BufReader, Cursor};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use indexmap::IndexMap;

use ttoon_core::ir::Node;
use ttoon_core::{
    FieldType, ScalarType, StreamSchema, TjsonArrowStreamReader, TjsonArrowStreamWriter,
    TjsonOptions, TjsonStreamReader, TjsonStreamWriter,
};

// ─── Tracking Allocator ──────────────────────────────────────────────────────
//
// Only tracks cumulative bytes allocated/freed (both monotonically increasing).
// Peak is derived from a high-water mark on (allocated - freed).

static TOTAL_ALLOC: AtomicUsize = AtomicUsize::new(0);
static TOTAL_FREED: AtomicUsize = AtomicUsize::new(0);
static PEAK_LIVE: AtomicUsize = AtomicUsize::new(0);

struct TrackingAllocator;

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let alloc = TOTAL_ALLOC.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            let freed = TOTAL_FREED.load(Ordering::Relaxed);
            let live = alloc.saturating_sub(freed);
            PEAK_LIVE.fetch_max(live, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        TOTAL_FREED.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

struct Snapshot {
    alloc: usize,
    freed: usize,
    peak: usize,
}

fn snapshot() -> Snapshot {
    // Force ordering: read alloc first, freed second
    let alloc = TOTAL_ALLOC.load(Ordering::SeqCst);
    let freed = TOTAL_FREED.load(Ordering::SeqCst);
    let peak = PEAK_LIVE.load(Ordering::SeqCst);
    Snapshot { alloc, freed, peak }
}

fn reset_peak() {
    let alloc = TOTAL_ALLOC.load(Ordering::SeqCst);
    let freed = TOTAL_FREED.load(Ordering::SeqCst);
    PEAK_LIVE.store(alloc.saturating_sub(freed), Ordering::SeqCst);
}

struct Measurement {
    total_alloc: usize,
    peak_live: usize,
}

fn measure_between(before: &Snapshot, after: &Snapshot) -> Measurement {
    Measurement {
        total_alloc: after.alloc.saturating_sub(before.alloc),
        peak_live: after.peak.saturating_sub(
            before
                .peak
                .saturating_sub(before.alloc.saturating_sub(before.freed)),
        ),
    }
}

fn format_bytes(bytes: usize) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.2} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn format_memory(measurement: &Measurement) -> String {
    format!(
        "heap_alloc={:<12} peak_live={}",
        format_bytes(measurement.total_alloc),
        format_bytes(measurement.peak_live),
    )
}

// ─── Data Generator ──────────────────────────────────────────────────────────

fn generate_tjson(num_rows: usize) -> String {
    let mut s = String::with_capacity(num_rows * 120);
    s.push_str("[\n");
    for i in 0..num_rows {
        if i > 0 {
            s.push_str(",\n");
        }
        let null_or_score = if i % 17 == 0 {
            "null".to_string()
        } else {
            format!("{:.2}", (i as f64) * 1.5)
        };
        s.push_str(&format!(
            r#"  {{"id": {}, "name": "user_{}", "score": {}, "active": {}}}"#,
            i,
            i,
            null_or_score,
            i % 2 == 0
        ));
    }
    s.push_str("\n]");
    s
}

fn generated_tjson_schema() -> StreamSchema {
    StreamSchema::new([
        ("id", FieldType::new(ScalarType::Int)),
        ("name", FieldType::new(ScalarType::String)),
        ("score", FieldType::nullable(ScalarType::Float)),
        ("active", FieldType::new(ScalarType::Bool)),
    ])
}

fn generated_rows(input: &str) -> Vec<IndexMap<String, Node>> {
    let node = ttoon_core::from_ttoon(input).unwrap();
    let Node::List(rows) = node else {
        panic!("expected top-level list");
    };

    rows.into_iter()
        .map(|row| match row {
            Node::Object(entries) => entries,
            _ => panic!("expected object row"),
        })
        .collect()
}

fn generated_rows_as_list_node(rows: &[IndexMap<String, Node>]) -> Node {
    Node::List(rows.iter().cloned().map(Node::Object).collect())
}

// ─── Benchmarks ──────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn bench_tokenizer_vs_token_iterator() {
    let sizes = [10_000, 100_000];

    for &num_rows in &sizes {
        let input = generate_tjson(num_rows);
        let input_size = input.len();

        println!("\n============================================================");
        println!(
            "Tokenizer vs TokenIterator — {} rows ({} input)",
            num_rows,
            format_bytes(input_size)
        );
        println!("============================================================");

        // Warmup
        {
            let _ = ttoon_core::tokenizer::Tokenizer::new(&input).tokenize();
            let _ = ttoon_core::tokenizer::TokenIterator::new(&input).count();
        }

        // ── Batch tokenize ──
        reset_peak();
        let before = snapshot();
        let t0 = Instant::now();
        let tokens = ttoon_core::tokenizer::Tokenizer::new(&input)
            .tokenize()
            .unwrap();
        let batch_time = t0.elapsed();
        let after = snapshot();
        let batch_m = measure_between(&before, &after);
        let token_count = tokens.len();
        drop(tokens);

        println!(
            "  Tokenizer::tokenize()     : {:>8.2?}  {}  tokens={}",
            batch_time,
            format_memory(&batch_m),
            token_count,
        );

        // ── Lazy iterator (consume without collecting) ──
        reset_peak();
        let before = snapshot();
        let t0 = Instant::now();
        let mut count = 0usize;
        for tok in ttoon_core::tokenizer::TokenIterator::new(&input) {
            tok.unwrap();
            count += 1;
        }
        let iter_time = t0.elapsed();
        let after = snapshot();
        let iter_m = measure_between(&before, &after);

        println!(
            "  TokenIterator (consume)   : {:>8.2?}  {}  tokens={}",
            iter_time,
            format_memory(&iter_m),
            count,
        );

        println!(
            "  → speed: {:.2}x  alloc ratio: {:.1}x  ({} → {})",
            batch_time.as_secs_f64() / iter_time.as_secs_f64(),
            batch_m.total_alloc as f64 / iter_m.total_alloc.max(1) as f64,
            format_bytes(batch_m.total_alloc),
            format_bytes(iter_m.total_alloc),
        );
    }
}

#[test]
#[ignore]
fn bench_read_arrow_direct() {
    let sizes = [10_000, 100_000];

    for &num_rows in &sizes {
        let input = generate_tjson(num_rows);
        let input_size = input.len();

        println!("\n============================================================");
        println!(
            "read_arrow: T-JSON direct path — {} rows ({} input)",
            num_rows,
            format_bytes(input_size)
        );
        println!("============================================================");

        // Warmup
        {
            let _ = ttoon_core::read_arrow(&input).unwrap();
        }

        // ── read_arrow: routes T-JSON to tjson_arrow direct path ──
        reset_peak();
        let before = snapshot();
        let t0 = Instant::now();
        let table = ttoon_core::read_arrow(&input).unwrap();
        let direct_time = t0.elapsed();
        let after = snapshot();
        let direct_m = measure_between(&before, &after);
        let rows = table.num_rows();
        let cols = table.schema.fields().len();
        drop(table);

        println!(
            "  read_arrow() direct       : {:>8.2?}  {}  rows={} cols={}",
            direct_time,
            format_memory(&direct_m),
            rows,
            cols,
        );
    }
}

// ─── from_ttoon(T-JSON) — TokenIterator baseline breakdown ──────────────────

#[test]
#[ignore]
fn bench_from_ttoon_tjson() {
    let sizes = [10_000, 100_000];

    for &num_rows in &sizes {
        let input = generate_tjson(num_rows);
        let input_size = input.len();

        println!("\n============================================================");
        println!(
            "from_ttoon(T-JSON) breakdown — {} rows ({} input)",
            num_rows,
            format_bytes(input_size)
        );
        println!("============================================================");

        // Warmup
        {
            let _ = ttoon_core::from_ttoon(&input).unwrap();
            let _ = ttoon_core::tokenizer::Tokenizer::new(&input).tokenize();
            let _ = ttoon_core::tokenizer::TokenIterator::new(&input).count();
        }

        // ── Full path: from_ttoon (T-JSON → Node AST) ──
        reset_peak();
        let before = snapshot();
        let t0 = Instant::now();
        let node = ttoon_core::from_ttoon(&input).unwrap();
        let full_time = t0.elapsed();
        let after = snapshot();
        let full_m = measure_between(&before, &after);
        drop(node);

        println!(
            "  from_ttoon() full path    : {:>8.2?}  {}",
            full_time,
            format_memory(&full_m),
        );

        // ── Tokenizer batch only (legacy reference) ──
        reset_peak();
        let before = snapshot();
        let t0 = Instant::now();
        let tokens = ttoon_core::tokenizer::Tokenizer::new(&input)
            .tokenize()
            .unwrap();
        let tok_time = t0.elapsed();
        let after = snapshot();
        let tok_m = measure_between(&before, &after);
        drop(tokens);

        println!(
            "  ├─ Tokenizer (legacy ref) : {:>8.2?}  {}",
            tok_time,
            format_memory(&tok_m),
        );

        // ── TokenIterator lazy only (current path baseline) ──
        reset_peak();
        let before = snapshot();
        let t0 = Instant::now();
        for tok in ttoon_core::tokenizer::TokenIterator::new(&input) {
            tok.unwrap();
        }
        let iter_time = t0.elapsed();
        let after = snapshot();
        let iter_m = measure_between(&before, &after);

        println!(
            "  ├─ TokenIterator (base)   : {:>8.2?}  {}",
            iter_time,
            format_memory(&iter_m),
        );

        // ── Breakdown ──
        let iter_pct = iter_m.total_alloc as f64 / full_m.total_alloc as f64 * 100.0;
        let rest_alloc = full_m.total_alloc.saturating_sub(iter_m.total_alloc);
        let legacy_extra_ratio = tok_m.total_alloc as f64 / iter_m.total_alloc.max(1) as f64;

        println!(
            "  └─ Rest (Node AST etc.)   :           {}",
            format!("heap_alloc={} peak_live=n/a", format_bytes(rest_alloc),),
        );
        println!();
        println!("  TokenIter share of total  : {:.1}%", iter_pct,);
        println!(
            "  Current alloc split       : {} = {} + {}",
            format_bytes(full_m.total_alloc),
            format_bytes(iter_m.total_alloc),
            format_bytes(rest_alloc),
        );
        println!(
            "  Legacy Vec<Token> ref     : {} vs {}  ({:.1}x larger)",
            format_bytes(tok_m.total_alloc),
            format_bytes(iter_m.total_alloc),
            legacy_extra_ratio,
        );
    }
}

// ─── tjson_to_ttoon() — TokenIterator baseline breakdown ────────────────────

#[test]
#[ignore]
fn bench_tjson_to_ttoon() {
    let sizes = [10_000, 100_000];

    for &num_rows in &sizes {
        let input = generate_tjson(num_rows);
        let input_size = input.len();

        println!("\n============================================================");
        println!(
            "tjson_to_ttoon() breakdown — {} rows ({} input)",
            num_rows,
            format_bytes(input_size)
        );
        println!("============================================================");

        // Warmup
        {
            let _ = ttoon_core::tjson_to_ttoon(&input, None).unwrap();
            let _ = ttoon_core::tokenizer::Tokenizer::new(&input).tokenize();
            let _ = ttoon_core::tokenizer::TokenIterator::new(&input).count();
        }

        // ── Full path: tjson_to_ttoon ──
        reset_peak();
        let before = snapshot();
        let t0 = Instant::now();
        let output = ttoon_core::tjson_to_ttoon(&input, None).unwrap();
        let full_time = t0.elapsed();
        let after = snapshot();
        let full_m = measure_between(&before, &after);
        let output_size = output.len();
        drop(output);

        println!(
            "  tjson_to_ttoon() full path: {:>8.2?}  {}  output={}",
            full_time,
            format_memory(&full_m),
            format_bytes(output_size),
        );

        // ── Tokenizer batch only (legacy reference) ──
        reset_peak();
        let before = snapshot();
        let t0 = Instant::now();
        let tokens = ttoon_core::tokenizer::Tokenizer::new(&input)
            .tokenize()
            .unwrap();
        let tok_time = t0.elapsed();
        let after = snapshot();
        let tok_m = measure_between(&before, &after);
        drop(tokens);

        println!(
            "  ├─ Tokenizer (legacy ref) : {:>8.2?}  {}",
            tok_time,
            format_memory(&tok_m),
        );

        // ── TokenIterator lazy only (current path baseline) ──
        reset_peak();
        let before = snapshot();
        let t0 = Instant::now();
        for tok in ttoon_core::tokenizer::TokenIterator::new(&input) {
            tok.unwrap();
        }
        let iter_time = t0.elapsed();
        let after = snapshot();
        let iter_m = measure_between(&before, &after);

        println!(
            "  ├─ TokenIterator (base)   : {:>8.2?}  {}",
            iter_time,
            format_memory(&iter_m),
        );

        // ── Breakdown ──
        let iter_pct = iter_m.total_alloc as f64 / full_m.total_alloc as f64 * 100.0;
        let rest_alloc = full_m.total_alloc.saturating_sub(iter_m.total_alloc);
        let legacy_extra_ratio = tok_m.total_alloc as f64 / iter_m.total_alloc.max(1) as f64;

        println!(
            "  └─ Rest (Node + serialize): {:>8.2?}  {}",
            full_time.saturating_sub(iter_time),
            format!("heap_alloc={} peak_live=n/a", format_bytes(rest_alloc),),
        );
        println!();
        println!("  TokenIter share of total  : {:.1}%", iter_pct,);
        println!(
            "  Current alloc split       : {} = {} + {}",
            format_bytes(full_m.total_alloc),
            format_bytes(iter_m.total_alloc),
            format_bytes(rest_alloc),
        );
        println!(
            "  Legacy Vec<Token> ref     : {} vs {}  ({:.1}x larger)",
            format_bytes(tok_m.total_alloc),
            format_bytes(iter_m.total_alloc),
            legacy_extra_ratio,
        );
    }
}

#[test]
#[ignore]
fn bench_tjson_stream_read_family() {
    let sizes = [10_000, 100_000];

    for &num_rows in &sizes {
        let input = generate_tjson(num_rows);
        let input_size = input.len();
        let schema = generated_tjson_schema();

        println!("\n============================================================");
        println!(
            "T-JSON streaming readers — {} rows ({} input)",
            num_rows,
            format_bytes(input_size)
        );
        println!("============================================================");

        {
            let _ = ttoon_core::from_ttoon(&input).unwrap();
            let mut row_reader = TjsonStreamReader::new(
                BufReader::new(Cursor::new(input.as_bytes())),
                schema.clone(),
            );
            while let Some(row) = row_reader.next() {
                row.unwrap();
            }

            let mut arrow_reader = TjsonArrowStreamReader::new(
                BufReader::new(Cursor::new(input.as_bytes())),
                schema.clone(),
                1024,
            )
            .unwrap();
            while let Some(batch) = arrow_reader.next() {
                batch.unwrap();
            }
        }

        reset_peak();
        let before = snapshot();
        let t0 = Instant::now();
        let node = ttoon_core::from_ttoon(&input).unwrap();
        let full_time = t0.elapsed();
        let after = snapshot();
        let full_m = measure_between(&before, &after);
        let full_rows = match &node {
            Node::List(rows) => rows.len(),
            _ => panic!("expected list"),
        };
        drop(node);

        println!(
            "  from_ttoon() full path    : {:>8.2?}  {}  rows={}",
            full_time,
            format_memory(&full_m),
            full_rows,
        );

        reset_peak();
        let before = snapshot();
        let t0 = Instant::now();
        let mut row_reader = TjsonStreamReader::new(
            BufReader::new(Cursor::new(input.as_bytes())),
            schema.clone(),
        );
        let mut row_count = 0usize;
        while let Some(row) = row_reader.next() {
            row.unwrap();
            row_count += 1;
        }
        let row_time = t0.elapsed();
        let after = snapshot();
        let row_m = measure_between(&before, &after);

        println!(
            "  TjsonStreamReader         : {:>8.2?}  {}  rows={}",
            row_time,
            format_memory(&row_m),
            row_count,
        );

        reset_peak();
        let before = snapshot();
        let t0 = Instant::now();
        let mut arrow_reader = TjsonArrowStreamReader::new(
            BufReader::new(Cursor::new(input.as_bytes())),
            schema.clone(),
            1024,
        )
        .unwrap();
        let mut arrow_rows = 0usize;
        while let Some(batch) = arrow_reader.next() {
            arrow_rows += batch.unwrap().num_rows();
        }
        let arrow_time = t0.elapsed();
        let after = snapshot();
        let arrow_m = measure_between(&before, &after);

        println!(
            "  TjsonArrowStreamReader    : {:>8.2?}  {}  rows={}",
            arrow_time,
            format_memory(&arrow_m),
            arrow_rows,
        );

        assert_eq!(row_count, full_rows);
        assert_eq!(arrow_rows, full_rows);
    }
}

#[test]
#[ignore]
fn bench_tjson_stream_write_family() {
    let sizes = [10_000, 100_000];

    for &num_rows in &sizes {
        let input = generate_tjson(num_rows);
        let input_size = input.len();
        let schema = generated_tjson_schema();
        let rows = generated_rows(&input);
        let list_node = generated_rows_as_list_node(&rows);
        let table = ttoon_core::read_arrow(&input).unwrap();

        let warmup_full_rows = ttoon_core::to_tjson(&list_node, None).unwrap();
        let warmup_full_arrow = ttoon_core::arrow_to_tjson(&table, None).unwrap();

        let mut warmup_row_out = Vec::new();
        let mut warmup_row_writer =
            TjsonStreamWriter::new(&mut warmup_row_out, schema.clone(), TjsonOptions::default());
        for row in &rows {
            warmup_row_writer.write(row).unwrap();
        }
        warmup_row_writer.close().unwrap();
        assert_eq!(String::from_utf8(warmup_row_out).unwrap(), warmup_full_rows);

        let mut warmup_arrow_out = Vec::new();
        let mut warmup_arrow_writer = TjsonArrowStreamWriter::new(
            &mut warmup_arrow_out,
            schema.clone(),
            TjsonOptions::default(),
        )
        .unwrap();
        warmup_arrow_writer.write_batch(&table.batches[0]).unwrap();
        warmup_arrow_writer.close().unwrap();
        assert_eq!(
            String::from_utf8(warmup_arrow_out).unwrap(),
            warmup_full_arrow
        );

        println!("\n============================================================");
        println!(
            "T-JSON streaming writers — {} rows ({} input)",
            num_rows,
            format_bytes(input_size)
        );
        println!("============================================================");

        reset_peak();
        let before = snapshot();
        let t0 = Instant::now();
        let full_rows_output = ttoon_core::to_tjson(&list_node, None).unwrap();
        let full_rows_time = t0.elapsed();
        let after = snapshot();
        let full_rows_m = measure_between(&before, &after);
        let full_rows_size = full_rows_output.len();
        drop(full_rows_output);

        println!(
            "  to_tjson(list rows)       : {:>8.2?}  {}  output={}",
            full_rows_time,
            format_memory(&full_rows_m),
            format_bytes(full_rows_size),
        );

        reset_peak();
        let before = snapshot();
        let t0 = Instant::now();
        let mut row_output = Vec::new();
        let mut row_writer =
            TjsonStreamWriter::new(&mut row_output, schema.clone(), TjsonOptions::default());
        for row in &rows {
            row_writer.write(row).unwrap();
        }
        row_writer.close().unwrap();
        let row_time = t0.elapsed();
        let after = snapshot();
        let row_m = measure_between(&before, &after);
        let row_size = row_output.len();
        drop(row_output);

        println!(
            "  TjsonStreamWriter         : {:>8.2?}  {}  output={}",
            row_time,
            format_memory(&row_m),
            format_bytes(row_size),
        );

        reset_peak();
        let before = snapshot();
        let t0 = Instant::now();
        let full_arrow_output = ttoon_core::arrow_to_tjson(&table, None).unwrap();
        let full_arrow_time = t0.elapsed();
        let after = snapshot();
        let full_arrow_m = measure_between(&before, &after);
        let full_arrow_size = full_arrow_output.len();
        drop(full_arrow_output);

        println!(
            "  arrow_to_tjson(table)     : {:>8.2?}  {}  output={}",
            full_arrow_time,
            format_memory(&full_arrow_m),
            format_bytes(full_arrow_size),
        );

        reset_peak();
        let before = snapshot();
        let t0 = Instant::now();
        let mut arrow_output = Vec::new();
        let mut arrow_writer =
            TjsonArrowStreamWriter::new(&mut arrow_output, schema.clone(), TjsonOptions::default())
                .unwrap();
        arrow_writer.write_batch(&table.batches[0]).unwrap();
        arrow_writer.close().unwrap();
        let arrow_time = t0.elapsed();
        let after = snapshot();
        let arrow_m = measure_between(&before, &after);
        let arrow_size = arrow_output.len();
        drop(arrow_output);

        println!(
            "  TjsonArrowStreamWriter    : {:>8.2?}  {}  output={}",
            arrow_time,
            format_memory(&arrow_m),
            format_bytes(arrow_size),
        );

        assert_eq!(row_size, full_rows_size);
        assert_eq!(arrow_size, full_arrow_size);
    }
}
