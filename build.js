const { exec } = require('node:child_process')
const { join } = require('node:path')
const download = require('download')
const unzipper = require('unzipper')
const fs = require('node:fs')

const Args = process
    .argv
    .slice(process.argv.indexOf('--') + 1)
    .map(item => item.replace('--', ''))
    .reduce((args, item) => Object.assign(args, {
        [item]: true
    }), {})

const Command = (cmd, options = {}) => new Promise((
    resolve,
    reject,
    ps = exec(process.platform == 'win32' ? '$ProgressPreference = \'SilentlyContinue\';' + cmd : cmd, {
        shell: process.platform == 'win32' ? 'powershell.exe' : 'bash',
        cwd: __dirname,
        ...options,
    }
    )) => {
    ps.stdout.pipe(process.stdout)
    ps.stderr.pipe(process.stderr)

    ps.on('error', reject)
    ps.on('close', code => {
        code == 0 ? resolve() : reject(code || 0)
    })
})

const Replace = (file, filters) => {
    let src = fs.readFileSync(file).toString()
    for (const item of filters) {
        src = src.replace(...item)
    }

    fs.writeFileSync(file, src)
}

/* async block */ void (async () => {
    const Profile = Args.release ? 'Release' : 'Debug'
    const BaseDistributions = 'https://github.com/mycrl/third-party/releases/download/distributions'

    for (const path of [
        './target',
        './build',
        './build/bin',
        './build/lib',
        './build/server',
        './build/include',
        './build/examples',
        './build/examples/cpp',
        './build/examples/rust',
    ]) {
        if (!fs.existsSync(path)) {
            fs.mkdirSync(path)
        }
    }

    await Command(`cargo build ${Args.release ? '--release' : ''} -p mirror-shared`)
    await Command(`cargo build ${Args.release ? '--release' : ''} -p mirror-example`)
    await Command(`cargo build ${Args.release ? '--release' : ''} -p mirror-service`)

    /* download ffmpeg librarys for windows */
    if (process.platform == 'win32') {
        if (!fs.existsSync('./target/ffmpeg')) {
            if (!fs.existsSync('./target/ffmpeg-windows-x64.zip')) {
                console.log('Start download ffmpeg...')
                await download(`${BaseDistributions}/ffmpeg-windows-x64.zip`, './target')
            }

            await (await unzipper.Open.file('./target/ffmpeg-windows-x64.zip')).extract({ path: './target' })
            fs.rmdirSync(`./target/ffmpeg-windows-x64.zip`)
        }
    }

    if (!fs.existsSync('./examples/cpp/build')) {
        fs.mkdirSync('./examples/cpp/build')
    }

    await Command(`cmake -DCMAKE_BUILD_TYPE=${Profile} ..`, { cwd: join(__dirname, './examples/cpp/build') })
    await Command(`cmake --build . --config=${Profile}`, { cwd: join(__dirname, './examples/cpp/build') })

    for (const item of [
        ['./README.md', './build/README.md'],
        ['./LICENSE.txt', './build/LICENSE.txt'],

        /* examples */
        ['./examples/cpp/src', './build/examples/cpp/src'],
        ['./examples/cpp/CMakeLists.txt', './build/examples/cpp/CMakeLists.txt'],
        ['./examples/rust', './build/examples/rust'],

        /* inculde */
        ['./ffi/include/mirror.h', './build/include/mirror.h'],
    ]) {
        fs.cpSync(...item, { force: true, recursive: true })
    }

    if (process.platform == 'win32') {
        for (const item of [
            [`./examples/cpp/build/${Profile}/example.exe`, './build/bin/example-cpp.exe'],
            [`./target/${Profile.toLowerCase()}/mirror-example.exe`, './build/bin/example.exe'],
            [`./target/${Profile.toLowerCase()}/mirror-service.exe`, './build/server/mirror-service.exe'],
            [`./target/${Profile.toLowerCase()}/mirror.dll.lib`, './build/lib/mirror.dll.lib'],
            [`./target/${Profile.toLowerCase()}/mirror.dll`, './build/bin/mirror.dll'],
            [`./target/ffmpeg/bin/avcodec-61.dll`, './build/bin/avcodec-61.dll'],
            [`./target/ffmpeg/bin/avutil-59.dll`, './build/bin/avutil-59.dll'],
            [`./target/ffmpeg/bin/swresample-5.dll`, './build/bin/swresample-5.dll'],
        ]) {
            fs.cpSync(...item, { force: true, recursive: true })
        }
    }
    else {
        for (const item of [
            [`./examples/cpp/build/example`, './build/bin/example-cpp'],
            [`./target/${Profile.toLowerCase()}/mirror-example`, './build/bin/example'],
            [`./target/${Profile.toLowerCase()}/mirror-service`, './build/server/mirror-service'],
            process.platform == 'darwin' ?
                [`./target/${Profile.toLowerCase()}/libmirror.dylib`, './build/bin/libmirror.dylib'] :
                [`./target/${Profile.toLowerCase()}/libmirror.so`, './build/bin/libmirror.so'],
        ]) {
            fs.cpSync(...item, { force: true, recursive: true })
        }
    }

    if (process.platform == 'win32') {
        for (const item of [
            ['./target/debug/mirror.pdb', './build/bin/mirror.pdb'],
            ['./target/debug/mirror_service.pdb', './build/server/mirror-service.pdb'],
        ]) {
            if (!Args.release) {
                fs.cpSync(...item, { force: true, recursive: true })
            }
            else {
                fs.rmSync(item[1], { force: true, recursive: true })
            }
        }
    }

    Replace('./build/examples/cpp/CMakeLists.txt', [
        ['../../sdk/renderer/include', '../include'],
        ['../../sdk/cpp/include', '../include'],
        ['../../frame/include', '../include'],
        ['../../target/debug', '../lib'],
        ['../../target/release', '../lib'],
    ])

    /* async block end */
})().catch(e => 
{
    console.error(e)
    process.exit(-1)
})
