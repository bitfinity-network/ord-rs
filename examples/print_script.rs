use argh::FromArgs;
use bitcoin::ScriptBuf;

#[derive(FromArgs, Debug)]
#[argh(description = "Transfer BRC20 tokens")]
struct Args {
    #[argh(positional)]
    pub hex: String,
}

fn main() -> anyhow::Result<()> {
    let args: Args = argh::from_env();

    let script_data = hex::decode(args.hex)?;
    let script = ScriptBuf::from_bytes(script_data);

    println!("script: {}", script);

    Ok(())
}
