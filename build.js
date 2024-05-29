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

const Profile = Args.release ? 'Release' : 'Debug'
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

for (const path of [
    './build', 
    './build/bin', 
    './build/lib',
    './build/server', 
    './build/include', 
    './build/examples', 
    './build/examples/receiver', 
    './build/examples/sender'
]) {
    if (!fs.existsSync(path)) {
        fs.mkdirSync(path)
    }
}

if (!fs.existsSync('./build/bin/data')) {
    if (!fs.existsSync('./build/distributions-windows.zip')) {
        console.log('Start download distributions...')
        await Command('Invoke-WebRequest \
            -Uri https://github.com/mycrl/distributions/releases/download/distributions/distributions-windows.zip \
            -OutFile build\\distributions-windows.zip')
    }

    await Command('Expand-Archive -Path build\\distributions-windows.zip -DestinationPath build\\bin -Force')
    fs.unlinkSync('./build/distributions-windows.zip')
}

fs.copyFileSync('./examples/desktop/common.h', './build/examples/common.h')
fs.copyFileSync('./examples/desktop/CMakeLists.txt', './build/examples/CMakeLists.txt')
fs.copyFileSync('./examples/desktop/sender/main.cpp', './build/examples/sender/main.cpp')
fs.copyFileSync('./examples/desktop/receiver/main.cpp', './build/examples/receiver/main.cpp')

fs.copyFileSync('./sdk/desktop/include/mirror.h', './build/include/mirror.h')
fs.copyFileSync('./common/include/frame.h', './build/include/frame.h')

await Command(`cargo build ${Args.release ? '--release' : ''} ${Args.ffmpeg4 ? '--features ffmpeg4' : ''} -p mirror`)
await Command(`cargo build ${Args.release ? '--release' : ''} ${Args.ffmpeg4 ? '--features ffmpeg4' : ''} -p service`)

fs.copyFileSync(`./target/${Profile.toLowerCase()}/mirror.dll`, './build/bin/mirror.dll')
fs.copyFileSync(`./target/${Profile.toLowerCase()}/mirror.dll.lib`, './build/lib/mirror.dll.lib')
fs.copyFileSync(`./target/${Profile.toLowerCase()}/service.exe`, './build/server/mirror-service.exe')

if (!Args.release) {
    fs.copyFileSync('./target/debug/mirror.pdb', './build/bin/mirror.pdb')
}

if (!Args.ffmpeg4) {
    for (const item of [
        './build/bin/swresample-3.dll',
        './build/bin/avcodec-58.dll',
        './build/bin/avutil-56.dll',
    ]) {
        if (fs.existsSync(item)) {
            fs.unlinkSync(item)
        }
    }
}

if (!fs.existsSync('./examples/desktop/build')) {
    fs.mkdirSync('./examples/desktop/build')
}

await Command(`cmake -DCMAKE_BUILD_TYPE=${Profile} ..`, { cwd: join(__dirname, './examples/desktop/build') })
await Command(`cmake --build . --config=${Profile}`, { cwd: join(__dirname, './examples/desktop/build') })

fs.copyFileSync(`./examples/desktop/build/receiver/${Profile}/receiver.exe`, './build/bin/receiver.exe')
fs.copyFileSync(`./examples/desktop/build/sender/${Profile}/sender.exe`, './build/bin/sender.exe')

fs.writeFileSync('./build/examples/sender/CMakeLists.txt', 
    fs.readFileSync('./examples/desktop/sender/CMakeLists.txt')
        .toString()
        .replace('../../../sdk/desktop/include', '../../include')
        .replace('../../../common/include', '../../include')
        .replace('../../../target/debug', '../../lib')
        .replace('../../../target/release', '../../lib'))

fs.writeFileSync('./build/examples/receiver/CMakeLists.txt', 
    fs.readFileSync('./examples/desktop/receiver/CMakeLists.txt')
        .toString()
        .replace('../../../sdk/desktop/include', '../../include')
        .replace('../../../common/include', '../../include')
        .replace('../../../target/debug', '../../lib')
        .replace('../../../target/release', '../../lib'))

/* async block end */ })()
