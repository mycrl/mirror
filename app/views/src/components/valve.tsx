import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faChromecast } from "@fortawesome/free-brands-svg-icons";
import { faPowerOff } from "@fortawesome/free-solid-svg-icons";
import styles from "@/styles/valve.module.css";

export interface ValvePops {
    isWorking: boolean;
    onClick?: () => void;
}

export default function Valve({ isWorking, onClick }: ValvePops) {
    return (
        <>
            <div id={styles.valve}>
                <div id={styles.box}>
                    <div
                        id={styles.ring}
                        style={{
                            clipPath: undefined,
                        }}
                    />
                    <button onClick={onClick}>
                        <FontAwesomeIcon
                            icon={faChromecast}
                            style={{
                                display: isWorking ? "none" : undefined,
                            }}
                        />
                        <FontAwesomeIcon
                            icon={faPowerOff}
                            style={{
                                display: isWorking ? undefined : "none",
                            }}
                        />
                    </button>
                </div>
            </div>
        </>
    );
}
