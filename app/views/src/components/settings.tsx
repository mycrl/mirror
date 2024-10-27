import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faCaretDown } from "@fortawesome/free-solid-svg-icons";
import { useState } from "react";

export default function Settings() {
    const [isShow, setIsShow] = useState(false);

    return (
        <>
            <div id='settings'>
                <div id='box'>
                    <FontAwesomeIcon
                        id='switch'
                        icon={faCaretDown}
                        onClick={() => setIsShow(!isShow)}
                    />

                    <div id='items'></div>
                </div>
            </div>
        </>
    );
}
