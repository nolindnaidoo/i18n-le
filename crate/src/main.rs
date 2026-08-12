mod audit;
mod catalogue;
mod cli;
mod identify;
mod library;
mod locale;
mod mcp;
mod message;
mod scan;

#[cfg(test)]
mod corpus;
#[cfg(test)]
mod testing;

fn main() -> std::process::ExitCode {
    cli::run()
}
