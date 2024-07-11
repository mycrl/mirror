const { exec } = require('child_process')
const { join } = require('path')
const fs = require('fs')

const Args = process
    .argv
    .slice(process.argv.indexOf('--') + 1)
    .map(item=>item.replace('--', ''))
    .reduce((args,item) => Object.assign(args, {
        [item]: true
    }), {})

const Command = (cmd, options = {}) => new Promise((
    resolve, 
    reject, 
    ps = exec('$ProgressPreference = \'SilentlyContinue\';' + cmd,  {
        shell: 'powershell.exe',
        cwd: __dirname,
        ...options,
    }
)) => {
    ps.stdout.pipe(process.stdout)
    ps.stderr.pipe(process.stderr)
    
    ps.on('close', resolve)
    ps.on('error', reject)
})

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
    './build/examples/src',
]) {
    if (!fs.existsSync(path)) {
        fs.mkdirSync(path)
    }
}

if (!fs.existsSync('./build/bin/data')) {
    if (!fs.existsSync('./target/obs.zip')) {
        console.log('Start download distributions...')
        await Command('Invoke-WebRequest \
            -Uri https://github.com/mycrl/distributions/releases/download/distributions/obs-windows-x64.zip \
            -OutFile target\\obs.zip')
    }

    await Command('Expand-Archive -Path target\\obs.zip -DestinationPath build\\bin -Force')
}

if (!fs.existsSync('./target/ffmpeg')) {
    console.log('Start download ffmpeg...')
    await Command(`Invoke-WebRequest \
        -Uri https://github.com/mycrl/distributions/releases/download/distributions/ffmpeg-windows-x64-${Args.release ? 'release' : 'debug'}.zip \
        -OutFile target\\ffmpeg.zip`)

    await Command('Expand-Archive -Path target\\ffmpeg.zip -DestinationPath target -Force')
}

await Command(`cargo build ${Args.release ? '--release' : ''} -p mirror`)
await Command(`cargo build ${Args.release ? '--release' : ''} -p service`)

if (!fs.existsSync('./examples/desktop/build')) {
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
    ['./examples/desktop/CMakeLists.txt', './build/examples/CMakeLists.txt'],
    ['./examples/desktop/README.md', './build/examples/README.md'],
    
    /* inculde */
    ['./sdk/desktop/include/mirror.h', './build/include/mirror.h'],
    ['./common/include/frame.h', './build/include/frame.h'],
    
    /* service */
    [`./target/${Profile.toLowerCase()}/service.exe`, './build/server/mirror-service.exe'],
    
    /* bin */
    [`./examples/desktop/build/${Profile}/example.exe`, './build/bin/example.exe'],
    [`./target/${Profile.toLowerCase()}/mirror.dll`, './build/bin/mirror.dll'],
    [`./target/${Profile.toLowerCase()}/mirror.dll.lib`, './build/lib/mirror.dll.lib'],
    ['./target/ffmpeg/bin/avcodec-60.dll', './build/bin/avcodec-60.dll'],
    ['./target/ffmpeg/bin/avdevice-60.dll', './build/bin/avdevice-60.dll'],
    ['./target/ffmpeg/bin/avfilter-9.dll', './build/bin/avfilter-9.dll'],
    ['./target/ffmpeg/bin/avformat-60.dll', './build/bin/avformat-60.dll'],
    ['./target/ffmpeg/bin/avutil-58.dll', './build/bin/avutil-58.dll'],
    ['./target/ffmpeg/bin/postproc-57.dll', './build/bin/postproc-57.dll'],
    ['./target/ffmpeg/bin/swresample-4.dll', './build/bin/swresample-4.dll'],
    ['./target/ffmpeg/bin/swscale-7.dll', './build/bin/swscale-7.dll'],
]) {
    fs.copyFileSync(...item)
}

if (!Args.release) {
    fs.copyFileSync('./target/debug/mirror.pdb', './build/bin/mirror.pdb')
    fs.copyFileSync('./target/debug/service.pdb', './build/server/service.pdb')
}

fs.writeFileSync('./build/examples/CMakeLists.txt', 
    fs.readFileSync('./examples/desktop/CMakeLists.txt')
        .toString()
        .replace('../../sdk/desktop/include', '../include')
        .replace('../../common/include', '../include')
        .replace('../../target/debug', '../lib')
        .replace('../../target/release', '../lib'))

/* async block end */ })()
