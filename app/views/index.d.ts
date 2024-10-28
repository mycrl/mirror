import {
    MirrorSourceDescriptor,
    MirrorSourceType,
    MirrorVideoDecoderType,
    MirrorVideoEncoderType,
} from "mirror-napi";

declare global {
    export interface Settings {
        channel: number;
        server: string;
        multicast: string;
        mtu: number;
        decoder: MirrorVideoDecoderType;
        encoder: MirrorVideoEncoderType;
        frameRate: number;
        width: number;
        height: number;
        bitRate: number;
        keyFrameInterval: number;
    }

    export interface DevicesDescriptor {
        video?: MirrorSourceDescriptor;
        audio?: MirrorSourceDescriptor;
    }

    export interface electronAPI {
        getSources: (kind: MirrorSourceType) => Promise<MirrorSourceDescriptor[]>;
        setSettings: (settings: Settings) => Promise<void>;
        getSettings: () => Promise<Settings>;
        createSender: (device: DevicesDescriptor) => Promise<void>;
        closeSender: () => Promise<void>;
        close: () => Promise<void>;
    }

    export const electronAPI: electronAPI;

    export interface Window {
        electronAPI: electronAPI;
    }

    export {
        MirrorSourceDescriptor,
        MirrorSourceType,
        MirrorVideoDecoderType,
        MirrorVideoEncoderType,
    };
}
