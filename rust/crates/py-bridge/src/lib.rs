use arrow_array::RecordBatch;
use indexmap::IndexMap;
use pyo3::create_exception;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDate, PyDateTime, PyDict, PyList, PyTime, PyTuple};
use pyo3::IntoPyObjectExt;
use pyo3_arrow::PyRecordBatch;
use std::fmt::Write as _;
use std::io::{self, BufRead, Read, Write};
use ttoon_core::ir::{ArrowTable, Node};
use ttoon_core::{
    ArrowStreamReader as CoreArrowStreamReader, ArrowStreamWriter as CoreArrowStreamWriter,
    BinaryFormat, Delimiter, FieldType, ParseMode, ScalarType, StreamReader as CoreStreamReader,
    StreamResult as CoreStreamResult, StreamSchema as CoreStreamSchema,
    StreamWriter as CoreStreamWriter, TjsonArrowStreamReader as CoreTjsonArrowStreamReader,
    TjsonArrowStreamWriter as CoreTjsonArrowStreamWriter, TjsonOptions,
    TjsonStreamReader as CoreTjsonStreamReader, TjsonStreamWriter as CoreTjsonStreamWriter,
    TtoonOptions,
};

create_exception!(_core, TranscodeError, pyo3::exceptions::PyValueError);

/// 快取 Python 型別的 type object 指標，避免每值重複探測。
/// 在 dumps()/to_tjson() 入口建構一次，遞迴傳遞。
struct TypeCache {
    // 基本型別（按 benchmark 出現頻率排列）
    bool_type: *mut pyo3::ffi::PyObject,
    int_type: *mut pyo3::ffi::PyObject,
    float_type: *mut pyo3::ffi::PyObject,
    str_type: *mut pyo3::ffi::PyObject,
    bytes_type: *mut pyo3::ffi::PyObject,
    // 容器
    list_type: *mut pyo3::ffi::PyObject,
    dict_type: *mut pyo3::ffi::PyObject,
    // datetime 系列
    datetime_type: *mut pyo3::ffi::PyObject,
    date_type: *mut pyo3::ffi::PyObject,
    time_type: *mut pyo3::ffi::PyObject,
    // 需要 import 的型別（模組未安裝時為 None）
    uuid_type: Option<*mut pyo3::ffi::PyObject>,
    decimal_type: Option<*mut pyo3::ffi::PyObject>,
}

impl TypeCache {
    fn new(py: Python<'_>) -> PyResult<Self> {
        let builtins = py.import("builtins")?;
        let datetime_mod = py.import("datetime")?;

        let uuid_type = py
            .import("uuid")
            .and_then(|m| m.getattr("UUID"))
            .ok()
            .map(|c| c.as_ptr());
        let decimal_type = py
            .import("decimal")
            .and_then(|m| m.getattr("Decimal"))
            .ok()
            .map(|c| c.as_ptr());

        Ok(TypeCache {
            bool_type: builtins.getattr("bool")?.as_ptr(),
            int_type: builtins.getattr("int")?.as_ptr(),
            float_type: builtins.getattr("float")?.as_ptr(),
            str_type: builtins.getattr("str")?.as_ptr(),
            bytes_type: builtins.getattr("bytes")?.as_ptr(),
            list_type: builtins.getattr("list")?.as_ptr(),
            dict_type: builtins.getattr("dict")?.as_ptr(),
            datetime_type: datetime_mod.getattr("datetime")?.as_ptr(),
            date_type: datetime_mod.getattr("date")?.as_ptr(),
            time_type: datetime_mod.getattr("time")?.as_ptr(),
            uuid_type,
            decimal_type,
        })
    }
}

fn format_uuid_from_int(uuid_int: u128) -> String {
    let time_low = (uuid_int >> 96) as u32;
    let time_mid = ((uuid_int >> 80) & 0xffff) as u16;
    let time_hi_and_version = ((uuid_int >> 64) & 0xffff) as u16;
    let clock_seq = ((uuid_int >> 48) & 0xffff) as u16;
    let node = (uuid_int & 0xffff_ffff_ffff) as u64;

    let mut buf = String::with_capacity(36);
    write!(
        &mut buf,
        "{time_low:08x}-{time_mid:04x}-{time_hi_and_version:04x}-{clock_seq:04x}-{node:012x}"
    )
    .unwrap();
    buf
}

fn exact_uuid_to_node(obj: &Bound<'_, PyAny>) -> PyResult<Node> {
    let uuid_int = obj.getattr("int")?.extract::<u128>()?;
    Ok(Node::Uuid(format_uuid_from_int(uuid_int)))
}

fn format_date_fields(year: i32, month: u8, day: u8) -> String {
    let mut buf = String::with_capacity(10);
    write!(&mut buf, "{year:04}-{month:02}-{day:02}").unwrap();
    buf
}

fn format_time_fields(hour: u8, minute: u8, second: u8, microsecond: u32) -> String {
    let mut buf = String::with_capacity(if microsecond == 0 { 8 } else { 15 });
    write!(&mut buf, "{hour:02}:{minute:02}:{second:02}").unwrap();
    if microsecond != 0 {
        write!(&mut buf, ".{microsecond:06}").unwrap();
    }
    buf
}

fn timedelta_total_micros(delta: &Bound<'_, PyAny>) -> PyResult<i64> {
    let days: i64 = delta.getattr("days")?.extract()?;
    let seconds: i64 = delta.getattr("seconds")?.extract()?;
    let micros: i64 = delta.getattr("microseconds")?.extract()?;
    Ok(((days * 86_400) + seconds) * 1_000_000 + micros)
}

fn push_offset_suffix(buf: &mut String, total_micros: i64) {
    let sign = if total_micros < 0 { '-' } else { '+' };
    let abs = total_micros.abs();
    let hours = abs / 3_600_000_000;
    let minutes = (abs / 60_000_000) % 60;
    let seconds = (abs / 1_000_000) % 60;
    let micros = abs % 1_000_000;

    write!(buf, "{sign}{hours:02}:{minutes:02}").unwrap();
    if seconds != 0 || micros != 0 {
        write!(buf, ":{seconds:02}").unwrap();
    }
    if micros != 0 {
        write!(buf, ".{micros:06}").unwrap();
    }
}

