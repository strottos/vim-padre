'use strict'

const stream = require('stream')

const nodePty = require('node-pty')

class NodeProcess extends stream.Transform {
  constructor (progName, args) {
    super()

    this.args = args
    this.progName = progName
    if (!this.args) {
      this.args = []
    }

    this._id = 1
  }

  async setup () {
    const exe = this.exe = nodePty.spawn('node', ['--inspect-brk', this.progName, ...this.args])

    exe.pipe(this).pipe(exe)
  }

  _transform (chunk, encoding, callback) {
    console.log('Node Write')
    console.log(chunk.toString('utf-8'))

    let text = chunk.toString('utf-8').trim()

    for (let line of text.split('\r\n')) {
      const match = line.match(/^Debugger listening on .*$/)
      if (match) {
        console.log('Node Started')
        this.emit('nodestarted')
      }
    }

    callback()
  }
}

module.exports = {
  NodeProcess
}
