use anyhow::Context;

fn main() -> anyhow::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", etw_consumer::usage());
        return Ok(());
    }

    let options = etw_consumer::parse_cli_args(args)?;
    etw_consumer::run(options).context("etw-consumer failed")
}
