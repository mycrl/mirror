#### Examples

```sh
./sender id=0,width=1920,height=1080,fps=30,encoder=libx264,decoder=h264,server=127.0.0.1:8080
```

#### Args

| Field   | default          | help                                    |
|---------|------------------|-----------------------------------------|
| id      | 0                | stream id                               |
| fps     | 30               | frame rate                              |
| width   | 1280             | video width                             |
| height  | 720              | video height                            |
| encoder | [hardware first] | libx264,h264_qsv,h264_nvenc             |
| decoder | [hardware first] | h264,h264_qsv,h264_cuvid                |
| server  | 127.0.0.1:8080   | mirror service bind address             |
