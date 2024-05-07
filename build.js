const { execSync } = require('child_process')
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
const Command = (cmd, options = {}) => execSync(cmd,  {
    shell: 'powershell.exe',
    stdio: process.stdio,
    cwd: __dirname,
    ...options,
})

for (const path of [
    './build', 
    './build/bin', 
    './build/lib', 
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
        Command('Invoke-WebRequest \
            -Uri https://github.com/mycrl/distributions/releases/download/distributions/distributions-windows.zip \
            -OutFile build\\distributions-windows.zip')
    }

    Command('Expand-Archive -Path build\\distributions-windows.zip -DestinationPath build\\bin -Force')
    fs.unlinkSync('./build/distributions-windows.zip')
}

fs.copyFileSync('./examples/desktop/CMakeLists.txt', './build/examples/CMakeLists.txt')
fs.copyFileSync('./examples/desktop/sender/main.cpp', './build/examples/sender/main.cpp')
fs.copyFileSync('./examples/desktop/receiver/main.cpp', './build/examples/receiver/main.cpp')

fs.copyFileSync('./sdk/desktop/include/mirror.h', './build/include/mirror.h')
fs.copyFileSync('./common/include/frame.h', './build/include/frame.h')

execSync(`cargo build ${Args.release ? '--release' : ''}`)

fs.copyFileSync(`./target/${Profile.toLowerCase()}/mirror.dll`, './build/bin/mirror.dll')
fs.copyFileSync(`./target/${Profile.toLowerCase()}/mirror.dll.lib`, './build/lib/mirror.dll.lib')

if (!Args.release) {
    fs.copyFileSync('./target/debug/mirror.pdb', './build/bin/mirror.pdb')
}

if (!fs.existsSync('./examples/desktop/build')) {
    fs.mkdirSync('./examples/desktop/build')
}

Command(`cmake -DCMAKE_BUILD_TYPE=${Profile} ..`, { cwd: join(__dirname, './examples/desktop//build') })
Command(`cmake --build . --config=${Profile}`, { cwd: join(__dirname, './examples/desktop//build') })

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