fn exact_date_to_node(date: &Bound<'_, PyDate>) -> PyResult<Node> {
    let year: i32 = date.getattr("year")?.extract()?;
    let month: u8 = date.getattr("month")?.extract()?;
    let day: u8 = date.getattr("day")?.extract()?;
    Ok(Node::Date(format_date_fields(year, month, day)))
}

fn exact_time_to_node(time: &Bound<'_, PyTime>) -> PyResult<Node> {
    let tzinfo = time.getattr("tzinfo")?;
    if !tzinfo.is_none() {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "TP Time does not allow tzinfo; convert to a naive (timezone-free) time first",
        ));
    }
    let hour: u8 = time.getattr("hour")?.extract()?;
    let minute: u8 = time.getattr("minute")?.extract()?;
    let second: u8 = time.getattr("second")?.extract()?;
    let microsecond: u32 = time.getattr("microsecond")?.extract()?;
    Ok(Node::Time(format_time_fields(
        hour,
        minute,
        second,
        microsecond,
    )))
}

fn exact_datetime_to_node(datetime: &Bound<'_, PyDateTime>) -> PyResult<Node> {
    let year: i32 = datetime.getattr("year")?.extract()?;
    let month: u8 = datetime.getattr("month")?.extract()?;
    let day: u8 = datetime.getattr("day")?.extract()?;
    let hour: u8 = datetime.getattr("hour")?.extract()?;
    let minute: u8 = datetime.getattr("minute")?.extract()?;
    let second: u8 = datetime.getattr("second")?.extract()?;
    let microsecond: u32 = datetime.getattr("microsecond")?.extract()?;

    let mut buf = String::with_capacity(32);
    buf.push_str(&format_date_fields(year, month, day));
    buf.push('T');
    buf.push_str(&format_time_fields(hour, minute, second, microsecond));

    let offset = datetime.call_method0("utcoffset")?;
    if !offset.is_none() {
        push_offset_suffix(&mut buf, timedelta_total_micros(&offset)?);
    }

    Ok(Node::DateTime(buf))
}

fn map_core_err(err: ttoon_core::Error) -> PyErr {
    if let Some(transcode) = err.transcode.as_ref() {
        return Python::attach(|py| {
            let py_err = PyErr::new::<TranscodeError, _>(err.message.clone());
            let value = py_err.value(py);
            let source = PyDict::new(py);
            let _ = source.set_item("kind", transcode.source_kind.as_str());
            let _ = source.set_item("message", transcode.source.message.as_str());
            if let Some(span) = transcode.source.span {
                let span_dict = PyDict::new(py);
                let _ = span_dict.set_item("offset", span.offset);
                let _ = span_dict.set_item("line", span.line);
                let _ = span_dict.set_item("column", span.column);
                let _ = source.set_item("span", span_dict);
            } else {
                let _ = source.set_item("span", py.None());
            }
            let _ = value.setattr("operation", transcode.operation.as_str());
            let _ = value.setattr("phase", transcode.phase.as_str());
            let _ = value.setattr("source_kind", transcode.source_kind.as_str());
            let _ = value.setattr("source_message", transcode.source.message.as_str());
            let _ = value.setattr("source", source);
            py_err
        });
    }

    PyErr::new::<pyo3::exceptions::PyValueError, _>(err.message)
}

/// 序列化 Python 物件（或 PyArrow Table/RecordBatch）為 T-TOON 格式字串
///
/// 自動偵測：PyArrow Table/RecordBatch → Arrow 路徑；其他 Python 物件 → Node 路徑
/// delimiter: "," (預設) / "\t" / "|"
#[pyfunction]
#[pyo3(signature = (obj, delimiter=None, indent_size=None, binary_format=None))]
fn dumps(
    obj: &Bound<'_, PyAny>,
    delimiter: Option<&str>,
    indent_size: Option<u8>,
    binary_format: Option<&str>,
) -> PyResult<String> {
    let opts = build_ttoon_options(delimiter, indent_size, binary_format)?;
    let opts_ref = opts.as_ref();

    let is_arrow =
        obj.extract::<PyRecordBatch>().is_ok() || obj.hasattr("to_batches").unwrap_or(false);

    if is_arrow {
        let arrow_table = pyarrow_to_arrow_table(obj)?;
        return ttoon_core::arrow_to_ttoon(&arrow_table, opts_ref).map_err(map_core_err);
    }

    let cache = TypeCache::new(obj.py())?;
    let node = python_to_node_cached(obj, &cache)?;
    ttoon_core::to_ttoon(&node, opts_ref).map_err(map_core_err)
}

/// 反序列化 T-TOON/T-JSON 字串為 Python 物件
///
/// 自動偵測格式（T-JSON 或 T-TOON），支援頂層純量回傳
#[pyfunction]
#[pyo3(signature = (text, mode=None))]
fn loads(py: Python<'_>, text: String, mode: Option<&str>) -> PyResult<Py<PyAny>> {
    let parse_mode = parse_mode_from_str(mode)?;
    let node = ttoon_core::from_ttoon_with_mode(&text, parse_mode).map_err(map_core_err)?;
    node_to_python(py, &node)
}

/// 將 T-TOON/T-JSON 字串解析為 pyarrow RecordBatch（零複製 Arrow 路徑）
///
/// 輸入必須為 list-of-uniform-objects 結構；Python 層可再包裝為 pyarrow.Table
#[pyfunction]
fn read_arrow(py: Python<'_>, text: String) -> PyResult<Py<PyAny>> {
    let table = ttoon_core::read_arrow(&text).map_err(map_core_err)?;
    // read_arrow always produces single-batch; take the first batch for Python
    let batch = table.batches.into_iter().next().ok_or_else(|| {
        map_core_err(ttoon_core::Error::new(
            ttoon_core::ErrorKind::ArrowError,
            "read_arrow returned empty batches",
            None,
        ))
    })?;
    let py_batch = PyRecordBatch::new(batch);
    py_batch.into_pyarrow(py).map(|b| b.unbind())
}

