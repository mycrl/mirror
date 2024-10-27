import { useEffect, useState } from "react";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import {
    faDesktop,
    faMicrophone,
    faVideo,
    IconDefinition,
} from "@fortawesome/free-solid-svg-icons";
import styles from "@/styles/devices.module.css";

export const SourceType: { [key in MirrorSourceType]: number } = {
    /**
     * Camera or video capture card and other devices (and support virtual
     * camera)
     */
    Camera: 0,
    /**
     * The desktop or monitor corresponds to the desktop in the operating
     * system.
     */
    Screen: 1,
    /** Audio input and output devices. */
    Audio: 2,
};

const SourceTypeIcon: { [key in MirrorSourceType]: IconDefinition } = {
    Camera: faVideo,
    Screen: faDesktop,
    Audio: faMicrophone,
};

export interface DevicesProps {
    onChange?: (type: MirrorSourceType, source: MirrorSourceDescriptor) => void;
}

export default function Devices({ onChange }: DevicesProps) {
    const [kind, setKind] = useState(SourceType.Screen);
    const [devices, setDevices] = useState<MirrorSourceDescriptor[]>([]);

    const selectSourceType = async (type: MirrorSourceType) => {
        setDevices(await electronAPI.getSources(type));
        setKind(type);
    };

    useEffect(() => {
        return () => {
            selectSourceType(kind).then(() => {
                if (onChange && devices.length > 0) {
                    onChange(kind, devices[0]);
                }
            });
        };
    }, []);

    return (
        <>
            <div id={styles.devices}>
                <div id={styles.types}>
                    {Object.keys(SourceType).map((key) => {
                        return (
                            <div className={styles.type} key={key}>
                                <FontAwesomeIcon
                                    fixedWidth
                                    icon={SourceTypeIcon[key]}
                                    onClick={() => selectSourceType(SourceType[key])}
                                />
                                <p id={kind == SourceType[key] ? styles.selected : undefined}>·</p>
                            </div>
                        );
                    })}
                </div>
                <div id={styles.values}>
                    {Object.keys(SourceType).map((key) => {
                        return (
                            <select
                                key={key}
                                style={{
                                    display: kind == SourceType[key] ? undefined : "none",
                                }}
                                onChange={({ target }) => {
                                    onChange && onChange(kind, devices[Number(target.value)]);
                                }}
                            >
                                {devices.map((it, index) => (
                                    <option key={index} value={index}>
                                        {it.name}
                                    </option>
                                ))}
                            </select>
                        );
                    })}
                </div>
            </div>
        </>
    );
}
