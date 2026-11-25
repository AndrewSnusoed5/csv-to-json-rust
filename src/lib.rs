#![forbid(unsafe_code)]

use csv::{ReaderBuilder, StringRecord, Terminator};
use serde::ser::{Serialize, SerializeMap, Serializer};
use std::{borrow::Cow, collections::HashMap, env, fs, rc::Rc};

pub type ProductResult<T> = Result<T, Box<dyn std::error::Error>>;

pub const USAGE: &str = "Usage: csv-to-json INPUT DELIMITER LF|CRLF arrays|header skip|keep";

pub fn validate_args() -> ProductResult<()> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    match args.as_slice() {
        [_, delimiter, newline, shape, blanks]
            if [",", "\t", ";", "|"]
                .iter()
                .any(|value| delimiter == *value)
                && (newline == "LF" || newline == "CRLF")
                && (shape == "arrays" || shape == "header")
                && (blanks == "skip" || blanks == "keep") =>
        {
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}

#[derive(Clone)]
pub enum Value {
    Str(String),
    Index(usize),
    Bool(bool),
    Options(HashMap<String, Value>),
    Argv,
    Input(Rc<String>),
    Document(Rc<Document>),
    Count(usize),
    Unit,
}

impl Value {
    pub fn map(entries: Vec<(Self, Self)>) -> ProductResult<Self> {
        let mut options = HashMap::new();
        for (key, value) in entries {
            let Self::Str(key) = key else {
                return Err("Invalid CSV option name".into());
            };
            options.insert(key, value);
        }
        Ok(Self::Options(options))
    }

    pub fn is_true(&self) -> bool {
        matches!(self, Self::Bool(true))
    }
}

#[derive(Default)]
pub struct Scope(HashMap<&'static str, Value>);

impl Scope {
    pub fn bind(&mut self, name: &'static str, value: Value) {
        self.0.insert(name, value);
    }

    pub fn get(&self, name: &str) -> ProductResult<Value> {
        self.0
            .get(name)
            .cloned()
            .ok_or_else(|| format!("Missing application value {name}").into())
    }
}

pub enum BinOp {
    StrictEq,
}

pub fn binary(_op: BinOp, left: Value, right: Value) -> ProductResult<Value> {
    match (left, right) {
        (Value::Str(left), Value::Str(right)) => Ok(Value::Bool(left == right)),
        _ => Err("Invalid option comparison".into()),
    }
}

pub fn index(value: Value, index: Value) -> ProductResult<Value> {
    match (value, index) {
        (Value::Argv, Value::Index(index)) if index >= 2 => {
            let value = env::args_os().nth(index - 1).ok_or(USAGE)?;
            Ok(Value::Str(
                value.into_string().map_err(|_| "Arguments must be UTF-8")?,
            ))
        }
        (Value::Document(document), Value::Str(key)) if key == "length" => {
            Ok(Value::Count(document.rows))
        }
        _ => Err("Invalid application index".into()),
    }
}

pub fn dispatch(operation: &str, values: Vec<Value>) -> ProductResult<Value> {
    match (operation, values.as_slice()) {
        ("argv", []) => Ok(Value::Argv),
        ("fs.read", [Value::Str(path), Value::Str(encoding)]) if encoding == "utf8" => {
            Ok(Value::Input(Rc::new(fs::read_to_string(path)?)))
        }
        ("csv-parse.sync", [Value::Input(text), Value::Options(options)]) => {
            let config = Config::from_options(options)?;
            Ok(Value::Document(Rc::new(convert(text, config)?)))
        }
        ("json.dumps", [Value::Document(document)]) => Ok(Value::Document(Rc::clone(document))),
        ("fs.write", [Value::Str(path), Value::Document(document), Value::Str(encoding)])
            if encoding == "utf8" =>
        {
            fs::write(path, &document.json)?;
            Ok(Value::Unit)
        }
        ("print", [Value::Count(count)]) => {
            println!("{count}");
            Ok(Value::Unit)
        }
        _ => Err(format!("Unsupported application operation {operation}").into()),
    }
}

#[derive(Clone, Copy)]
struct Config {
    delimiter: u8,
    crlf: bool,
    header: bool,
    skip_blank: bool,
}

impl Config {
    fn from_options(options: &HashMap<String, Value>) -> ProductResult<Self> {
        let Some(Value::Str(delimiter)) = options.get("delimiter") else {
            return Err("Missing delimiter".into());
        };
        let Some(Value::Str(newline)) = options.get("record_delimiter") else {
            return Err("Missing record delimiter".into());
        };
        let &[delimiter] = delimiter.as_bytes() else {
            return Err("Expected a single-byte delimiter".into());
        };
        Ok(Self {
            delimiter,
            crlf: newline == "\r\n",
            header: matches!(options.get("columns"), Some(Value::Bool(true))),
            skip_blank: matches!(options.get("skip_empty_lines"), Some(Value::Bool(true))),
        })
    }
}

pub struct Document {
    json: Vec<u8>,
    rows: usize,
}

fn convert(input: &str, config: Config) -> ProductResult<Document> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let text = if config.skip_blank {
        Cow::Borrowed(input)
    } else {
        mark_blank_records(input, config.crlf)
    };
    let terminator = if config.crlf {
        Terminator::CRLF
    } else {
        Terminator::Any(b'\n')
    };
    let mut reader = ReaderBuilder::new()
        .has_headers(config.header)
        .delimiter(config.delimiter)
        .terminator(terminator)
        .from_reader(text.as_bytes());
    let header = if config.header {
        reader.headers()?.clone()
    } else {
        StringRecord::new()
    };
    let fields = ordered_fields(&header)?;
    let mut row = StringRecord::new();
    let mut json = Vec::with_capacity(text.len().saturating_add(2));
    json.push(b'[');
    let mut rows = 0;
    while reader.read_record(&mut row)? {
        if rows != 0 {
            json.push(b',');
        }
        if config.header {
            let mut serializer = serde_json::Serializer::new(&mut json);
            let mut object = serializer.serialize_map(Some(fields.len()))?;
            for &(index, name) in &fields {
                object.serialize_entry(name, &row[index])?;
            }
            object.end()?;
        } else {
            serde_json::to_writer(&mut json, &ArrayRow(&row))?;
        }
        rows += 1;
    }
    json.push(b']');
    Ok(Document { json, rows })
}

struct ArrayRow<'a>(&'a StringRecord);