/// 序列化 Python 物件為 T-JSON 格式字串（不接受 Arrow 輸入）
///
/// 輸出 JSON-like 的 {}/[] 括號格式，值層使用 typed 語法（uuid(...)、123.45m 等）
/// Arrow 輸入請使用 stringify_arrow_tjson()
#[pyfunction]
#[pyo3(signature = (obj, binary_format=None))]
fn to_tjson(obj: &Bound<'_, PyAny>, binary_format: Option<&str>) -> PyResult<String> {
    let opts = build_tjson_options(binary_format)?;
    let cache = TypeCache::new(obj.py())?;
    let node = python_to_node_cached(obj, &cache)?;
    ttoon_core::to_tjson(&node, Some(&opts)).map_err(map_core_err)
}

/// 序列化 PyArrow Table/RecordBatch 為 T-JSON 格式字串（Arrow 獨立入口）
///
/// 輸出 JSON array of objects: [{...}, ...]
#[pyfunction]
#[pyo3(signature = (obj, binary_format=None))]
fn stringify_arrow_tjson(obj: &Bound<'_, PyAny>, binary_format: Option<&str>) -> PyResult<String> {
    let opts = build_tjson_options(binary_format)?;
    let arrow_table = pyarrow_to_arrow_table(obj)?;
    ttoon_core::arrow_to_tjson(&arrow_table, Some(&opts)).map_err(map_core_err)
}

/// T-JSON → T-TOON direct transcode（不經 Python 物件中轉）
///
/// 走專用 T-JSON parser，T-JSON 路徑固定 strict，不接受 mode。
#[pyfunction]
#[pyo3(signature = (text, delimiter=None, indent_size=None, binary_format=None))]
fn tjson_to_ttoon(
    text: String,
    delimiter: Option<&str>,
    indent_size: Option<u8>,
    binary_format: Option<&str>,
) -> PyResult<String> {
    let opts = build_ttoon_options(delimiter, indent_size, binary_format)?;
    ttoon_core::tjson_to_ttoon(&text, opts.as_ref()).map_err(map_core_err)
}

/// T-TOON → T-JSON direct transcode（不經 Python 物件中轉）
///
/// 走 T-TOON parser，mode 預設 compat。
#[pyfunction]
#[pyo3(signature = (text, mode=None, binary_format=None))]
fn ttoon_to_tjson(
    text: String,
    mode: Option<&str>,
    binary_format: Option<&str>,
) -> PyResult<String> {
    let parse_mode = parse_mode_from_str(mode)?;
    let opts = build_tjson_options(binary_format)?;
    ttoon_core::ttoon_to_tjson(&text, parse_mode, Some(&opts)).map_err(map_core_err)
}

/// 偵測輸入字串的格式（"tjson"、"ttoon" 或 "typed_unit"）
#[pyfunction]
fn detect_format(text: String) -> PyResult<String> {
    let format = ttoon_core::format_detect::detect(&text);
    match format {
        ttoon_core::format_detect::Format::Tjson => Ok("tjson".to_string()),
        ttoon_core::format_detect::Format::Ttoon => Ok("ttoon".to_string()),
        ttoon_core::format_detect::Format::TypedUnit => Ok("typed_unit".to_string()),
    }
}

fn build_ttoon_options(
    delimiter: Option<&str>,
    indent_size: Option<u8>,
    binary_format: Option<&str>,
) -> PyResult<Option<TtoonOptions>> {
    if delimiter.is_none() && indent_size.is_none() && binary_format.is_none() {
        return Ok(None);
    }
    let mut eo = TtoonOptions::default();
    if let Some(d) = delimiter {
        eo.delimiter = Delimiter::parse(d).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "unknown delimiter '{}': expected ',', '\\t' or '|'",
                d
            ))
        })?;
    }
    if let Some(is) = indent_size {
        eo.indent_size = is;
    }
    if let Some(bf) = binary_format {
        eo.binary_format = BinaryFormat::parse(bf).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "unknown binary_format '{}': expected 'hex' or 'b64'",
                bf
            ))
        })?;
    }
    Ok(Some(eo))
}

fn build_stream_ttoon_options(
    delimiter: Option<&str>,
    binary_format: Option<&str>,
) -> PyResult<TtoonOptions> {
    let mut eo = TtoonOptions::default();
    if let Some(d) = delimiter {
        eo.delimiter = Delimiter::parse(d).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "unknown delimiter '{}': expected ',', '\\t' or '|'",
                d
            ))
        })?;
    }
    if let Some(bf) = binary_format {
        eo.binary_format = BinaryFormat::parse(bf).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "unknown binary_format '{}': expected 'hex' or 'b64'",
                bf
            ))
        })?;
    }
    Ok(eo)
}

fn build_tjson_options(binary_format: Option<&str>) -> PyResult<TjsonOptions> {
    let mut opts = TjsonOptions::default();
    if let Some(bf) = binary_format {
        opts.binary_format = BinaryFormat::parse(bf).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "unknown binary_format '{}': expected 'hex' or 'b64'",
                bf
            ))
        })?;
    }
    Ok(opts)
}

fn parse_mode_from_str(mode: Option<&str>) -> PyResult<ParseMode> {
    match mode.unwrap_or("compat") {
        "compat" => Ok(ParseMode::Compat),
        "strict" => Ok(ParseMode::Strict),
        other => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
            "unknown mode '{}': expected 'compat' or 'strict'",
            other
        ))),
    }
}

