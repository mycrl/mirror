import { useEffect, useState } from "react";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import {
    faDesktop,
    faMicrophone,
    faVideo,
    IconDefinition,
} from "@fortawesome/free-solid-svg-icons";
import styles from "@/styles/devices.module.css";
import { MirrorSourceDescriptor, MirrorSourceType } from "mirror-napi";

const SourceTypeIcon: { [key in MirrorSourceType]: IconDefinition } = {
    [MirrorSourceType.Camera]: faVideo,
    [MirrorSourceType.Screen]: faDesktop,
    [MirrorSourceType.Audio]: faMicrophone,
};

export interface DevicesProps {
    onChange?: (type: MirrorSourceType, source: MirrorSourceDescriptor) => void;
}

export default function Devices({ onChange }: DevicesProps) {
    const [kind, setKind] = useState(MirrorSourceType.Screen);
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
                    {Object.values(MirrorSourceType)
                        .filter((it) => typeof it != "string")
                        .map((type) => {
                            return (
                                <div className={styles.type} key={type}>
                                    <FontAwesomeIcon
                                        fixedWidth
                                        icon={SourceTypeIcon[type]}
                                        onClick={() => selectSourceType(type)}
                                    />
                                    <p id={kind == type ? styles.selected : undefined}>·</p>
                                </div>
                            );
                        })}
                </div>
                <div id={styles.values}>
                    {Object.keys(MirrorSourceType)
                        .filter((it) => typeof it != "string")
                        .map((type) => {
                            return (
                                <select
                                    key={type}
                                    style={{
                                        display: kind == type ? undefined : "none",
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
