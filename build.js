'use strict'

const { exec } = require('node:child_process')
const path = require('node:path')
const fs = require('node:fs')

function spawn(command, options) {
    return new Promise((resolve, reject, ps = exec(command, options)) => {
        ps.stderr.pipe(process.stderr)
        ps.stdout.pipe(process.stdout)
        ps.on('close', resolve)
        ps.on('error', reject)
    })
}

async function build(Args) {
    await spawn(`cargo build ${Args.release ? '--release' : ''}`, { cwd: __dirname })

    const mirrorLibraryPath = path.join(__dirname, './target', Args.release ? 'release' : 'debug', './mirror.dll')
    const mirrorLibraryPdbPath = path.join(__dirname, './target', Args.release ? 'release' : 'debug', './mirror.pdb')
    const distributionsPath = path.join(__dirname, './target/distributions')
    const libraryPath = path.join(distributionsPath, './bin')
    const includePath = path.join(distributionsPath, './include')
    const examplePath = path.join(distributionsPath, './example')

    if (!fs.existsSync(distributionsPath)) {
        fs.mkdirSync(distributionsPath)
    }

    if (!fs.existsSync(includePath)) {
        fs.mkdirSync(includePath)
    }

    fs.copyFileSync(
        path.join(__dirname, './sdk/desktop/include/mirror.h'), 
        path.join(includePath, './mirror.h')
    )

    fs.copyFileSync(
        path.join(__dirname, './common/include/frame.h'), 
        path.join(includePath, './frame.h')
    )

    for (const file of ['sender', 'receiver', 'CMakeLists.txt']) {
        await spawn(`Copy-Item \
                    -Path ./examples/desktop/${file} \
                    -Destination ${examplePath} \
                    -Recurse \
                    -Force`, { cwd: __dirname, shell: 'powershell.exe' })
    }

    if (!fs.existsSync(libraryPath)) {
        await spawn('Invoke-WebRequest \
            -Uri https://github.com/mycrl/distributions/releases/download/distributions/distributions-windows.zip \
            -OutFile distributions-windows.zip', { cwd: distributionsPath, shell: 'powershell.exe' })

        await spawn('Expand-Archive \
            -Path ./distributions-windows.zip \
            -DestinationPath ./', { cwd: distributionsPath, shell: 'powershell.exe' })
        fs.unlinkSync(path.join(distributionsPath, './distributions-windows.zip'))
        fs.renameSync(path.join(distributionsPath, './distributions-windows'), libraryPath)
    }

    fs.copyFileSync(mirrorLibraryPath, path.join(libraryPath, './mirror.dll'))

    if (Args.example) {
        const exampleBuildPath = path.join(__dirname, './examples/desktop/build')
        const profile = Args.release ? 'Release' : 'Debug'

        if (!fs.existsSync(exampleBuildPath)) {
            fs.mkdirSync(exampleBuildPath)
        }

        await spawn(`cmake -DCMAKE_BUILD_TYPE=${profile} ..`, { cwd: exampleBuildPath })
        await spawn('cmake --build .', { cwd: exampleBuildPath })
        
        for (const project of ['sender', 'receiver']) {
            await spawn(`Copy-Item \
                -Path ${libraryPath}/* \
                -Destination ${exampleBuildPath}/${project}/${profile} \
                -Recurse \
                -Force`, { cwd: __dirname, shell: 'powershell.exe' })

            fs.copyFileSync(
                mirrorLibraryPath,
                path.join(exampleBuildPath, project, `./${profile}/mirror.dll`),
            )

            fs.copyFileSync(
                path.join(exampleBuildPath, project, `./${profile}/${project}.exe`),
                path.join(libraryPath, `${project}.exe`)
            )

            if (!Args.release) {
                fs.copyFileSync(
                    mirrorLibraryPdbPath,
                    path.join(exampleBuildPath, project, `./${profile}/mirror.pdb`),
                )
            }
        }
    }
}

build(process
    .argv
    .slice(process.argv.indexOf('--') + 1)
    .map(item => item.replace('--', ''))
    .reduce((args, item) => Object.assign(args, { [item]: true }), {}))