fn python_to_node(obj: &Bound<'_, PyAny>) -> PyResult<Node> {
    if obj.is_none() {
        return Ok(Node::Null);
    }

    let py = obj.py();

    // 檢測 datetime 物件（必須在 date 之前，因為 datetime 是 date 的子類）
    if let Ok(dt) = obj.downcast::<PyDateTime>() {
        let datetime_str: String = dt.call_method0("isoformat")?.extract()?;
        return Ok(Node::DateTime(datetime_str));
    }

    // 檢測 date 物件
    if let Ok(d) = obj.downcast::<PyDate>() {
        let date_str: String = d.call_method0("isoformat")?.extract()?;
        return Ok(Node::Date(date_str));
    }

    // 檢測 time 物件
    if let Ok(t) = obj.downcast::<PyTime>() {
        let tzinfo = t.getattr("tzinfo")?;
        if !tzinfo.is_none() {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "TP Time does not allow tzinfo; convert to a naive (timezone-free) time first",
            ));
        }
        let time_str: String = t.call_method0("isoformat")?.extract()?;
        return Ok(Node::Time(time_str));
    }

    // 檢測 UUID（必須在 float 之前，避免被轉換）
    if let Ok(uuid_module) = py.import("uuid") {
        if let Ok(uuid_class) = uuid_module.getattr("UUID") {
            if obj.is_instance(&uuid_class)? {
                if obj.get_type().as_ptr() == uuid_class.as_ptr() {
                    return exact_uuid_to_node(obj);
                }
                let uuid_str: String = obj.call_method0("__str__")?.extract()?;
                return Ok(Node::Uuid(uuid_str));
            }
        }
    }

    // 檢測 Decimal（必須在 float 之前，避免被轉換為 float）
    if let Ok(decimal_module) = py.import("decimal") {
        if let Ok(decimal_class) = decimal_module.getattr("Decimal") {
            if obj.is_instance(&decimal_class)? {
                let mut decimal_str: String = obj.call_method0("__str__")?.extract()?;
                decimal_str.push('m');
                return Ok(Node::Decimal(decimal_str));
            }
        }
    }

    // 檢測基本類型
    if let Ok(b) = obj.extract::<bool>() {
        return Ok(Node::Bool(b));
    }
    if let Ok(i) = obj.extract::<i64>() {
        return Ok(Node::Int(i));
    }
    if let Ok(f) = obj.extract::<f64>() {
        return Ok(Node::Float(f));
    }

    // 檢測 bytes 物件（必須在 string 之前）
    if let Ok(bytes_obj) = obj.downcast::<PyBytes>() {
        let bytes: &[u8] = bytes_obj.as_bytes();
        return Ok(Node::Binary(bytes.to_vec()));
    }

    if let Ok(s) = obj.extract::<String>() {
        return Ok(Node::String(s));
    }

    if let Ok(list) = obj.downcast::<PyList>() {
        let mut nodes = Vec::new();
        for item in list.iter() {
            nodes.push(python_to_node(&item)?);
        }
        return Ok(Node::List(nodes));
    }

    if let Ok(dict) = obj.downcast::<PyDict>() {
        let mut map = IndexMap::new();
        for (key, value) in dict.iter() {
            let key_str = key.extract::<String>()?;
            map.insert(key_str, python_to_node(&value)?);
        }
        return Ok(Node::Object(map));
    }

    Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
        "Unsupported type for serialization",
    ))
}

/// TypeCache 版本：用 type pointer 比較取代 sequential downcast 鏈。
/// 未命中快取的型別（含子類）回退 python_to_node()。
fn python_to_node_cached(obj: &Bound<'_, PyAny>, cache: &TypeCache) -> PyResult<Node> {
    if obj.is_none() {
        return Ok(Node::Null);
    }

    let type_ptr = obj.get_type().as_ptr();

    // bool 必須在 int 之前（Python 中 bool 是 int 子類，但 type_ptr 精確匹配不會混淆）
    if type_ptr == cache.bool_type {
        return Ok(Node::Bool(obj.extract::<bool>()?));
    }
    if type_ptr == cache.int_type {
        return Ok(Node::Int(obj.extract::<i64>()?));
    }
    if type_ptr == cache.float_type {
        return Ok(Node::Float(obj.extract::<f64>()?));
    }
    if type_ptr == cache.str_type {
        return Ok(Node::String(obj.extract::<String>()?));
    }

    // datetime 必須在 date 之前（datetime 是 date 子類，但 type_ptr 精確匹配不會混淆）
    if type_ptr == cache.datetime_type {
        let dt = obj.downcast::<PyDateTime>()?;
        return exact_datetime_to_node(&dt);
    }
    if type_ptr == cache.date_type {
        let d = obj.downcast::<PyDate>()?;
        return exact_date_to_node(&d);
    }
    if type_ptr == cache.time_type {
        let t = obj.downcast::<PyTime>()?;
        return exact_time_to_node(&t);
    }

    // uuid / decimal — 快取 class pointer，不再每值 import
    if let Some(uuid_ptr) = cache.uuid_type {
        if type_ptr == uuid_ptr {
            return exact_uuid_to_node(obj);
        }
    }
    if let Some(decimal_ptr) = cache.decimal_type {
        if type_ptr == decimal_ptr {
            let mut s: String = obj.call_method0("__str__")?.extract()?;
            s.push('m');
            return Ok(Node::Decimal(s));
        }
    }

    if type_ptr == cache.bytes_type {
        let bytes_obj = obj.downcast::<PyBytes>()?;
        return Ok(Node::Binary(bytes_obj.as_bytes().to_vec()));
    }

    // 容器型別 — 遞迴傳遞 cache
    if type_ptr == cache.list_type {
        let list = obj.downcast::<PyList>()?;
        let mut nodes = Vec::with_capacity(list.len());
        for item in list.iter() {
            nodes.push(python_to_node_cached(&item, cache)?);
        }
        return Ok(Node::List(nodes));
    }
    if type_ptr == cache.dict_type {
        let dict = obj.downcast::<PyDict>()?;
        let mut map = IndexMap::new();
        for (key, value) in dict.iter() {
            let key_str = key.extract::<String>()?;
            map.insert(key_str, python_to_node_cached(&value, cache)?);
        }
        return Ok(Node::Object(map));
    }

    // Fallback：子類或未知型別 → 回退原始 downcast 鏈
    python_to_node(obj)
}

