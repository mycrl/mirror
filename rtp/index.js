const dgram = require('node:dgram');
const server = dgram.createSocket('udp4');

server.on('message', (msg, info) => {
    console.log(msg)
})

server.bind(8086)
