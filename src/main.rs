#![forbid(unsafe_code)]

// Generated entry point with product-specific imports and unused declarations removed.
use csv_to_json::{self as rt, BinOp, ProductResult, Scope, Value as V};

#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn run() -> ProductResult<()> {
    rt::validate_args()?;
    let mut env = Scope::default();
    let value_0 = rt::dispatch(
        "fs.read",
        vec![
            rt::index(rt::dispatch("argv", vec![])?, V::Index(2))?,
            V::Str("utf8".into()),
        ],
    )?;
    env.bind("input", value_0);
    let value_1 = rt::dispatch(
        "csv-parse.sync",
        vec![
            env.get("input")?,
            V::map(vec![
                (
                    V::Str("delimiter".into()),
                    rt::index(rt::dispatch("argv", vec![])?, V::Index(3))?,
                ),
                (
                    V::Str("record_delimiter".into()),
                    if (rt::binary(
                        BinOp::StrictEq,
                        rt::index(rt::dispatch("argv", vec![])?, V::Index(4))?,
                        V::Str("CRLF".into()),
                    )?)
                    .is_true()
                    {
                        V::Str("\r\n".into())
                    } else {
                        V::Str("\n".into())
                    },
                ),
                (
                    V::Str("columns".into()),
                    rt::binary(
                        BinOp::StrictEq,
                        rt::index(rt::dispatch("argv", vec![])?, V::Index(5))?,
                        V::Str("header".into()),
                    )?,
                ),
                (
                    V::Str("skip_empty_lines".into()),
                    rt::binary(
                        BinOp::StrictEq,
                        rt::index(rt::dispatch("argv", vec![])?, V::Index(6))?,
                        V::Str("skip".into()),
                    )?,
                ),
                (V::Str("bom".into()), V::Bool(true)),
            ])?,
        ],
    )?;
    env.bind("records", value_1);
    let _ = rt::dispatch(
        "fs.write",
        vec![
            V::Str("rows.json".into()),
            rt::dispatch("json.dumps", vec![env.get("records")?])?,
            V::Str("utf8".into()),
        ],
    )?;
    let _ = rt::dispatch(
        "print",
        vec![rt::index(env.get("records")?, V::Str("length".into()))?],
    )?;
    Ok(())
}
fn main() -> std::process::ExitCode {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        println!("{}", rt::USAGE);
        return std::process::ExitCode::SUCCESS;
    }
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