impl Serialize for ArrayRow<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.iter())
    }
}

fn ordered_fields(header: &StringRecord) -> ProductResult<Vec<(usize, &str)>> {
    let mut slots = HashMap::with_capacity(header.len());
    let mut fields = Vec::with_capacity(header.len());
    for (index, name) in header.iter().enumerate() {
        if name == "__proto__" {
            return Err(format!("Unsupported reserved header {name:?}").into());
        }
        let slot = *slots.entry(name).or_insert(fields.len());
        if slot == fields.len() {
            fields.push((index, name));
        } else {
            fields[slot] = (index, name);
        }
    }
    fields.sort_by_key(|&(_, name)| {
        name.parse::<u32>()
            .ok()
            .filter(|index| {
                name == "0" || (!name.starts_with('0') && !name.starts_with('+') && *index != 0)
            })
            .unwrap_or(u32::MAX)
    });
    Ok(fields)
}

// The csv crate skips physical blank records. Express each kept blank record
// as a single quoted empty field while leaving quoted multiline input intact.
fn mark_blank_records(input: &str, crlf: bool) -> Cow<'_, str> {
    let mut output: Option<String> = None;
    let mut offset = 0;
    let mut in_quotes = false;
    for line in input.split_inclusive('\n') {
        if !in_quotes && (line == "\n" || (crlf && line == "\r\n")) {
            output
                .get_or_insert_with(|| {
                    let mut buffer = String::with_capacity(input.len());
                    buffer.push_str(&input[..offset]);
                    buffer
                })
                .push_str("\"\"");
        }
        if let Some(output) = &mut output {
            output.push_str(line);
        }
        in_quotes ^= memchr::memchr_iter(b'"', line.as_bytes()).count() % 2 == 1;
        offset += line.len();
    }
    output.map_or(Cow::Borrowed(input), Cow::Owned)
}
