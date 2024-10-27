import { useState } from "react";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import {
    faDesktop,
    faMicrophone,
    faVideo,
    IconDefinition,
} from "@fortawesome/free-solid-svg-icons";
import styles from "@/styles/devices.module.css";

export const MirrorSourceType = {
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

const MirrorSourceTypeIcon: { [key in keyof typeof MirrorSourceType]: IconDefinition } = {
    Camera: faVideo,
    Screen: faDesktop,
    Audio: faMicrophone,
};

export default function Devices() {
    const [kind, setKind] = useState(MirrorSourceType.Screen);

    return (
        <>
            <div id={styles.devices}>
                <div id={styles.types}>
                    {Object.keys(MirrorSourceType).map((item) => {
                        const key = item as keyof typeof MirrorSourceType;
                        const value = MirrorSourceType[key];
                        return (
                            <div className={styles.type}>
                                <FontAwesomeIcon
                                    fixedWidth
                                    style={{ fontSize: "16px" }}
                                    icon={MirrorSourceTypeIcon[key]}
                                    onClick={() => setKind(value)}
                                />
                                <p id={kind == value ? styles.selected : undefined}>·</p>
                            </div>
                        );
                    })}
                </div>
                <div id={styles.values}>
                    {Object.keys(MirrorSourceType).map((item) => {
                        const key = item as keyof typeof MirrorSourceType;
                        const value = MirrorSourceType[key];
                        return (
                            <select
                                style={{
                                    display: kind == value ? undefined : "none",
                                }}
                            ></select>
                        );
                    })}
                </div>
            </div>
        </>
    );
}
