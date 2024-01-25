mod logger;

use anyhow::Result;
use asciinema::format;
use asciinema::format::asciicast;
use asciinema::pty;
use asciinema::recorder;
use clap::Parser;
use logger::AsyncLogger;
use std::collections::HashMap;
use std::env;

/// Record a terminal session and stream it to CloudWatch Logs
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
	/// CloudWatch Logs log group
	group: String,

	/// Session ID
	id: String,

	#[arg(short, long)]
	verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
	let args = Cli::parse();

	run(args.group, args.id).await
}

async fn run(group: String, id: String) -> Result<()> {
	let logger = AsyncLogger::create(
		group.to_owned(),
		id.to_owned()
	).await?;
	let writer: Box<dyn format::Writer + Send> = Box::new(asciicast::Writer::new(logger, 0.0));
	// let file = fs::OpenOptions::new()
	// 	.write(true)
	// 	.create(true)
	// 	.truncate(true)
	// 	.open("/tmp/asciinema.rec")?;
	// let writer: Box<dyn format::Writer + Send> = Box::new(asciicast::Writer::new(file, 0.0));

	let command = None;
	let env = HashMap::new();
	let mut recorder = recorder::Recorder::new(
		writer,
		false,
		true,
		None,
		command,
		Some(id.clone()),
		env,
	);

	let exec_args = build_exec_args( /*command*/ None );

	println!("recording asciicast to {}/{}", group, id);

	let extra_env = HashMap::new();
	println!("exec_args: {:?}", exec_args);
	println!("extra_env: {:?}", extra_env);
	pty::exec(
		&exec_args,
		&extra_env,
		(None, None),
		&mut recorder,
	)?;

	println!("done recording");

	Ok(())
}

fn build_exec_args(command: Option<String>) -> Vec<String> {
	let command = command
		.or(env::var("SHELL").ok())
		.unwrap_or("/bin/sh".to_owned());

	vec!["/bin/sh".to_owned(), "-c".to_owned(), command]
}
