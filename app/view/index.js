function ring(rate)
{
    rate = rate * 4

    if (rate <= 50)
    {
        rate += 50
        return `polygon(50% 50%, 50% 0%, ${rate}% 0%)`
    } else if ((rate > 50) && (rate <= 150))
    {
        rate -= 50
        return `polygon(50% 50%, 50% 0%, 100% 0%, 100% ${rate}%)`
    } else if ((rate > 150) && (rate <= 250))
    {
        rate = 250 - rate
        return `polygon(50% 50%, 50% 0%, 100% 0%, 100% 100%, ${rate}% 100%)`
    } else if ((rate > 250) && (rate <= 350))
    {
        rate = 350 - rate
        return `polygon(50% 50%, 50% 0%, 100% 0%, 100% 100%, 0% 100%, 0% ${rate}%)`
    } else if ((rate > 350) && (rate <= 400))
    {
        rate -= 350
        return `polygon(50% 50%, 50% 0%, 100% 0%, 100% 100%, 0% 100%, 0% 0%, ${rate}% 0%)`
    }
}

const SourceType = {
    Camera: 0,
    Screen: 1,
    Audio: 2,
}

const VideoDecoderType = {
    D3D11: 0,
    Qsv: 1,
    Cuda: 2,
}

const VideoEncoderType = {
    X264: 0,
    Qsv: 1,
    Cuda: 2,
}

window.onload = () => new Vue({
    el: '#app',
    data: {
        SourceType,
        VideoDecoderType,
        VideoEncoderType,
        working: false,
        ring: {
            style: {
                clipPath: null,
            }
        },
        sources: {
            kind: SourceType.Screen,
            index: 0,
            values: [],
        },
        settings: {
            status: false,
            value: {
                channel: 0,
                server: "127.0.0.1:8080",
                multicast: "139.0.0.1",
                mtu: 1400,
                decoder: VideoDecoderType.D3D11,
                encoder: VideoEncoderType.Qsv,
                frameRate: 24,
                width: 1280,
                height: 720,
                bitRate: 500 * 1024 * 8,
                keyFrameInterval: 20,
            }
        }
    },
    methods: {
        settingsSwitch()
        {
            if (!this.working)
            {
                this.settings.status = !this.settings.status
                if (!this.settings.status)
                {
                    this.updateSettings()
                }
            }
        },
        ringAnimation()
        {
            let rate = 0
            let time = setInterval(() =>
            {
                if (rate > 100)
                {
                    clearInterval(time)
                }

                this.ring.style.clipPath = ring(rate)
                rate += 1
            }, 10)
        },
        async switchSender()
        {
            this.ringAnimation()
            this.working = !this.working
            if (this.working)
            {
                if (this.sources.values[this.sources.index])
                {
                    await electronAPI.createSender(this.sources.values[this.sources.index])
                }
            }
            else
            {
                await electronAPI.closeSender()
                await this.kindSelect(this.sources.kind)
            }
        },
        async kindSelect(kind)
        {
            if (!this.working)
            {
                this.sources.kind = kind
                this.sources.values = await electronAPI.getSources(kind) || []
                this.sources.index = 0
            }
        },
        async updateSettings()
        {
            await electronAPI.setSettings(this.settings.value)
            await this.kindSelect(this.sources.kind)
        },
        close()
        {
            electronAPI.close()
        }
    },
    async mounted()
    {
        this.settings.value = await electronAPI.getSettings()
        await this.updateSettings()
    }
})
