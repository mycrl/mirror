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

window.onload = () => new Vue({
    el: '#app',
    data: {
        working: false,
        ring: {
            style: {
                clipPath: null,
            }
        },
        devices: {
            kind: 'screen',
            index: 0,
            values: [],
        },
        settings: {
            status: false,
            value: {
                id: 0,
                encoder: 'libx264',
                decoder: 'h264',
                bitrate: 500 * 1024 * 8,
                multicast: '239.0.0.1',
                mtu: 1500,
                fps: 30,
                size: {
                    width: 1280,
                    height: 720,
                },
            }
        }
    },
    methods: {
        settings_switch()
        {
            if (!this.working)
            {
                this.settings.status = !this.settings.status
                if (!this.settings.status)
                {
                    this._update_settings()
                }
            }
        },
        ring_animation()
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
        async switch_sender()
        {
            this.ring_animation()
            this.working = !this.working
            if (this.working)
            {
                if (this.devices.values[this.devices.index])
                {
                    await electronAPI.create_sender(this.devices.values[this.devices.index])
                }
            }
            else
            {
                await electronAPI.close_sender()
                await this.kind_select(this.devices.kind)
            }
        },
        async kind_select(kind)
        {
            if (!this.working)
            {
                this.devices.kind = kind
                this.devices.values = await electronAPI.get_devices(kind)
                this.devices.index = 0
            }
        },
        async _update_settings()
        {
            await electronAPI.update_settings({
                id: Number(this.settings.value.id),
                encoder: this.settings.value.encoder,
                decoder: this.settings.value.decoder,
                bit_rate: Number(this.settings.value.bitrate),
                multicast: this.settings.value.multicast,
                mtu: Number(this.settings.value.mtu),
                fps: Number(this.settings.value.fps),
                width: Number(this.settings.value.size.width),
                height: Number(this.settings.value.size.height),
                server: '192.168.2.129:8088',
            })
        },
        close()
        {
            electronAPI.close()
        }
    },
    async mounted()
    {
        await this._update_settings()
        await this.kind_select('screen')
    }
})