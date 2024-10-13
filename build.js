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
    )) =>
{
    ps.stdout.pipe(process.stdout)
    ps.stderr.pipe(process.stderr)

    ps.on('error', reject)
    ps.on('close', code => {
        code == 0 ? resolve() : reject(`exit codec: ${code}`)
    })
})

const Replace = (file, filters) =>
{
    let src = fs.readFileSync(file).toString()
    for (const item of filters)
    {
        src = src.replace(...item)
    }

    fs.writeFileSync(file, src)
}

/* async block */ void (async () =>
{
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
        './build/examples/src',
    ])
    {
        if (!fs.existsSync(path))
        {
            fs.mkdirSync(path)
        }
    }

    await Command(`cargo build ${Args.release ? '--release' : ''} -p mirror-shared`)
    await Command(`cargo build ${Args.release ? '--release' : ''} -p mirror-service`)

    /* download ffmpeg librarys for windows */
    if (process.platform == 'win32')
    {
        if (!fs.existsSync('./target/ffmpeg'))
        {
            if (!fs.existsSync('./target/ffmpeg.zip'))
            {
                const name = process.platform == 'win32' ?
                    `ffmpeg-windows-x64-${Args.release ? 'release' : 'debug'}.zip` :
                    'ffmpeg-linux-x64-release.zip'

                console.log('Start download ffmpeg...')
                await download(`${BaseDistributions}/${name}`, './target')
                fs.renameSync(`./target/${name}`, './target/ffmpeg.zip')
            }

            await (await unzipper.Open.file('./target/ffmpeg.zip')).extract({ path: './target' })
        }
    }

    if (!fs.existsSync('./examples/desktop/build'))
    {
        fs.mkdirSync('./examples/desktop/build')
    }

    await Command(`cmake -DCMAKE_BUILD_TYPE=${Profile} ..`, { cwd: join(__dirname, './examples/desktop/build') })
    await Command(`cmake --build . --config=${Profile}`, { cwd: join(__dirname, './examples/desktop/build') })

    for (const item of [
        /* examples */
        ['./examples/desktop/src/main.cpp', './build/examples/src/main.cpp'],
        ['./examples/desktop/CMakeLists.txt', './build/examples/CMakeLists.txt'],
        ['./examples/desktop/README.md', './build/examples/README.md'],

        /* inculde */
        ['./ffi/include/mirror.h', './build/include/mirror.h'],
    ])
    {
        fs.copyFileSync(...item)
    }

    if (process.platform == 'win32')
    {
        for (const item of [
            [`./examples/desktop/build/${Profile}/example.exe`, './build/bin/example.exe'],
            [`./target/${Profile.toLowerCase()}/mirror-service.exe`, './build/server/mirror-service.exe'],
            [`./target/${Profile.toLowerCase()}/mirror.dll.lib`, './build/lib/mirror.dll.lib'],
            [`./target/${Profile.toLowerCase()}/mirror.dll`, './build/bin/mirror.dll'],
            [`./target/ffmpeg/bin/avcodec-61.dll`, './build/bin/avcodec-61.dll'],
            [`./target/ffmpeg/bin/avutil-59.dll`, './build/bin/avutil-59.dll'],
            [`./target/ffmpeg/bin/swresample-5.dll`, './build/bin/swresample-5.dll'],
        ])
        {
            fs.copyFileSync(...item)
        }
    }
    else
    {
        for (const item of [
            [`./examples/desktop/build/example`, './build/bin/example'],
            [`./target/${Profile.toLowerCase()}/mirror-service`, './build/server/mirror-service'],
            process.platform == 'darwin' ? 
                [`./target/${Profile.toLowerCase()}/libmirror.dylib`, './build/bin/libmirror.dylib']: 
                [`./target/${Profile.toLowerCase()}/libmirror.so`, './build/bin/libmirror.so'],
        ])
        {
            fs.copyFileSync(...item)
        }
    }

    if (process.platform == 'win32')
    {
        for (const item of [
            ['./target/debug/mirror.pdb', './build/bin/mirror.pdb'],
            ['./target/debug/mirror_service.pdb', './build/server/mirror-service.pdb'],
        ])
        {
            if (!Args.release)
            {
                fs.copyFileSync(...item)
            }
            else
            {
                fs.rmSync(item[1], { force: true, recursive: true })
            }
        }
    }

    Replace('./build/examples/CMakeLists.txt', [
        ['../../sdk/renderer/include', '../include'],
        ['../../sdk/desktop/include', '../include'],
        ['../../frame/include', '../include'],
        ['../../target/debug', '../lib'],
        ['../../target/release', '../lib'],
    ])

    /* package electron app */
    if (Args.app)
    {
        await Command('npm i', { cwd: join(__dirname, './app') })
        await Command('npm run package', { cwd: join(__dirname, './app') })

        let output = process.platform == 'win32' ? 
            'win-unpacked' : 
            (process.platform == 'darwin' ? 'mac-arm64' : 'linux-unpacked')
        fs.cpSync(`./app/dist/${output}`, './build/bin', { force: true, recursive: true })
    }

    /* async block end */
})().catch(console.error)
