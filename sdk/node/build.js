const fs = require("node:fs");

fs.writeFileSync(
    ".//index.d.ts",
    fs.readFileSync("./index.d.ts").toString().replaceAll("const enum", "enum")
);
