{
    "targets": [
        {
            "target_name": "mirror",
            "cflags!": [ 
                "-fno-exceptions" 
            ],
            "cflags_cc!": [ 
                "-fno-exceptions" 
            ],
            "sources": [
                "./src/context.h",
                "./src/lib.cpp",
                "./src/lib.h",
                "./src/capture.cpp",
                "./src/capture.h",
                "./src/sender.cpp",
                "./src/sender.h",
                "./src/receiver.cpp",
                "./src/receiver.h",
                "./src/window.cpp",
                "./src/window.h"
            ],
            "include_dirs": [
                "<!@(node -p \"require('node-addon-api').include\")",
                "../../common/include",
                "../../sdk/desktop/include",
                "../../sdk/renderer/include"
            ],
            "defines": [ 
                "NAPI_DISABLE_CPP_EXCEPTIONS",
                "NODE_ADDON_API_ENABLE_MAYBE"
            ],
            "libraries": [
                "../../../build/lib/mirror.dll.lib",
                "../../../build/lib/renderer.dll.lib"
            ]
        }
    ]
}