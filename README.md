# cinemastream

Record a shell session with asciinema, directly to CloudWatch Logs!


## Building

You'll need a Rust toolchain.

```sh
cargo build
```

Note: the version of asciinema included with this project is [forked from the original](https://github.com/rmccue/asciinema/tree/rust-lib), and is based on an in-progress build of asciinema v3. Functionality may be unstable (e.g. macOS is not currently supported).


## Usage

cinemastream uses the AWS SDK, and expects SDK configuration to be set at the system level (either via environment variables or instance profiles).

For example, for hardcoded access keys, set environment variables:

```sh
AWS_ACCESS_KEY_ID=AKABC123DEF456ABC123
AWS_SECRET_ACCESS_KEY=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
AWS_REGION=us-east-1
```

cinemastream should be invoked as `cinemastream <group> <id>`, where `<group>` is the CloudWatch Logs group, and `<id>` is the stream ID within that group. cinemastream will attempt to create this log stream when it starts, and will allow it to already exist.


## Limitations

* cinemastream can buffer up to 2048 log entries. If the command output exceeds this buffer, cinemastream will fail to log further entries.
* Custom commands or environment variables may not be passed. cinemastream will invoke $SHELL if it is set, or /bin/sh if not.
* The title of the stream is always set to the `<id>` parameter.


## License

Copyright 2024 Ryan McCue.

Licensed under the GNU GPL v3 or later.

Based on the asciinema project, copyright 2011-2024 Marcin Kulik. Licensed under the GNU GPL v3 or later.

For other dependencies, see the Cargo dependencies.
