import Head from "next/head";
import Settings from "@/components/settings";
import Devices from "@/components/devices";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faXmark } from "@fortawesome/free-solid-svg-icons";
import styles from "@/styles/index.module.css";

export default function Index() {
    return (
        <>
            <Head>
                <meta charSet='UTF-8' />
                <meta content='text/html;charset=utf-8' httpEquiv='Content-Type' />
                <meta name='viewport' content='width=device-width, initial-scale=1' />
            </Head>
            <div id={styles.app}>
                <FontAwesomeIcon id={styles.close} icon={faXmark} />
                <div id={styles.box}>
                    <Devices />
                    <div id={styles.channel}>
                        <p>channel:</p>
                        <span>#0</span>
                    </div>
                    <Settings />
                </div>
            </div>
        </>
    );
}
