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
      this.emit('inspect_error', `${error.name}: ${error.message}`, error.stack)
    }
  }

  _transform (chunk, encoding, callback) {
    let text = chunk.toString('utf-8')

    console.log(text)

    for (let line of text.trim().split('\r\n')) {
      const match = line.match(/^Debugger listening on .*$/)
      if (match) {
        this.emit('inspectstarted')
      }
    }

    callback()
  }
}

module.exports = {
  NodeProcess
}
