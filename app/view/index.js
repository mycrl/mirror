window.onload = () =>
    new Vue({
        el: "#app",
        data: {
            SourceType: types.MirrorSourceType,
            VideoDecoderType: types.MirrorVideoDecoderType,
            VideoEncoderType: types.MirrorVideoEncoderType,
            working: false,
            ring: {
                style: {
                    clipPath: null,
                },
            },
            sources: {
                kind: types.MirrorSourceType.Screen,
                videoIndex: 0,
                audioIndex: 0,
                videos: [],
                audios: [],
            },
            settings: {
                status: false,
                value: {
                    channel: 0,
                    server: "127.0.0.1:8080",
                    multicast: "139.0.0.1",
                    mtu: 1400,
                    decoder: types.MirrorVideoDecoderType.D3D11,
                    encoder: types.MirrorVideoEncoderType.Qsv,
                    frameRate: 24,
                    width: 1280,
                    height: 720,
                    bitRate: 500 * 1024 * 8,
                    keyFrameInterval: 20,
                },
            },
        },
        methods: {
            settingsSwitch() {
                if (!this.working) {
                    this.settings.status = !this.settings.status;
                    if (!this.settings.status) {
                        this.updateSettings();
                    }
                }
            },
            ringAnimation() {
                let rate = 0;
                let time = setInterval(() => {
                    if (rate > 100) {
                        clearInterval(time);
                    }

                    this.ring.style.clipPath = this.ring(rate);
                    rate += 1;
                }, 10);
            },
            async switchSender() {
                this.ringAnimation();
                this.working = !this.working;
                if (this.working) {
                    await electronAPI.createSender({
                        video: this.sources.videos[this.sources.videoIndex],
                        audio: this.sources.audios[this.sources.audioIndex],
                    });
                } else {
                    await electronAPI.closeSender();
                    await this.kindSelect(this.sources.kind);
                }
            },
            async kindSelect(kind) {
                if (!this.working) {
                    this.sources.kind = kind;

                    if (kind == types.MirrorSourceType.Audio) {
                        this.sources.audios = (await electronAPI.getSources(kind)) || [];
                        this.sources.audioIndex = 0;
                    } else {
                        this.sources.videos = (await electronAPI.getSources(kind)) || [];
                        this.sources.videoIndex = 0;
                    }
                }
            },
            async updateSettings() {
                this.settings.value.frameRate = Number(this.settings.value.frameRate);
                this.settings.value.channel = Number(this.settings.value.channel);
                this.settings.value.width = Number(this.settings.value.width);
                this.settings.value.height = Number(this.settings.value.height);
                this.settings.value.mtu = Number(this.settings.value.mtu);

                await electronAPI.setSettings(this.settings.value);
                await this.kindSelect(this.sources.kind);
            },
            close() {
                electronAPI.close();
            },
            ring(rate) {
                rate = rate * 4;

                if (rate <= 50) {
                    rate += 50;
                    return `polygon(50% 50%, 50% 0%, ${rate}% 0%)`;
                } else if (rate > 50 && rate <= 150) {
                    rate -= 50;
                    return `polygon(50% 50%, 50% 0%, 100% 0%, 100% ${rate}%)`;
                } else if (rate > 150 && rate <= 250) {
                    rate = 250 - rate;
                    return `polygon(50% 50%, 50% 0%, 100% 0%, 100% 100%, ${rate}% 100%)`;
                } else if (rate > 250 && rate <= 350) {
                    rate = 350 - rate;
                    return `polygon(50% 50%, 50% 0%, 100% 0%, 100% 100%, 0% 100%, 0% ${rate}%)`;
                } else if (rate > 350 && rate <= 400) {
                    rate -= 350;
                    return `polygon(50% 50%, 50% 0%, 100% 0%, 100% 100%, 0% 100%, 0% 0%, ${rate}% 0%)`;
                }
            },
        },
        async mounted() {
            this.settings.value = await electronAPI.getSettings();
            await this.updateSettings();
        },
    });
