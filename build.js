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
    ps = exec('$ProgressPreference = \'SilentlyContinue\';' + cmd, {
        shell: 'powershell.exe',
        cwd: __dirname,
        ...options,
    }
    )) =>
{
    ps.stdout.pipe(process.stdout)
    ps.stderr.pipe(process.stderr)

    ps.on('close', resolve)
    ps.on('error', reject)
})
    
const Replace = (file, filters) => {
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
    const BaseDistributions = 'https://github.com/mycrl/mirror/releases/download/distributions'

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

//    if (!fs.existsSync('./build/bin/data'))
//    {
//        if (!fs.existsSync('./target/obs.zip'))
//        {
//            console.log('Start download distributions...')
//            await download(`${BaseDistributions}/obs-windows-x64.zip`, './target')
//            fs.renameSync('./target/obs-windows-x64.zip', './target/obs.zip')
//        }
//
//        await (await unzipper.Open.file('./target/obs.zip')).extract({ path: './build/bin' })
//    }

    if (!fs.existsSync('./target/ffmpeg'))
    {
        if (!fs.existsSync('./target/ffmpeg.zip'))
        {
            console.log('Start download ffmpeg...')
            await download(`${BaseDistributions}/ffmpeg-windows-x64-${Args.release ? 'release' : 'debug'}.zip`,'./target')
            fs.renameSync(`./target/ffmpeg-windows-x64-${Args.release ? 'release' : 'debug'}.zip`, './target/ffmpeg.zip')
        }
        
        await (await unzipper.Open.file('./target/ffmpeg.zip')).extract({ path: './target' })
    }

    await Command(`cargo build ${Args.release ? '--release' : ''} -p mirror`)
    await Command(`cargo build ${Args.release ? '--release' : ''} -p service`)
    await Command(`cargo build ${Args.release ? '--release' : ''} -p renderer`)

    if (!fs.existsSync('./examples/desktop/build'))
    {
        fs.mkdirSync('./examples/desktop/build')
    }

    await Command(`cmake -DCMAKE_BUILD_TYPE=${Profile} ..`, { cwd: join(__dirname, './examples/desktop/build') })
    await Command(`cmake --build . --config=${Profile}`, { cwd: join(__dirname, './examples/desktop/build') })

    for (const item of [
        /* examples */
        ['./examples/desktop/src/main.cpp', './build/examples/src/main.cpp'],
        ['./examples/desktop/src/args.cpp', './build/examples/src/args.cpp'],
        ['./examples/desktop/src/args.h', './build/examples/src/args.h'],
        ['./examples/desktop/src/render.cpp', './build/examples/src/render.cpp'],
        ['./examples/desktop/src/render.h', './build/examples/src/render.h'],
        ['./examples/desktop/src/service.cpp', './build/examples/src/service.cpp'],
        ['./examples/desktop/src/service.h', './build/examples/src/service.h'],
//        ['./examples/desktop/src/wrapper.cpp', './build/examples/src/wrapper.cpp'],
//        ['./examples/desktop/src/wrapper.h', './build/examples/src/wrapper.h'],
        ['./examples/desktop/CMakeLists.txt', './build/examples/CMakeLists.txt'],
        ['./examples/desktop/README.md', './build/examples/README.md'],

        /* inculde */
        ['./sdk/renderer/include/renderer.h', './build/include/renderer.h'],
        ['./sdk/desktop/include/mirror.h', './build/include/mirror.h'],
        ['./common/include/frame.h', './build/include/frame.h'],

        /* service */
        [`./target/${Profile.toLowerCase()}/service.exe`, './build/server/mirror-service.exe'],

        /* lib */
        [`./target/${Profile.toLowerCase()}/mirror.dll.lib`, './build/lib/mirror.dll.lib'],
        [`./target/${Profile.toLowerCase()}/renderer.dll.lib`, './build/lib/renderer.dll.lib'],

        /* bin */
        ['./LIBRARYS.txt', './build/bin/LIBRARYS.txt'],
        [`./examples/desktop/build/${Profile}/example.exe`, './build/bin/example.exe'],
        [`./target/${Profile.toLowerCase()}/renderer.dll`, './build/bin/renderer.dll'],
        [`./target/${Profile.toLowerCase()}/mirror.dll`, './build/bin/mirror.dll'],
        ['./target/ffmpeg/bin/avcodec-60.dll', './build/bin/avcodec-60.dll'],
        ['./target/ffmpeg/bin/avdevice-60.dll', './build/bin/avdevice-60.dll'],
        ['./target/ffmpeg/bin/avfilter-9.dll', './build/bin/avfilter-9.dll'],
        ['./target/ffmpeg/bin/avformat-60.dll', './build/bin/avformat-60.dll'],
        ['./target/ffmpeg/bin/avutil-58.dll', './build/bin/avutil-58.dll'],
        ['./target/ffmpeg/bin/postproc-57.dll', './build/bin/postproc-57.dll'],
        ['./target/ffmpeg/bin/swresample-4.dll', './build/bin/swresample-4.dll'],
        ['./target/ffmpeg/bin/swscale-7.dll', './build/bin/swscale-7.dll'],
    ])
    {
        fs.copyFileSync(...item)
    }

    for (const item of [
        ['./target/debug/mirror.pdb', './build/bin/mirror.pdb'],
        ['./target/debug/renderer.pdb', './build/bin/renderer.pdb'],
        ['./target/debug/service.pdb', './build/server/service.pdb'],
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

    Replace('./build/examples/CMakeLists.txt', [
        ['../../sdk/renderer/include', '../include'],
        ['../../sdk/desktop/include', '../include'],
        ['../../common/include', '../include'],
        ['../../target/debug', '../lib'],
        ['../../target/release', '../lib'],
    ])
    
    /* package electron app */
    if (Args.app)
    {
        await Command('npm i', { cwd: join(__dirname, './app') })
        await Command('npm run package', { cwd: join(__dirname, './app') })
        fs.cpSync('./app/dist/win-unpacked', './build/bin', { force: true, recursive: true })
    }

    /* async block end */
})()