fn node_to_python(py: Python<'_>, node: &Node) -> PyResult<Py<PyAny>> {
    match node {
        Node::Null => Ok(py.None()),
        Node::Bool(b) => Ok(b.into_bound_py_any(py)?.unbind()),
        Node::Int(i) => Ok(i.into_bound_py_any(py)?.unbind()),
        Node::Float(f) => Ok(f.into_bound_py_any(py)?.unbind()),
        Node::String(s) => Ok(s.into_bound_py_any(py)?.unbind()),
        Node::Date(date_str) => {
            let datetime_module = py.import("datetime")?;
            let date_class = datetime_module.getattr("date")?;
            let py_date = date_class
                .call_method1("fromisoformat", (date_str,))
                .map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                        "Failed to parse date '{}': {}",
                        date_str, e
                    ))
                })?;
            Ok(py_date.into())
        }
        Node::Time(time_str) => {
            let datetime_module = py.import("datetime")?;
            let time_class = datetime_module.getattr("time")?;
            let py_time = time_class
                .call_method1("fromisoformat", (time_str,))
                .map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                        "Failed to parse time '{}': {}",
                        time_str, e
                    ))
                })?;
            Ok(py_time.into())
        }
        Node::DateTime(datetime_str) => {
            let datetime_module = py.import("datetime")?;
            let datetime_class = datetime_module.getattr("datetime")?;
            let py_datetime = datetime_class
                .call_method1("fromisoformat", (datetime_str,))
                .map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                        "Failed to parse datetime '{}': {}",
                        datetime_str, e
                    ))
                })?;
            Ok(py_datetime.into())
        }
        Node::Binary(bytes) => {
            let py_bytes = PyBytes::new(py, bytes);
            Ok(py_bytes.into_any().unbind())
        }
        Node::Uuid(uuid_str) => {
            let uuid_module = py.import("uuid")?;
            let uuid_class = uuid_module.getattr("UUID")?;
            let py_uuid = uuid_class.call1((uuid_str,)).map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Failed to parse UUID '{}': {}",
                    uuid_str, e
                ))
            })?;
            Ok(py_uuid.unbind())
        }
        Node::Decimal(decimal_str) => {
            let decimal_module = py.import("decimal")?;
            let decimal_class = decimal_module.getattr("Decimal")?;
            let value_str = decimal_str.strip_suffix('m').ok_or_else(|| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Invalid decimal format '{}': missing 'm' suffix",
                    decimal_str
                ))
            })?;
            let py_decimal = decimal_class.call1((value_str,)).map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Failed to parse decimal '{}': {}",
                    value_str, e
                ))
            })?;
            Ok(py_decimal.unbind())
        }
        Node::List(items) => {
            let list = PyList::empty(py);
            for item in items {
                list.append(node_to_python(py, item)?)?;
            }
            Ok(list.into_any().unbind())
        }
        Node::Object(items) => {
            let dict = PyDict::new(py);
            for (k, v) in items {
                dict.set_item(k, node_to_python(py, v)?)?;
            }
            Ok(dict.into_any().unbind())
        }
    }
}

fn row_to_python(py: Python<'_>, row: &IndexMap<String, Node>) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    for (key, value) in row {
        dict.set_item(key, node_to_python(py, value)?)?;
    }
    Ok(dict.into_any().unbind())
}

fn python_to_row(obj: &Bound<'_, PyAny>, cache: &TypeCache) -> PyResult<IndexMap<String, Node>> {
    let dict = obj.downcast::<PyDict>().map_err(|_| {
        PyErr::new::<pyo3::exceptions::PyTypeError, _>("stream row must be a dict[str, Any]")
    })?;
    let mut map = IndexMap::new();
    for (key, value) in dict.iter() {
        let key_str = key.extract::<String>()?;
        map.insert(key_str, python_to_node_cached(&value, cache)?);
    }
    Ok(map)
}

fn extract_stream_schema(schema_obj: &Bound<'_, PyAny>) -> PyResult<CoreStreamSchema> {
    let iter = schema_obj.try_iter().map_err(|_| {
        PyErr::new::<pyo3::exceptions::PyTypeError, _>(
            "stream schema must be an iterable of (name, field_type) pairs",
        )
    })?;

    let mut fields = Vec::new();
    for item in iter {
        let item = item?;
        let pair = item.downcast::<PyTuple>().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "stream schema items must be 2-tuples of (name, field_type)",
            )
        })?;
        if pair.len() != 2 {
            return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "stream schema items must be 2-tuples of (name, field_type)",
            ));
        }
        let name = pair.get_item(0)?.extract::<String>()?;
        let field_type = extract_field_type(&pair.get_item(1)?)?;
        fields.push((name, field_type));
    }

    CoreStreamSchema::try_new(fields).map_err(map_core_err)
}

