'use strict'

const eventEmitter = require('events')

const _ = require('lodash')

const javaProcess = require('./java_process')
const javaSyntax = require('../../languages/java/syntax')
const javaJNI = require('../../languages/java/jni')

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
    if (!file.endsWith('.java')) {
      this.emit('padre_log', 2, `Bad Filename: ${file}`)
      return
    }

    // TODO: promise.all
    const classes = await this._getClassesWithGeneric()

    const ret = await javaSyntax.getPositionDataAtLine(file, line)
    const className = ret[0]
    const methodName = ret[1]
    const classSignature = javaJNI.convertClassToJNISignature(className)

    const classFound = _.get(classes.filter(x => x.signature === classSignature), '[0]')

    if (classFound) {
      const methods = await this._getMethodsWithGeneric(classFound.refTypeID)
      const methodFound = _.get(methods.filter(x => x.name === methodName), '[0]')
      await this._setBreakpoint(classFound.refTypeID, methodFound.methodID)
    }
  }

  async _getClassesWithGeneric () {
    const ret = await this.javaProcess.request(1, 20)
    // TODO: Error Handle
    const data = ret.data

    let pos = 4
    let classes = []

    for (let i = 0; i < data.readInt32BE(0); i++) {
      const clazz = {}

      clazz.refTypeTag = data.readInt8(pos)
      pos += 1

      clazz.refTypeID = data.slice(pos, pos + this.javaProcess.getReferenceTypeIDSize())
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

  async _getMethodsWithGeneric (refTypeID) {
    const ret = await this.javaProcess.request(2, 15, refTypeID)
    // TODO: Error Handle
    const data = ret.data

    let pos = 4
    let methods = []

    for (let i = 0; i < data.readInt32BE(0); i++) {
      const method = {}

      method.methodID = data.slice(pos, pos + 8)
      pos += 8

      const methodNameSize = data.readInt32BE(pos)
      pos += 4
      method.name = data.slice(pos, pos + methodNameSize).toString('utf-8')
      pos += methodNameSize

      const signatureSize = data.readInt32BE(pos)
      pos += 4
      method.signature = data.slice(pos, pos + signatureSize).toString('utf-8')
      pos += signatureSize

      const genericSignatureSize = data.readInt32BE(pos)
      pos += 4
      method.genericSignature = data.slice(pos, pos + genericSignatureSize).toString('utf-8')
      pos += genericSignatureSize

      method.modBits = data.readInt32BE(pos)
      pos += 4
      methods.push(method)
    }

    return methods
  }

  async _setBreakpoint (refTypeID, methodID) {
    const ret = await this.javaProcess.request(15, 1, Buffer.concat([
      Buffer.from([0x02, 0x02, 0x00, 0x00, 0x00, 0x01]),
      Buffer.from([0x07]),
      Buffer.from([0x01]),
      refTypeID,
      methodID,
      Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
    ]))
  }
}

module.exports = {
  JavaDebugger
}
