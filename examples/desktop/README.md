#### Examples

```sh
./sender width=1920,height=1080,encoder=libx264,decoder=h264,bind=0.0.0.0:8080
```

#### Args

| Field   | default          | help                                    |
|---------|------------------|-----------------------------------------|
| width   | 1280             | video width                             |
| height  | 720              | video height                            |
| encoder | [hardware first] | libx264,h264_qsv,h264_nvenc             |
| decoder | [hardware first] | h264,h264_qsv,h264_cuvid                |
| bind    | 0.0.0.0:8080     | Listening network card address and port |
