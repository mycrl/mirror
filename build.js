'use strict'

const { execSync } = require('node:child_process')
const path = require('node:path')
const fs = require('node:fs')

const Args = process
    .argv
    .slice(process.argv.indexOf('--') + 1)
    .map(item => item.replace('--', ''))
    .reduce((args, item) => Object.assign(args, { [item]: true }), {})

execSync(`cargo build ${Args.release ? '--release' : ''}`, { cwd: __dirname })

const distributionsPath = path.join(__dirname, './target')
if (!fs.existsSync(distributionsPath)) {
    fs.mkdirSync(distributionsPath)
}

if (!fs.existsSync(path.join(distributionsPath, './distributions'))) {
    execSync('Invoke-WebRequest \
        -Uri https://github.com/mycrl/mirror/releases/download/distributions/distributions-windows.zip \
        -OutFile distributions-windows.zip', { cwd: distributionsPath, shell: 'powershell.exe' })
        
    execSync('Expand-Archive \
        -Path ./distributions-windows.zip \
        -DestinationPath ./', { cwd: distributionsPath, shell: 'powershell.exe' })
    fs.unlinkSync(path.join(distributionsPath, './distributions-windows.zip'))
    fs.renameSync(path.join(distributionsPath, './distributions-windows'), path.join(distributionsPath, './distributions'))
}

fs.copyFileSync(
    path.join(__dirname, `./target/${Args.release ? 'release' : 'debug'}/mirror.dll`),
    path.join(distributionsPath, './distributions/mirror.dll'),
)

if (Args.example) {
    const exampleBuildPath = path.join(__dirname, './examples/desktop/build')
    if (!fs.existsSync(exampleBuildPath)) {
        fs.mkdirSync(exampleBuildPath)
    }

    execSync('cmake ..', { cwd: exampleBuildPath })
    execSync('cmake --build .', { cwd: exampleBuildPath })
    execSync('Copy-Item \
        -Path ./target/distributions/* \
        -Destination ./examples/desktop/build/Debug \
        -Recurse \
        -Force', { cwd: __dirname, shell: 'powershell.exe' })

    fs.copyFileSync(
        path.join(__dirname, `./target/${Args.release ? 'release' : 'debug'}/mirror.dll`),
        path.join(exampleBuildPath, './Debug/mirror.dll'),
    )

    if (!Args.release) {
        fs.copyFileSync(
            path.join(__dirname, './target/debug/mirror.pdb'),
            path.join(exampleBuildPath, './Debug/mirror.pdb'),
        )
    }
}
