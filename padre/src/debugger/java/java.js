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

    this.javaProcess.request({
      'commandSet': 1,
      'command': 1
    })

    return new Promise((resolve, reject) => {
      resolve({
        'pid': 0
      })
    })
  }

  async breakpointFileAndLine (file, line) {
    const javaClass = file.substr(0, file.lastIndexOf('.'))
    const javaExtension = file.substr(file.lastIndexOf('.'))

    if (javaExtension !== '.java') {
      this.emit('padre_log', 2, `Bad Filename: ${file}`)
      return
    }

    const classes = await this._getClasses()

    console.log(classes)
  }

  async _getClasses () {
    const ret = await this.javaProcess.request(1, 20)
    // TODO: Error Handle
    const data = ret.data

    let pos
    let classes
    pos = 4
    classes = []
    for (let i = 0; i < data.readInt32BE(0); i++) {
      const clazz = {}

      clazz.refTypeTag = data.readInt8(pos)
      pos += 1

      clazz.typeID = data.slice(pos, pos + 8)
      pos += this.javaProcess.getReferenceTypeIDSize()

      const signatureSize = data.readInt32BE(pos)
      pos += 4
      clazz.signature = data.slice(pos, pos + signatureSize).toString('utf-8')
      pos += signatureSize

      const genericSignatureSize = data.readInt32BE(pos)
      pos += 4
      clazz.genericSignature = data.slice(pos, pos + genericSignatureSize).toString('utf-8')
      pos += genericSignatureSize

      clazz.status = data.readInt32BE(pos)
      pos += 4
      classes.push(clazz)
    }

    return classes
  }
}

module.exports = {
  JavaDebugger
}
