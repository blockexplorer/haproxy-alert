# haproxy-alert

A proxy that receives SMTP alerts from HAproxy and sends a slack message (for now).

## Tested on

* Linux (4.16.3)
* Rust (tested on rustc 1.27.0-nightly (79252ff4e 2018-04-29))

## Installation

```bash
$ cargo install --git=https://github.com/blockexplorer/haproxy-alert
```

## Usage

```bash
$ SLACK_HOOK="https://hooks.slack.com/..." haproxy-alert
```

## License

Distributed under the MIT license. See `LICENSE` for more information.

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

Please make sure to update tests as appropriate.
