const { app } = require('electron')

app.whenReady().then(() => require('./src/index.js'))