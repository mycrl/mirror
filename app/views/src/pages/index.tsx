import Head from "next/head";
import Settings, { SettingsState } from "@/components/settings";
import Devices from "@/components/devices";
import Valve from "@/components/valve";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faXmark } from "@fortawesome/free-solid-svg-icons";
import styles from "@/styles/index.module.css";
import { useState } from "react";

export default function Index() {
    const [settingsState, setSettingsState] = useState(SettingsState.Min);

    return (
        <>
            <Head>
                <meta charSet='UTF-8' />
                <meta content='text/html;charset=utf-8' httpEquiv='Content-Type' />
                <meta name='viewport' content='width=device-width, initial-scale=1' />
            </Head>
            <div id={styles.app}>
                <div id={styles.arrow}>
                    <div id='arrow-item' />
                </div>
                <div id={styles.box}>
                    <FontAwesomeIcon id={styles.close} icon={faXmark} />
                    <Valve isWorking={false} />
                    <Devices />
                    <div id={styles.channel}>
                        <p>channel:</p>
                        <span>#0</span>
                    </div>
                    <Settings
                        state={settingsState}
                        onClick={() => {
                            setSettingsState(
                                settingsState == SettingsState.Min
                                    ? SettingsState.Max
                                    : SettingsState.Min
                            );
                        }}
                    />
                </div>
            </div>
        </>
    );
}
