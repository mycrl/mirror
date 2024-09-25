const { app } = require('electron')

app.whenReady().then(() => require('./out/index.js'))
