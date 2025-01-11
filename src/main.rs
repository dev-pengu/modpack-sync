mod sync;

use std::env;

use sync::Config;

fn main() {
    let args: Vec<String> = env::args().collect();

    let config: Config = Config::build(&args).expect("expected a valid config");

    sync::run(config).expect("expected to install mods successfully");
}
