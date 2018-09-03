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

    this._pendingBreakpointMethodForClassess = {}

    this._handleJavaEventCommand = this._handleJavaEventCommand.bind(this)
    this._getClassesWithGeneric = this._getClassesWithGeneric.bind(this)
    this._getMethodsWithGeneric = this._getMethodsWithGeneric.bind(this)
    this._setBreakpoint = this._setBreakpoint.bind(this)
    this._breakOnClassPrepare = this._breakOnClassPrepare.bind(this)
    this._handleClassPrepareEvent = this._handleClassPrepareEvent.bind(this)
  }

  setup () {
    this.javaProcess.on('padre_log', (level, str) => {
      this.emit('padre_log', level, str)
    })

    this.emit('started')
  }

  async run () {
    console.log('Java: Run')

    this.javaProcess.on('request', async (commandSet, command, data) => {
      if (commandSet === 64 && command === 100) {
        await this._handleJavaEventCommand(data)
        return
      }
      console.log('REQUEST')
      console.log({
        'commandSet': commandSet,
        'command': command,
        'data': data,
      })
    })

    this.javaProcess.run()

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Timeout starting node process'))
      }, 2000)

      this.on('jvmstarted', async () => {
        clearTimeout(timeout)

        resolve({
          'pid': 0
        })
      })
    })
  }

  async breakpointFileAndLine (file, line) {
    console.log(`Java: Breakpoint at ${file}:${line}`)

    if (!file.endsWith('.java')) {
      this.emit('padre_log', 2, `Bad Filename: ${file}`)
      return
    }

    return new Promise(async (resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Timeout setting breakpoint'))
      }, 2000)

      const [classes, positionData] = await Promise.all([
        this._getClassesWithGeneric(),
        javaSyntax.getPositionDataAtLine(file, line)
      ])

      const className = positionData[0]
      const methodName = positionData[1]
      const classSignature = javaJNI.convertClassToJNISignature(className)

      const classFound = _.get(classes.filter(x => x.signature === classSignature), '[0]')

      let status = 'OK'

      if (classFound) {
        await this._setBreakpoint(classFound.refTypeID, methodName)
      } else {
        await this._breakOnClassPrepare(className)
        this._pendingBreakpointMethodForClassess[classSignature] = methodName
        status = 'PENDING'
      }

      clearTimeout(timeout)
      resolve({
        'status': status
      })
    })
  }

  async stepIn () {
    console.log('Java: StepIn')

    return new Promise(async (resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Timeout stepping in'))
      }, 2000)

      // TODO: Error handle
      await this.javaProcess.request(15, 1, Buffer.concat([
        Buffer.from([0x01]),
        Buffer.from([0x02]),
        Buffer.from([0x00, 0x00, 0x00, 0x06]),
        Buffer.from([0x0a]),
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]), // TODO: Thread ID
        Buffer.from([0x00, 0x00, 0x00, 0x01]),
        Buffer.from([0x00, 0x00, 0x00, 0x00]),
        Buffer.from([0x06]), // Class Exclude (java.*)
        Buffer.from([0x00, 0x00, 0x00, 0x06, 0x6a, 0x61, 0x76, 0x61, 0x2e, 0x2a]),
        Buffer.from([0x06]), // Class Exclude (javax.*)
        Buffer.from([0x00, 0x00, 0x00, 0x07, 0x6a, 0x61, 0x76, 0x61, 0x78, 0x2e, 0x2a]),
        Buffer.from([0x06]), // Class Exclude (sun.*)
        Buffer.from([0x00, 0x00, 0x00, 0x05, 0x73, 0x75, 0x6e, 0x2e, 0x2a]),
        Buffer.from([0x06]), // Class Exclude (com.sun.*)
        Buffer.from([0x00, 0x00, 0x00, 0x09, 0x63, 0x6f, 0x6d, 0x2e, 0x73, 0x75, 0x6e, 0x2e, 0x2a]),
        Buffer.from([0x01]), // Count 1
        Buffer.from([0x00, 0x00, 0x00, 0x01]),
      ]))

      await this.javaProcess.request(1, 9)

      clearTimeout(timeout)
      resolve({})
    })
  }

  async stepOver () {
    console.log('Java: StepOver')

    return new Promise(async (resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Timeout stepping over'))
      }, 2000)

      // TODO: Error handle
      await this.javaProcess.request(15, 1, Buffer.concat([
        Buffer.from([0x01]),
        Buffer.from([0x02]),
        Buffer.from([0x00, 0x00, 0x00, 0x06]),
        Buffer.from([0x0a]),
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]), // TODO: Thread ID
        Buffer.from([0x00, 0x00, 0x00, 0x02]),
        Buffer.from([0x00, 0x00, 0x00, 0x00]),
        Buffer.from([0x06]), // Class Exclude (java.*)
        Buffer.from([0x00, 0x00, 0x00, 0x06, 0x6a, 0x61, 0x76, 0x61, 0x2e, 0x2a]),
        Buffer.from([0x06]), // Class Exclude (javax.*)
        Buffer.from([0x00, 0x00, 0x00, 0x07, 0x6a, 0x61, 0x76, 0x61, 0x78, 0x2e, 0x2a]),
        Buffer.from([0x06]), // Class Exclude (sun.*)
        Buffer.from([0x00, 0x00, 0x00, 0x05, 0x73, 0x75, 0x6e, 0x2e, 0x2a]),
        Buffer.from([0x06]), // Class Exclude (com.sun.*)
        Buffer.from([0x00, 0x00, 0x00, 0x09, 0x63, 0x6f, 0x6d, 0x2e, 0x73, 0x75, 0x6e, 0x2e, 0x2a]),
        Buffer.from([0x01]), // Count 1
        Buffer.from([0x00, 0x00, 0x00, 0x01]),
      ]))

      await this.javaProcess.request(1, 9)

      clearTimeout(timeout)
      resolve({})
    })
  }

  async continue () {
    console.log('Java: Continue')

    const ret = this.javaProcess.request(1, 9)

    return new Promise(async (resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Timeout continuing'))
      }, 2000)

      await ret
      clearTimeout(timeout)
      resolve({})
    })
  }

  async _handleJavaEventCommand (data) {
    let pos = 5
    for (let i = 0; i < data.readInt32BE(1); i++) {
      const eventKind = data.readInt8(pos)
      pos += 1
      if (eventKind === 8) {
        pos += await this._handleClassPrepareEvent(data.slice(pos))
      } else if (eventKind === 90) {
        this.emit('jvmstarted')
        pos += 12
      } else if (eventKind === 99) {
        this.emit('process_exit', 0, 0) // TODO: Exit Codes
        pos += 4
      } else {
        console.log('TODO: Handle eventKind ' + eventKind)
        console.log(data.slice(pos))
      }
    }
  }

  async _getClassesWithGeneric () {
    const ret = await this.javaProcess.request(1, 20)
    // TODO: Error Handle
    console.log('Get all classes - ' + ret.errorCode)
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
    console.log('Get methods - ' + ret.errorCode)
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

  async _setBreakpoint (refTypeID, methodName) {
    const methods = await this._getMethodsWithGeneric(refTypeID)
    const methodFound = _.get(methods.filter(x => x.name === methodName), '[0]')

    // TODO: If not methodFound??

    await this.javaProcess.request(15, 1, Buffer.concat([
      Buffer.from([0x02, 0x02]),
      Buffer.from([0x00, 0x00, 0x00, 0x01]),
      Buffer.from([0x07]),
      Buffer.from([0x01]),
      refTypeID,
      methodFound.methodID,
      Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
    ]))
  }

  async _breakOnClassPrepare (className) {
    let length = Buffer.from([0x00, 0x00, 0x00, 0x00])
    length.writeInt32BE(className.length)
    await this.javaProcess.request(15, 1, Buffer.concat([
      Buffer.from([0x08, 0x02]),
      Buffer.from([0x00, 0x00, 0x00, 0x02]),
      Buffer.from([0x05]),
      length,
      Buffer.from(className),
      Buffer.from([0x01]),
      Buffer.from([0x00, 0x00, 0x00, 0x01])
    ]))
  }

  async _handleClassPrepareEvent (data) {
    console.log(data)
    let pos = 0

    pos += 4
    pos += this.javaProcess.getObjectIDSize()
    pos += 1

    const refTypeID = data.slice(pos, pos + this.javaProcess.getReferenceTypeIDSize())
    pos += this.javaProcess.getReferenceTypeIDSize()

    const signatureSize = data.readInt32BE(pos)
    pos += 4
    const classSignature = data.slice(pos, pos + signatureSize).toString('utf-8')
    pos += signatureSize

    pos += 4

    await this._setBreakpoint(refTypeID, this._pendingBreakpointMethodForClassess[classSignature])

    delete this._pendingBreakpointMethodForClassess[classSignature]

    await this.javaProcess.request(1, 9)

    return pos
  }
}

module.exports = {
  JavaDebugger
}
