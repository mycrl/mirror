const { existsSync, mkdirSync, copyFileSync, unlinkSync } = require('fs')
const { execSync } = require('child_process')
const { join } = require('path')

const Args = process
    .argv
    .slice(process.argv.indexOf('--') + 1)
    .map(item=>item.replace('--', ''))
    .reduce((args,item) => Object.assign(args, {
        [item]: true
    }), {})

const Command = (cmd, options = {}) => execSync(cmd,  {
    shell: 'powershell.exe',
    stdio: process.stdio,
    cwd: __dirname,
    ...options,
})

execSync(`cargo build ${Args.release ? '--release' : ''}`)

for (const path of [
    './build', 
    './build/bin', 
    './build/lib', 
    './build/include', 
    './build/examples', 
    './build/examples/receiver', 
    './build/examples/sender'
]) {
    if (!existsSync(path)) {
        mkdirSync(path)
    }
}

copyFileSync('./examples/desktop/CMakeLists.txt', './build/examples/CMakeLists.txt')
copyFileSync('./examples/desktop/sender/CMakeLists.txt', './build/examples/sender/CMakeLists.txt')
copyFileSync('./examples/desktop/sender/main.cpp', './build/examples/sender/main.cpp')
copyFileSync('./examples/desktop/receiver/CMakeLists.txt', './build/examples/receiver/CMakeLists.txt')
copyFileSync('./examples/desktop/receiver/main.cpp', './build/examples/receiver/main.cpp')

copyFileSync('./sdk/desktop/include/mirror.h', './build/include/mirror.h')
copyFileSync('./common/include/frame.h', './build/include/frame.h')
copyFileSync(`./target/${Args.release ? 'release' : 'debug'}/mirror.dll`, './build/bin/mirror.dll')
copyFileSync(`./target/${Args.release ? 'release' : 'debug'}/mirror.dll.lib`, './build/lib/mirror.dll.lib')

if (!Args.release) {
    copyFileSync('./target/debug/mirror.pdb', './build/bin/mirror.pdb')
}

if (!existsSync('./build/bin/data')) {
    Command('Invoke-WebRequest \
        -Uri https://github.com/mycrl/distributions/releases/download/distributions/distributions-windows.zip \
        -OutFile build\\distributions-windows.zip')

    execSync('Expand-Archive -Path build\\distributions-windows.zip -DestinationPath build\\bin')
    unlinkSync('./build/distributions-windows.zip')
}

if (Args.example) {
    if (!existsSync('./examples/desktop/build')) {
        mkdirSync('./examples/desktop/build')
    }

    const cwd = join(__dirname, './examples/desktop//build')
    execSync(`cmake -DCMAKE_BUILD_TYPE=${Args.release ? 'Release' : 'Debug'} ..`, { cwd })
    execSync('cmake --build .', { cwd })

    copyFileSync('./examples/desktop/build/receiver/Debug/receiver.exe', './build/bin/receiver.exe')
    copyFileSync('./examples/desktop/build/receiver/Debug/sender.exe', './build/bin/sender.exe')
}
