# CSV to JSON

Convert UTF-8 CSV files to compact JSON. Use header rows for JSON objects or keep every row as an array. The program prints the number of output records and writes `rows.json` in the current directory. Each successful run replaces that file.

## Build and run

Install Rust 1.89 or later and the platform's C toolchain. Build with Cargo from the repository root.

    cargo build --release --locked
    cargo run --release --locked -- examples/taxi-sample.csv ',' LF header skip

Arguments are `INPUT DELIMITER LF|CRLF arrays|header skip|keep`. Supported delimiters are comma, semicolon, pipe and a literal tab. Use `CRLF` for CSV files with Windows line endings. Fields are kept as strings. Quoted fields may contain commas, escaped quotes and line breaks. Rows need a consistent field count. If a header name appears more than once, the last column supplies its value. The name `__proto__` is reserved.

Build locally or download the Windows x86-64 ZIP from [Releases](https://github.com/AndrewSnusoed5/csv-to-json-rust/releases). After extracting the ZIP, run `.\csv-to-json.exe taxi-sample.csv ',' LF header skip` in its folder. The executable runs without Node.js or Python. The program reuses a CSV record buffer and writes each record into the final JSON buffer. Peak memory depends on input and output size.

## Reproduce the checks

    cargo build --release --locked
    cargo test --locked
    npm ci --prefix reference
    python3 tests/compare.py --report results/benchmark.json

On Windows, use `py -3` if `python3` is unavailable. The comparison uses the pinned `csv-parse` 5.6.0 reference and retrieves the full Our World in Data energy file and the NYC Taxi and Limousine Commission's taxi zone lookup. The test runner checks the input SHA-256 values. Every comparison requires identical stdout and JSON bytes. Measurements include process startup and file I/O.

## Benchmark

Medians from seven measured pairs after two warm-ups per input. Windows 11, Intel Core i7-13700KF, Node.js 24.22.1, and Rust 1.89.0 release build.

| Dataset | Records | Node.js | Rust | Speedup | Peak RSS, Node.js to Rust |
| --- | ---: | ---: | ---: | ---: | ---: |
| Our World in Data energy, 9.23 MB | 23,378 | 399.78 ms | 46.32 ms | 8.63x | 245.77 to 32.59 MiB |
| NYC taxi zone lookup | 265 | 58.07 ms | 6.22 ms | 9.33x | 45.67 to 6.15 MiB |

All 81 pairs across nine inputs produced identical stdout and JSON. The full energy run wrote 15,354,406 matching JSON bytes. Small-file timings include the cost of starting each executable.

## Origin

OmniRust generated the Rust entry point from [the JavaScript reference](reference/csv-import.mjs). For this standalone release, the generated entry point was trimmed and connected to a small application library that uses the public crates `csv` and `serde_json`. The release covers the CSV-to-JSON workflow described above. The JavaScript package offers additional APIs. The 5.6.0 distribution contains about 2,000 lines of JavaScript in its library source. This Rust application has 354 physical source lines, excluding tests and dependencies.

## License

MIT. See [LICENSE](LICENSE).
