#### Examples

```sh
cargo run -- --width=1920 --height=1080 --fps=30 --encoder=libx264 --decoder=h264 --address=127.0.0.1:8080 --strategy=relay
```

#### Args

> example --help

| Field    | default          | help                          |
| -------- | ---------------- | ----------------------------- |
| fps      | 30               | frame rate                    |
| width    | 1280             | video width                   |
| height   | 720              | video height                  |
| encoder  | [hardware first] | libx264,h264_qsv,h264_nvenc   |
| decoder  | [hardware first] | h264,h264_qsv,h264_cuvid      |
| address  | 127.0.0.1:8080   | hylarana service bind address |
| strategy | direct           | direct,relay,multicast        |

The examples do not have graphical operations like buttons, they need to be operated by keys on the keyboard.
`"S"` creates the sender, `"R"` creates the receiver, and `"K"` stops both the sender and receiver.
