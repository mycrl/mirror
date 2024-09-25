const { app } = require('electron')

app.whenReady().then(() => require('./dist/index.js'))