fn extract_field_type(spec_obj: &Bound<'_, PyAny>) -> PyResult<FieldType> {
    let spec = spec_obj.downcast::<PyDict>().map_err(|_| {
        PyErr::new::<pyo3::exceptions::PyTypeError, _>("field type spec must be a dict")
    })?;

    let kind = spec
        .get_item("type")?
        .ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "field type spec missing required key 'type'",
            )
        })?
        .extract::<String>()?;
    let nullable = spec
        .get_item("nullable")?
        .map(|value| value.extract::<bool>())
        .transpose()?
        .unwrap_or(false);

    let scalar_type = match kind.as_str() {
        "string" => ScalarType::String,
        "int" => ScalarType::Int,
        "float" => ScalarType::Float,
        "bool" => ScalarType::Bool,
        "date" => ScalarType::Date,
        "time" => ScalarType::Time,
        "uuid" => ScalarType::Uuid,
        "binary" => ScalarType::Binary,
        "decimal" => {
            let precision = spec
                .get_item("precision")?
                .ok_or_else(|| {
                    PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                        "decimal field type requires 'precision'",
                    )
                })?
                .extract::<u8>()?;
            let scale = spec
                .get_item("scale")?
                .ok_or_else(|| {
                    PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                        "decimal field type requires 'scale'",
                    )
                })?
                .extract::<i8>()?;
            ScalarType::decimal(precision, scale)
        }
        "datetime" => {
            let has_tz = spec
                .get_item("has_tz")?
                .map(|value| value.extract::<bool>())
                .transpose()?
                .unwrap_or(true);
            if has_tz {
                ScalarType::datetime()
            } else {
                ScalarType::datetime_naive()
            }
        }
        other => {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "unknown stream scalar type '{}'",
                other
            )))
        }
    };

    Ok(if nullable {
        FieldType::nullable(scalar_type)
    } else {
        FieldType::new(scalar_type)
    })
}

fn py_err_to_io(err: PyErr) -> io::Error {
    io::Error::other(err.to_string())
}

struct PyTextSink {
    sink: Py<PyAny>,
}

impl PyTextSink {
    fn new(sink: Py<PyAny>) -> Self {
        Self { sink }
    }

    fn validate(sink: &Bound<'_, PyAny>) -> PyResult<()> {
        if !sink.hasattr("write")? {
            return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "stream sink must provide a write(str) method",
            ));
        }
        if !sink.hasattr("flush")? {
            return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "stream sink must provide a flush() method",
            ));
        }
        Ok(())
    }
}

impl Write for PyTextSink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = std::str::from_utf8(buf)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        Python::attach(|py| {
            self.sink
                .bind(py)
                .call_method1("write", (text,))
                .map_err(py_err_to_io)?;
            Ok(buf.len())
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Python::attach(|py| {
            self.sink
                .bind(py)
                .call_method0("flush")
                .map_err(py_err_to_io)?;
            Ok(())
        })
    }
}

struct PyTextReader {
    source: Py<PyAny>,
    buffer: Vec<u8>,
    position: usize,
    eof: bool,
}

impl PyTextReader {
    fn new(source: Py<PyAny>) -> Self {
        Self {
            source,
            buffer: Vec::new(),
            position: 0,
            eof: false,
        }
    }

    fn validate(source: &Bound<'_, PyAny>) -> PyResult<()> {
        if !source.hasattr("readline")? {
            return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "stream source must provide a readline() method",
            ));
        }
        Ok(())
    }

    fn refill(&mut self) -> io::Result<()> {
        if self.eof {
            return Ok(());
        }

        Python::attach(|py| {
            let line = self
                .source
                .bind(py)
                .call_method0("readline")
                .map_err(py_err_to_io)?
                .extract::<String>()
                .map_err(py_err_to_io)?;

            if line.is_empty() {
                self.eof = true;
                self.buffer.clear();
                self.position = 0;
            } else {
                self.buffer = line.into_bytes();
                self.position = 0;
            }
            Ok(())
        })
    }
}

impl Read for PyTextReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let available = self.fill_buf()?;
        if available.is_empty() {
            return Ok(0);
        }

        let len = available.len().min(buf.len());
        buf[..len].copy_from_slice(&available[..len]);
        self.consume(len);
        Ok(len)
    }
}

impl BufRead for PyTextReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.position >= self.buffer.len() && !self.eof {
            self.refill()?;
        }
        Ok(&self.buffer[self.position..])
    }

    fn consume(&mut self, amt: usize) {
        self.position = self.position.saturating_add(amt).min(self.buffer.len());
        if self.position >= self.buffer.len() {
            self.buffer.clear();
            self.position = 0;
        }
    }
}

#[pyclass(module = "ttoon._core", name = "StreamResult")]
#[derive(Clone)]
struct PyStreamResult {
    rows_emitted: usize,
}

impl From<CoreStreamResult> for PyStreamResult {
    fn from(value: CoreStreamResult) -> Self {
        Self {
            rows_emitted: value.rows_emitted,
        }
    }
}

#[pymethods]
impl PyStreamResult {
    #[getter]
    fn rows_emitted(&self) -> usize {
        self.rows_emitted
    }

    fn __repr__(&self) -> String {
        format!("StreamResult(rows_emitted={})", self.rows_emitted)
    }
}

#[pyclass(module = "ttoon._core", name = "StreamWriter")]
struct PyStreamWriter {
    inner: CoreStreamWriter<PyTextSink>,
}

#[pymethods]
impl PyStreamWriter {
    #[new]
    #[pyo3(signature = (sink, schema, delimiter=None, binary_format=None))]
    fn new(
        sink: &Bound<'_, PyAny>,
        schema: &Bound<'_, PyAny>,
        delimiter: Option<&str>,
        binary_format: Option<&str>,
    ) -> PyResult<Self> {
        PyTextSink::validate(sink)?;
        let schema = extract_stream_schema(schema)?;
        let opts = build_stream_ttoon_options(delimiter, binary_format)?;
        Ok(Self {
            inner: CoreStreamWriter::new(PyTextSink::new(sink.clone().unbind()), schema, opts),
        })
    }

    fn write(&mut self, row: &Bound<'_, PyAny>) -> PyResult<()> {
        let cache = TypeCache::new(row.py())?;
        let row = python_to_row(row, &cache)?;
        self.inner.write(&row).map_err(map_core_err)
    }

    fn close(&mut self) -> PyResult<PyStreamResult> {
        self.inner
            .close()
            .map(PyStreamResult::from)
            .map_err(map_core_err)
    }
}

#[pyclass(module = "ttoon._core", name = "StreamReader")]
struct PyStreamReader {
    inner: CoreStreamReader<PyTextReader>,
}

