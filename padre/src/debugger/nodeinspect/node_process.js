'use strict'

const stream = require('stream')

const nodePty = require('node-pty')

class NodeProcess extends stream.Transform {
  constructor (progName, args) {
    super()

    this.progName = progName
    this.args = args
    if (!this.args) {
      this.args = []
    }

    this._id = 1
  }

  async run () {
    try {
      const exe = this.exe = nodePty.spawn('node', ['--inspect-brk', this.progName, ...this.args])

      exe.pipe(this).pipe(exe)
    } catch (error) {
      this.emit('padre_error', error.name)
    }
  }

  _transform (chunk, encoding, callback) {
    console.log('Node Write')

    let text = chunk.toString('utf-8')

    console.log(text)

    for (let line of text.trim().split('\r\n')) {
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
