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
    if (process.platform == 'win32' || process.platform == 'linux') {
        const name = `ffmpeg-n7.1-latest-${process.platform == 'win32' ? 'win64' : 'linux64'}-gpl-shared-7.1`

        if (!fs.existsSync('./target/ffmpeg')) {
            if (!fs.existsSync('./target/ffmpeg')) {
                console.log('Start download ffmpeg...')
                await download(`https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/${name}.zip`, './target')
            }

            await (await unzipper.Open.file(`./target/${name}.zip`)).extract({ path: './target' })
            fs.renameSync(`./target/${name}`, './target/ffmpeg')
            fs.rmSync(`./target/${name}`)
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
    else if (process.platform == 'darwin') {
        for (const item of [
            [`./examples/cpp/build/example`, './build/bin/example-cpp'],
            [`./target/${Profile.toLowerCase()}/mirror-example`, './build/bin/example'],
            [`./target/${Profile.toLowerCase()}/mirror-service`, './build/server/mirror-service'],
            [`./target/${Profile.toLowerCase()}/libmirror.dylib`, './build/bin/libmirror.dylib'],
        ]) {
            fs.cpSync(...item, { force: true, recursive: true })
        }
    }
    else if (process.platform == 'linux') {
        for (const item of [
            [`./examples/cpp/build/example`, './build/bin/example-cpp'],
            [`./target/${Profile.toLowerCase()}/mirror-example`, './build/bin/example'],
            [`./target/${Profile.toLowerCase()}/mirror-service`, './build/server/mirror-service'],
            [`./target/${Profile.toLowerCase()}/libmirror.so`, './build/bin/libmirror.so'],
            [`./target/ffmpeg/lib/libavcodec.so.61.19.100.so`, './build/lib/libavcodec.so.61.19.100.so'],
            [`./target/ffmpeg/lib/libavdevice.so.61.3.100.so`, './build/lib/libavdevice.so.61.3.100.so'],
            [`./target/ffmpeg/lib/libavfilter.so.10.4.100.so`, './build/lib/libavfilter.so.10.4.100.so'],
            [`./target/ffmpeg/lib/libavformat.so.61.7.100.so`, './build/lib/libavformat.so.61.7.100.so'],
            [`./target/ffmpeg/lib/libavutil.so.59.39.100.so`, './build/lib/libavutil.so.59.39.100.so'],
            [`./target/ffmpeg/lib/libpostproc.so.58.3.100.so`, './build/lib/libpostproc.so.58.3.100.so'],
            [`./target/ffmpeg/lib/libswresample.so.5.3.100.so`, './build/lib/libswresample.so.5.3.100.so'],
            [`./target/ffmpeg/lib/libswscale.so.8.3.100.so`, './build/lib/libswscale.so.8.3.100.so'],
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
