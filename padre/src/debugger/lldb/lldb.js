const stream = require('stream')

const nodePty = require('node-pty')

class LLDB extends stream.Transform {
  constructor (progName, args) {
    super()

    this._properties = {}

    if (!args) {
      args = []
    }
    const exe = this.exe = nodePty.spawn('lldb', ['--', progName, ...args])

    exe.pipe(this).pipe(exe)
  }

  run () {
    console.log('LLDB Run')
    this.exe.write('run\n')
  }

  _transform (chunk, encoding, callback) {
    console.log('LLDB Write')
    console.log(chunk.toString('utf-8'))

    let text = chunk.toString('utf-8').trim()

    for (let line of text.split('\r\n')) {
      line = line.trim()
      let match = line.match(/^Current executable set to '.*' \(.*\)\.$/)
      if (match) {
        console.log('LLDB Started')
        this.emit('started')
      }

      match = line.match(/^Process (\d+) launched: '.*' \((.*)\)$/)
      if (match) {
        console.log('LLDB Process Launched')
        this._properties.pid = match[1]
        this._properties.arch = match[2]
        this.emit('process_spawn', this._properties.pid)
      }

      match = line.match(/^Process (\d+) exited with status = (\d+) \(0x[0-9a-f]*\)$/)
      if (match && match[1] === this._properties.pid) {
        console.log('here')
        this.emit('process_exit', match[2])
      }
    }

    callback()
  }
}

module.exports = {
  LLDB
}