#[pymethods]
impl PyStreamReader {
    #[new]
    #[pyo3(signature = (source, schema, mode=None))]
    fn new(
        source: &Bound<'_, PyAny>,
        schema: &Bound<'_, PyAny>,
        mode: Option<&str>,
    ) -> PyResult<Self> {
        PyTextReader::validate(source)?;
        let schema = extract_stream_schema(schema)?;
        let mode = parse_mode_from_str(mode)?;
        Ok(Self {
            inner: CoreStreamReader::with_mode(
                PyTextReader::new(source.clone().unbind()),
                schema,
                mode,
            ),
        })
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        match self.inner.next() {
            None => Ok(None),
            Some(Ok(row)) => row_to_python(py, &row).map(Some),
            Some(Err(err)) => Err(map_core_err(err)),
        }
    }
}

#[pyclass(module = "ttoon._core", name = "ArrowStreamWriter")]
struct PyArrowStreamWriter {
    inner: CoreArrowStreamWriter<PyTextSink>,
}

#[pymethods]
impl PyArrowStreamWriter {
    #[new]
    #[pyo3(signature = (sink, schema, delimiter=None, binary_format=None))]
    fn new(
        sink: &Bound<'_, PyAny>,
        schema: &Bound<'_, PyAny>,
        delimiter: Option<&str>,
        binary_format: Option<&str>,
    ) -> PyResult<Self> {
        PyTextSink::validate(sink)?;
        let schema = extract_stream_schema(schema)?;
        let opts = build_stream_ttoon_options(delimiter, binary_format)?;
        let inner =
            CoreArrowStreamWriter::new(PyTextSink::new(sink.clone().unbind()), schema, opts)
                .map_err(map_core_err)?;
        Ok(Self { inner })
    }

    fn write_batch(&mut self, batch_obj: &Bound<'_, PyAny>) -> PyResult<()> {
        let arrow_table = pyarrow_to_arrow_table(batch_obj)?;
        for batch in &arrow_table.batches {
            self.inner.write_batch(batch).map_err(map_core_err)?;
        }
        Ok(())
    }

    fn close(&mut self) -> PyResult<PyStreamResult> {
        self.inner
            .close()
            .map(PyStreamResult::from)
            .map_err(map_core_err)
    }
}

#[pyclass(module = "ttoon._core", name = "ArrowStreamReader")]
struct PyArrowStreamReader {
    inner: CoreArrowStreamReader<PyTextReader>,
}

#[pymethods]
impl PyArrowStreamReader {
    #[new]
    #[pyo3(signature = (source, schema, batch_size=1024, mode=None))]
    fn new(
        source: &Bound<'_, PyAny>,
        schema: &Bound<'_, PyAny>,
        batch_size: usize,
        mode: Option<&str>,
    ) -> PyResult<Self> {
        PyTextReader::validate(source)?;
        let schema = extract_stream_schema(schema)?;
        let mode = parse_mode_from_str(mode)?;
        let inner = CoreArrowStreamReader::with_mode(
            PyTextReader::new(source.clone().unbind()),
            schema,
            batch_size,
            mode,
        )
        .map_err(map_core_err)?;
        Ok(Self { inner })
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        match self.inner.next() {
            None => Ok(None),
            Some(Ok(batch)) => PyRecordBatch::new(batch)
                .into_pyarrow(py)
                .map(|value| Some(value.unbind())),
            Some(Err(err)) => Err(map_core_err(err)),
        }
    }
}

#[pyclass(module = "ttoon._core", name = "TjsonStreamWriter")]
struct PyTjsonStreamWriter {
    inner: CoreTjsonStreamWriter<PyTextSink>,
}

#[pymethods]
impl PyTjsonStreamWriter {
    #[new]
    #[pyo3(signature = (sink, schema, binary_format=None))]
    fn new(
        sink: &Bound<'_, PyAny>,
        schema: &Bound<'_, PyAny>,
        binary_format: Option<&str>,
    ) -> PyResult<Self> {
        PyTextSink::validate(sink)?;
        let schema = extract_stream_schema(schema)?;
        let opts = build_tjson_options(binary_format)?;
        Ok(Self {
            inner: CoreTjsonStreamWriter::new(PyTextSink::new(sink.clone().unbind()), schema, opts),
        })
    }

    fn write(&mut self, row: &Bound<'_, PyAny>) -> PyResult<()> {
        let cache = TypeCache::new(row.py())?;
        let row = python_to_row(row, &cache)?;
        self.inner.write(&row).map_err(map_core_err)
    }

    fn close(&mut self) -> PyResult<PyStreamResult> {
        self.inner
            .close()
            .map(PyStreamResult::from)
            .map_err(map_core_err)
    }
}

#[pyclass(module = "ttoon._core", name = "TjsonStreamReader")]
struct PyTjsonStreamReader {
    inner: CoreTjsonStreamReader<PyTextReader>,
}

#[pymethods]
impl PyTjsonStreamReader {
    #[new]
    #[pyo3(signature = (source, schema, mode=None))]
    fn new(
        source: &Bound<'_, PyAny>,
        schema: &Bound<'_, PyAny>,
        mode: Option<&str>,
    ) -> PyResult<Self> {
        PyTextReader::validate(source)?;
        let schema = extract_stream_schema(schema)?;
        let mode = parse_mode_from_str(mode)?;
        Ok(Self {
            inner: CoreTjsonStreamReader::with_mode(
                PyTextReader::new(source.clone().unbind()),
                schema,
                mode,
            ),
        })
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        match self.inner.next() {
            None => Ok(None),
            Some(Ok(row)) => row_to_python(py, &row).map(Some),
            Some(Err(err)) => Err(map_core_err(err)),
        }
    }
}

#[pyclass(module = "ttoon._core", name = "TjsonArrowStreamWriter")]
struct PyTjsonArrowStreamWriter {
    inner: CoreTjsonArrowStreamWriter<PyTextSink>,
}

