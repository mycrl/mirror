export interface MirrorOptions
{
    encoder: string;
    decoder: string;
    width: number;
    height: number;
    fps: number;
    bit_rate: number;
    multicast: string;
    server: string;
    mtu: number;
}

export declare class SenderService
{
    set_multicast(value: boolean);
    get_multicast(): boolean;
    close();
}

export declare class ReceiverService
{
    close();
}

export type DeviceType = "audio" | "video" | "screen" | "window";

export interface Device
{
    id: string;
    kind: DeviceType;
    index: number;
}

export declare class CaptureService
{
    start_capture(): boolean;
    get_devices(type: DeviceType): Device[] | null;
    set_input_device(device: Device): boolean;
    stop_capture();
}

export declare class MirrorService
{
    quit();
    init(options: MirrorOptions): boolean;
    create_capture_service(): CaptureService;
    create_sender(id: number, callback: () => void): SenderService | null;
    create_receiver(id: number, callback: () => void): ReceiverService | null;
}