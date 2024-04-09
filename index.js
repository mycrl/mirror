const dgram = require('node:dgram')
const server = dgram.createSocket('udp4')

server.on('message', msg => {
    console.log(msg)
})

server.bind(8080)
// server.addMembership("224.0.0.1")