#[pymethods]
impl PyTjsonArrowStreamWriter {
    #[new]
    #[pyo3(signature = (sink, schema, binary_format=None))]
    fn new(
        sink: &Bound<'_, PyAny>,
        schema: &Bound<'_, PyAny>,
        binary_format: Option<&str>,
    ) -> PyResult<Self> {
        PyTextSink::validate(sink)?;
        let schema = extract_stream_schema(schema)?;
        let opts = build_tjson_options(binary_format)?;
        let inner =
            CoreTjsonArrowStreamWriter::new(PyTextSink::new(sink.clone().unbind()), schema, opts)
                .map_err(map_core_err)?;
        Ok(Self { inner })
    }

    fn write_batch(&mut self, batch_obj: &Bound<'_, PyAny>) -> PyResult<()> {
        let arrow_table = pyarrow_to_arrow_table(batch_obj)?;
        for batch in &arrow_table.batches {
            self.inner.write_batch(batch).map_err(map_core_err)?;
        }
        Ok(())
    }

    fn close(&mut self) -> PyResult<PyStreamResult> {
        self.inner
            .close()
            .map(PyStreamResult::from)
            .map_err(map_core_err)
    }
}

#[pyclass(module = "ttoon._core", name = "TjsonArrowStreamReader")]
struct PyTjsonArrowStreamReader {
    inner: CoreTjsonArrowStreamReader<PyTextReader>,
}

#[pymethods]
impl PyTjsonArrowStreamReader {
    #[new]
    #[pyo3(signature = (source, schema, batch_size=1024, mode=None))]
    fn new(
        source: &Bound<'_, PyAny>,
        schema: &Bound<'_, PyAny>,
        batch_size: usize,
        mode: Option<&str>,
    ) -> PyResult<Self> {
        PyTextReader::validate(source)?;
        let schema = extract_stream_schema(schema)?;
        let mode = parse_mode_from_str(mode)?;
        let inner = CoreTjsonArrowStreamReader::with_mode(
            PyTextReader::new(source.clone().unbind()),
            schema,
            batch_size,
            mode,
        )
        .map_err(map_core_err)?;
        Ok(Self { inner })
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        match self.inner.next() {
            None => Ok(None),
            Some(Ok(batch)) => PyRecordBatch::new(batch)
                .into_pyarrow(py)
                .map(|value| Some(value.unbind())),
            Some(Err(err)) => Err(map_core_err(err)),
        }
    }
}

/// Convert PyArrow RecordBatch/Table to ArrowTable（零複製）
fn pyarrow_to_arrow_table(table_obj: &Bound<'_, PyAny>) -> PyResult<ArrowTable> {
    // 嘗試從 PyArrow RecordBatch 直接提取
    if let Ok(py_batch) = table_obj.extract::<PyRecordBatch>() {
        let batch: RecordBatch = py_batch.into();
        let schema = batch.schema();
        return Ok(ArrowTable {
            schema,
            batches: vec![batch],
        });
    }

    // PyArrow Table（有 to_batches 方法）— 支援 multi-batch
    if table_obj.hasattr("to_batches")? {
        let batches_obj = table_obj.call_method0("to_batches")?;
        let batches_list = batches_obj.downcast::<PyList>()?;

        if batches_list.is_empty() {
            let schema_obj = table_obj.getattr("schema")?;
            let pyarrow = table_obj.py().import("pyarrow")?;
            let empty_arrays = PyList::empty(table_obj.py());
            let field_names = PyList::empty(table_obj.py());
            let fields = schema_obj.getattr("names")?;
            let types = schema_obj.getattr("types")?;
            for (name, dtype) in fields
                .downcast::<PyList>()?
                .iter()
                .zip(types.downcast::<PyList>()?.iter())
            {
                let empty_arr =
                    pyarrow.call_method1("array", (PyList::empty(table_obj.py()), dtype))?;
                empty_arrays.append(empty_arr)?;
                field_names.append(name)?;
            }
            let empty_batch_obj = pyarrow
                .getattr("RecordBatch")?
                .call_method1("from_arrays", (empty_arrays, field_names))?;
            let py_batch = empty_batch_obj.extract::<PyRecordBatch>()?;
            let batch: RecordBatch = py_batch.into();
            return Ok(ArrowTable {
                schema: batch.schema(),
                batches: vec![batch],
            });
        }

        let mut batches = Vec::with_capacity(batches_list.len());
        let mut schema = None;
        for item in batches_list.iter() {
            let py_batch = item.extract::<PyRecordBatch>()?;
            let batch: RecordBatch = py_batch.into();
            if schema.is_none() {
                schema = Some(batch.schema());
            }
            batches.push(batch);
        }
        return Ok(ArrowTable {
            schema: schema.unwrap(),
            batches,
        });
    }

    Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
        "expected PyArrow RecordBatch or Table",
    ))
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("TranscodeError", m.py().get_type::<TranscodeError>())?;
    m.add(
        "BUILD_PROFILE",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
    )?;
    m.add_class::<PyStreamResult>()?;
    m.add_class::<PyStreamWriter>()?;
    m.add_class::<PyStreamReader>()?;
    m.add_class::<PyArrowStreamWriter>()?;
    m.add_class::<PyArrowStreamReader>()?;
    m.add_class::<PyTjsonStreamWriter>()?;
    m.add_class::<PyTjsonStreamReader>()?;
    m.add_class::<PyTjsonArrowStreamWriter>()?;
    m.add_class::<PyTjsonArrowStreamReader>()?;
    m.add_function(wrap_pyfunction!(dumps, m)?)?;
    m.add_function(wrap_pyfunction!(loads, m)?)?;
    m.add_function(wrap_pyfunction!(read_arrow, m)?)?;
    m.add_function(wrap_pyfunction!(detect_format, m)?)?;
    m.add_function(wrap_pyfunction!(to_tjson, m)?)?;
    m.add_function(wrap_pyfunction!(stringify_arrow_tjson, m)?)?;
    m.add_function(wrap_pyfunction!(tjson_to_ttoon, m)?)?;
    m.add_function(wrap_pyfunction!(ttoon_to_tjson, m)?)?;
    Ok(())
}
