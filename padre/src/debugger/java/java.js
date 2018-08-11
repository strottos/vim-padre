'use strict'

const eventEmitter = require('events')

const javaProcess = require('./java_process')

class JavaDebugger extends eventEmitter {
  constructor (progName, args, options) {
    super()

    this.javaProcess = new javaProcess.JavaProcess(progName, args)
  }

  setup () {
    this.javaProcess.on('started', () => {
      this.emit('started')
    })

    this.javaProcess.on('padre_log', (level, str) => {
      this.emit('padre_log', level, str)
    })

    this.javaProcess.setup()
  }

  async run () {
    console.log('Java: Run')

    this.javaProcess.sendToDebugger({
      'commandSet': 1,
      'command': 1
    })

    return new Promise((resolve, reject) => {
      resolve({
        'pid': 0
      })
    })
  }
}

module.exports = {
  JavaDebugger
}
