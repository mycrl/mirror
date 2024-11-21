#### Examples

```sh
./example-cpp --width=1920 --height=1080 --fps=30 --encoder=libx264 --decoder=h264 --address=127.0.0.1:8080 --strategy=relay
```

#### Args

> example-cpp --help

| Field    | default          | help                          |
| -------- | ---------------- | ----------------------------- |
| fps      | 30               | frame rate                    |
| width    | 1280             | video width                   |
| height   | 720              | video height                  |
| encoder  | [hardware first] | libx264,h264_qsv,h264_nvenc   |
| decoder  | [hardware first] | h264,h264_qsv,h264_cuvid      |
| address  | 127.0.0.1:8080   | hylarana service bind address |
| strategy | direct           | direct,relay,multicast        |
