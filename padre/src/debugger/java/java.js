'use strict'

const eventEmitter = require('events')
const path = require('path')

const _ = require('lodash')
const walk = require('fs-walk')
const {Int64BE} = require('int64-buffer')

const javaProcess = require('./java_process')
const javaSyntax = require('../../languages/java/syntax')
const javaJNI = require('../../languages/java/jni')

const MAX_CHILD_NUMBERS_TO_PRINT = 10

class JavaDebugger extends eventEmitter {
  constructor (progName, args, options) {
    super()

    this.javaProcess = new javaProcess.JavaProcess(progName, args)

    this._pendingBreakpointMethodForClasses = {}

    this._currentThreadID = Buffer.from([
      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01
    ])

    this._handleJavaEventCommand = this._handleJavaEventCommand.bind(this)
    this._getClassesWithGeneric = this._getClassesWithGeneric.bind(this)
    this._getMethodsWithGeneric = this._getMethodsWithGeneric.bind(this)
    this._setBreakpoint = this._setBreakpoint.bind(this)
    this._breakOnClassPrepare = this._breakOnClassPrepare.bind(this)
    this._handleClassPrepareEvent = this._handleClassPrepareEvent.bind(this)
    this._handleLocationEvent = this._handleLocationEvent.bind(this)
    this._getMethodLineNumbers = this._getMethodLineNumbers.bind(this)

    this._cache = {}

    this.allJavaFiles = new Set()
  }

  setup () {
    this.javaProcess.on('padre_log', (level, str) => {
      this.emit('padre_log', level, str)
    })

    for (let dir of ['./', '/Users/stevent@kainos.com/code/third_party/java']) {
      walk.filesSync(dir, (basedir, filename) => {
        this.allJavaFiles.add(path.normalize(`${basedir}/${filename}`))
      })
    }

    this.emit('started')
  }

  async run () {
    this.javaProcess.on('request', async (commandSet, command, data) => {
      if (commandSet === 64 && command === 100) {
        return this._handleJavaEventCommand(data)
      }
      console.log('REQUEST TODO -')
      console.log({
        'commandSet': commandSet,
        'command': command,
        'data': data,
      })
    })

    await this.javaProcess.run()

    this.exe = this.javaProcess.exe

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Timeout starting node process'))
      }, 60000)

      this.on('jvmstarted', async () => {
        clearTimeout(timeout)

        // await this.javaProcess.request(15, 1, Buffer.concat([
        //   Buffer.from([0x08, 0x02]), // Suspend all on CLASS_PREPARE
        //   Buffer.from([0x00, 0x00, 0x00, 0x00]), // 0 modifiers
        // ]))

        resolve({
          'pid': 0
        })
      })
    })
  }

  async breakpointFileAndLine (file, line) {
    if (!file.endsWith('.java')) {
      this.emit('padre_log', 2, `Bad Filename: ${file}`)
      return
    }

    return new Promise(async (resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Timeout setting breakpoint'))
      }, 10000)

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
        this._pendingBreakpointMethodForClasses[classSignature] = this._pendingBreakpointMethodForClasses[classSignature] || []
        this._pendingBreakpointMethodForClasses[classSignature].push(methodName)
        status = 'PENDING'
      }

      clearTimeout(timeout)
      resolve({
        'status': status
      })
    })
  }

  async stepIn () {
    return new Promise(async (resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Timeout stepping in'))
      }, 10000)

      // TODO: Error handle
      await this.javaProcess.request(15, 1, Buffer.concat([
        Buffer.from([0x01]), // SINGLE_STEP EventKind
        Buffer.from([0x02]), // Suspend All
        Buffer.from([0x00, 0x00, 0x00, 0x06]), // 6 Modifiers
        Buffer.from([0x0a]), // Step modKind
        this._currentThreadID,
        Buffer.from([0x00, 0x00, 0x00, 0x01]), // Size LINE
        Buffer.from([0x00, 0x00, 0x00, 0x00]), // Into
        Buffer.from([0x06]), // Class Exclude (java.*)
        Buffer.from([0x00, 0x00, 0x00, 0x06, 0x6a, 0x61, 0x76, 0x61, 0x2e, 0x2a]),
        Buffer.from([0x06]), // Class Exclude (javax.*)
        Buffer.from([0x00, 0x00, 0x00, 0x07, 0x6a, 0x61, 0x76, 0x61, 0x78, 0x2e, 0x2a]),
        Buffer.from([0x06]), // Class Exclude (sun.*)
        Buffer.from([0x00, 0x00, 0x00, 0x05, 0x73, 0x75, 0x6e, 0x2e, 0x2a]),
        Buffer.from([0x06]), // Class Exclude (com.sun.*)
        Buffer.from([0x00, 0x00, 0x00, 0x09, 0x63, 0x6f, 0x6d, 0x2e, 0x73, 0x75, 0x6e, 0x2e, 0x2a]),
        Buffer.from([0x01]), // Count: Do it once only
        Buffer.from([0x00, 0x00, 0x00, 0x01]),
      ]))

      await this.javaProcess.request(1, 9)

      clearTimeout(timeout)
      resolve({})
    })
  }

  async stepOver () {
    return new Promise(async (resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Timeout stepping over'))
      }, 10000)

      // TODO: Error handle
      await this.javaProcess.request(15, 1, Buffer.concat([
        Buffer.from([0x01]), // SINGLE_STEP EventKind
        Buffer.from([0x02]), // Suspend All
        Buffer.from([0x00, 0x00, 0x00, 0x06]), // 6 Modifiers
        Buffer.from([0x0a]), // Step modKind
        this._currentThreadID,
        Buffer.from([0x00, 0x00, 0x00, 0x01]), // Size LINE
        Buffer.from([0x00, 0x00, 0x00, 0x01]), // Over
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
    const ret = this.javaProcess.request(1, 9)

    return new Promise(async (resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Timeout continuing'))
      }, 10000)

      await ret
      clearTimeout(timeout)
      resolve({})
    })
  }

  async printVariable (variableName, file, line) {
    let timeoutId

    const delay = new Promise(async (resolve, reject) => {
      timeoutId = setTimeout(() => {
        reject(new Error('Timeout printing a variable'))
      }, 10000)
    })

    return Promise.race([delay, new Promise(async (resolve, reject) => {
      const [classes, positionData] = await Promise.all([
        this._getClassesWithGeneric(),
        javaSyntax.getPositionDataAtLine(file, line)
      ])

      const className = positionData[0]
      const classSignature = javaJNI.convertClassToJNISignature(className)
      const classFound = _.get(classes.filter(x => x.signature === classSignature), '[0]')
      const classRefTypeID = classFound.refTypeID

      const methodName = positionData[1]
      const methods = await this._getMethodsWithGeneric(classRefTypeID)
      const methodFound = _.get(methods.filter(x => x.name === methodName), '[0]')
      const methodID = methodFound.methodID

      let {
        type,
        value,
      } = await this._getValueForVariable(classRefTypeID, methodID, variableName)

      clearTimeout(timeoutId)
      resolve({
        'type': type,
        'value': value,
        'variable': variableName,
      })
    })])
  }

  async _handleJavaEventCommand (data) {
    let toResume = false

    let pos = 5
    for (let i = 0; i < data.readInt32BE(1); i++) {
      const eventKind = data.readInt8(pos)
      pos += 1
      if (eventKind === 0x01 || eventKind === 0x02) {
        pos += await this._handleLocationEvent(data.slice(pos))
      } else if (eventKind === 0x08) {
        pos += await this._handleClassPrepareEvent(data.slice(pos))
        toResume = true
      } else if (eventKind === 0x5a) {
        this.emit('jvmstarted')
        pos += 12
      } else if (eventKind === 0x63) {
        this.emit('process_exit', 0, 0) // TODO: Exit Codes
        pos += 4
      } else {
        console.log('TODO: Handle eventKind ' + eventKind)
        console.log(data.slice(pos))
      }
    }

    if (toResume) {
      await this.javaProcess.request(1, 9)
    }
  }

  async _getClassesWithGeneric () {
    const ret = await this.javaProcess.request(1, 20)

    // TODO: Error Handle
    if (ret.errorCode !== 0) {
      console.log('Get all classes errorCode - ' + ret.errorCode)
    }
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
    const ret = await this._doRequestWithCache([2, 15, refTypeID])
    // TODO: Error Handle
    if (ret.errorCode !== 0) {
      console.log('Get methods errorCode - ' + ret.errorCode)
    }
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
    length.writeInt32BE(Buffer.from(className).length) // TODO: Correct length for Unicode?
    await this.javaProcess.request(15, 1, Buffer.concat([
      Buffer.from([0x08, 0x02]), // Suspend all on CLASS_PREPARE
      Buffer.from([0x00, 0x00, 0x00, 0x02]), // 2 modifiers
      Buffer.from([0x05]), // Class pattern to match
      length,
      Buffer.from(className),
      Buffer.from([0x01]), // Count 1
      Buffer.from([0x00, 0x00, 0x00, 0x01])
    ]))
  }

  async _handleClassPrepareEvent (data) {
    let pos = 0

    pos += 4
    pos += this.javaProcess.getObjectIDSize()
    pos += 1

    const refTypeID = data.slice(pos, pos + this.javaProcess.getReferenceTypeIDSize())
    pos += this.javaProcess.getReferenceTypeIDSize()

    //const classPath = await this._getPathForClass(refTypeID)
    //if (!classPath) {
    //  return
    //}

    const signatureSize = data.readInt32BE(pos)
    pos += 4
    const classSignature = data.slice(pos, pos + signatureSize).toString('utf-8')
    pos += signatureSize

    pos += 4

    // const methods = await this._getMethodsWithGeneric(refTypeID)
    //
    // const promises = []
    //
    // for (let method of methods) {
    //   promises.push(this.javaProcess.request(15, 1, Buffer.concat([
    //     Buffer.from([0x02, 0x02]),
    //     Buffer.from([0x00, 0x00, 0x00, 0x01]),
    //     Buffer.from([0x07]),
    //     Buffer.from([0x01]),
    //     refTypeID,
    //     method.methodID,
    //     Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
    //   ])))
    // }
    //
    // await Promise.all(promises)

    if (classSignature in this._pendingBreakpointMethodForClasses) {
      for (let methodName of this._pendingBreakpointMethodForClasses[classSignature]) {
        await this._setBreakpoint(refTypeID, methodName)
      }

      // TODO: Clear Class Prepare Event

      delete this._pendingBreakpointMethodForClasses[classSignature]
    }

    return pos
  }

  async _handleLocationEvent (data) {
    // TODO: Get lengths from correct place

    this._currentThreadID = data.slice(4, 12)
    const classID = data.slice(13, 21)
    const methodID = data.slice(21, 29)
    const location = data.slice(29, 37)

    const [classPath, methodLines] = await Promise.all([
      this._getPathForClass(classID),
      this._getMethodLineNumbers(classID, methodID),
    ])

    if (!classPath) {
      return await this.stepOver()
    }

    const line = _.get(_.last(methodLines[2].filter(x => {
      // Loop over every index and compare in order, to check whether the current
      // line is less than or equal to the location. e.g.
      // 00 01 02 03 04 05 06 07 < 00 02 03 04 05 06 07 08 and
      // 00 01 02 03 04 05 06 07 == 00 01 02 03 04 05 06 07 and
      for (let i = 0; i < 8; i++) {
        if (location[i] < x.lineCodeIndex[i]) {
          return false
        }
      }
      return true
    })), 'lineNumber') || _.get(methodLines, '[2][0].lineNumber')

    this.emit('process_position', classPath, line)
  }

  async _getPathForClass (classID) {
    const [classes, sourceFile] = await Promise.all([
      this._getClassesWithGeneric(),
      this._doRequestWithCache([2, 7, classID])
    ])

    if (sourceFile.errorCode !== 0) {
      return null
    }

    const classFileSize = sourceFile.data.readInt32BE()
    const classFile = sourceFile.data.slice(4, 4 + classFileSize).toString('utf-8')
    const classFound = _.get(classes.filter(x => x.refTypeID.equals(classID)), '[0]')
    const fullClassPath = classFound.signature.substr(1, classFound.signature.lastIndexOf('/')) + classFile

    const classPathsFound = [...this.allJavaFiles].filter(x => x.indexOf(fullClassPath) !== -1)
    if (classPathsFound.length === 0) {
      // TODO: Why did we think the following was good?
      // await this.javaProcess.request(1, 9)
      return null
    }

    // TODO: error logging

    if (classPathsFound.length > 1) {
      console.log('TODO: Figure out what to do with:')
      console.log(JSON.stringify(classPathsFound))
    }

    return _.get(classPathsFound, '[0]')
  }

  async _getMethodLineNumbers (classID, methodID) {
    const ret = await this._doRequestWithCache([6, 1, Buffer.concat([classID, methodID])])

    // TODO: Error Handle
    if (ret.errorCode !== 0) {
      console.log('Get methods lines errorCode - ' + ret.errorCode)
    }
    const data = ret.data

    const startLine = data.slice(0, 8)
    const endLine = data.slice(8, 16)
    const lineNumbers = []

    let pos = 20
    for (let lineNum = 0; lineNum < data.readInt32BE(16); lineNum++) {
      const lineCodeIndex = data.slice(pos, pos + 8)
      pos += 8
      const lineNumber = data.readInt32BE(pos)
      pos += 4
      lineNumbers.push({
        'lineCodeIndex': lineCodeIndex,
        'lineNumber': lineNumber,
      })
    }

    return [startLine, endLine, lineNumbers]
  }

  async _getVariablesInMethod (classID, methodID) {
    const ret = await this._doRequestWithCache([6, 5, Buffer.concat([classID, methodID])])

    if (ret.errorCode === 101) {
      // Information Absent
      return []
    } else if (ret.errorCode !== 0) {
      // TODO: Error Handle these
      console.log('Get variables in method errorCode - ' + ret.errorCode)
    }

    const data = ret.data

    let variables = []

    let pos = 8
    for (let i = 0; i < data.readInt32BE(4); i++) {
      const variable = {}
      variable.codeIndex = data.slice(pos, pos + 8)
      pos += 8

      const variableNameSize = data.readInt32BE(pos)
      pos += 4
      variable.variableName = data.slice(pos, pos + variableNameSize).toString('utf-8')
      pos += variableNameSize

      const signatureSize = data.readInt32BE(pos)
      pos += 4
      variable.signature = data.slice(pos, pos + signatureSize).toString('utf-8')
      pos += signatureSize

      const genericSignatureSize = data.readInt32BE(pos)
      pos += 4
      variable.genericSignature = data.slice(pos, pos + genericSignatureSize).toString('utf-8')
      pos += genericSignatureSize

      variable.length = data.readInt32BE(pos)
      pos += 4

      variable.slot = data.readInt32BE(pos)
      pos += 4

      variables.push(variable)
    }

    return variables
  }

  async _getVariablesInClass (classRefTypeID) {
    const ret = await this._doRequestWithCache([2, 14, Buffer.concat([classRefTypeID])])

    if (ret.errorCode !== 0) {
      // TODO: Error Handle these
      console.log('Get variables in method errorCode - ' + ret.errorCode)
    }

    const data = ret.data

    let variables = []

    let pos = 4
    for (let i = 0; i < data.readInt32BE(0); i++) {
      const variable = {}
      variable.fieldID = data.slice(pos, pos + 8)
      pos += 8

      const variableNameSize = data.readInt32BE(pos)
      pos += 4
      variable.variableName = data.slice(pos, pos + variableNameSize).toString('utf-8')
      pos += variableNameSize

      const signatureSize = data.readInt32BE(pos)
      pos += 4
      variable.signature = data.slice(pos, pos + signatureSize).toString('utf-8')
      pos += signatureSize

      const genericSignatureSize = data.readInt32BE(pos)
      pos += 4
      variable.genericSignature = data.slice(pos, pos + genericSignatureSize).toString('utf-8')
      pos += genericSignatureSize

      // Ignore modbits
      pos += 4

      // TODO: This is a bit of a hack to get rid of static variables for now.
      // Need to work out the proper way of handling this.
      if (variable.fieldID[2] === 0x7f) {
        continue
      }

      variables.push(variable)
    }

    return variables
  }

  async _getValueForVariable (classRefTypeID, methodID, variableName) {
    const variables = await this._getVariablesInMethod(classRefTypeID, methodID)
    let variable = _.get(variables.filter(x => x.variableName === variableName), '[0]')

    if (variable) {
      return this._getValueForLocalVariable(variable)
    }

    const variablesForClass = await this._getVariablesInClass(classRefTypeID)
    variable = variablesForClass.filter(x => x.variableName === variableName)[0]

    const frameID = await this._getFirstFrameID()

    let ret = await this._doRequestWithCache([16, 3, Buffer.concat([
      this._currentThreadID,
      frameID
    ])])
    // TODO: Error Handle
    if (ret.errorCode !== 0) {
      console.log('16, 3 errorCode - ' + ret.errorCode)
    }

    const thisObjectID = ret.data.slice(1, 9)

    return (await this._getValueForFieldVariables(thisObjectID, [variable.fieldID]))[0]
  }

  async _getValueForFieldVariables (objectID, fieldIDs, childNumber) {
    let resp = []

    const sizeBuffer = Buffer.alloc(4)
    sizeBuffer.writeInt32BE(fieldIDs.length)

    const ret = await this._doRequestWithCache([9, 2, Buffer.concat([
      objectID,
      sizeBuffer,
      Buffer.concat(fieldIDs),
    ])])

    // TODO: Error Handle
    if (ret.errorCode !== 0) {
      console.log('9, 2 errorCode - ' + ret.errorCode)
    }

    let pos = 4
    for (let i = 0; i < ret.data.readInt32BE(); i++) {
      const valueData = await this._getValueResponseData(ret.data.slice(pos), childNumber)
      resp.push(valueData)
      pos += valueData.size
    }

    return resp
  }

  async _getValueForLocalVariable (variable) {
    const frameID = await this._getFirstFrameID()

    let slotBuffer = Buffer.alloc(4)
    slotBuffer.writeInt32BE(variable.slot)

    let tag = variable.signature[0]

    if (tag === 'L') {
      if (variable.signature === 'Ljava/lang/String;') {
        tag = 's'
      }
    }

    let ret = await this._doRequestWithCache([16, 1, Buffer.concat([
      this._currentThreadID,
      frameID,
      Buffer.from([0x00, 0x00, 0x00, 0x01]),
      slotBuffer,
      Buffer.from(tag)
    ])], 2000)
    // TODO: Error Handle
    if (ret.errorCode !== 0) {
      console.log('16, 1 errorCode - ' + ret.errorCode)
    }

    if (ret.data.readInt32BE() !== 1) {
      console.log('TODO: We have an error')
      console.log(ret)
    }

    return this._getValueResponseData(ret.data.slice(4))
  }

  async _getValueResponseData (data, childNumber) {
    let type = data.slice(0, 1).toString('utf-8')
    let value = data.slice(1)
    let size = 1

    switch (type) {
    case 'B':
      value = data.readInt8(1)
      size += 1
      break
    case 'C':
    case 'S':
      value = data.readInt16BE(1)
      size += 2
      break
    case 'D':
      value = data.readDoubleBE(1)
      size += 8
      break
    case 'F':
      value = data.readFloatBE(1)
      size += 4
      break
    case 'I':
      value = data.readInt32BE(1)
      size += 4
      break
    case 'J':
      const longValue = new Int64BE(data.slice(1))
      value = longValue.toString(10)
      size += 8
      break
    case 'l':
      value = 'CLASSLOADER'
      size += 8
      break
    case 'L':
      value = data.slice(1, 9)
      size += 8
      if (value.equals(Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]))) {
        value = null
      } else {
        value = await this._getObjectValue(value, childNumber + 1)
      }
      break
    case 'Z':
      value = data.readInt8(1) !== 0
      size += 1
      break
    case 's':
      value = await this._getStringValue(value.slice(0, 8))
      size += 8
      break
    case '[':
      value = await this._getArrayValue(value.slice(0, 8))
      size += 8
      break
    }

    type = this._getPadreType(type)

    return {
      'size': size,
      'type': type,
      'value': value,
    }
  }

  async _getStringValue (objectID) {
    const ret = await this._doRequestWithCache([10, 1, Buffer.concat([
      objectID,
    ])], 2000)
    // TODO: Error Handle
    if (ret.errorCode !== 0) {
      console.log('10, 1 errorCode - ' + ret.errorCode)
    }

    return ret.data.slice(4).toString('utf-8')
  }

  async _getObjectValue (objectID, childNumber) {
    if (!childNumber) {
      childNumber = 0
    }

    if (childNumber === MAX_CHILD_NUMBERS_TO_PRINT) {
      return '...'
    }

    let ret = await this._doRequestWithCache([9, 1, Buffer.concat([objectID])])

    // TODO: Error Handle
    if (ret.errorCode !== 0) {
      console.log('9, 1 errorCode - ' + ret.errorCode)
    }

    const classID = ret.data.slice(1)

    const fields = await this._getVariablesInClass(classID)

    const values = await this._getValueForFieldVariables(objectID, fields.map(x => x.fieldID), childNumber)

    let resp = {}

    for (let i = 0; i < fields.length; i++) {
      resp[fields[i].variableName] = values[i].value
    }

    return resp
  }

  async _getArrayValue (arrayID) {
    let ret = await this._doRequestWithCache([13, 1, Buffer.concat([
      arrayID,
    ])], 2000)
    // TODO: Error Handle
    if (ret.errorCode !== 0) {
      console.log('13, 1 errorCode - ' + ret.errorCode)
    }

    const arrayLength = ret.data

    if (arrayLength.readInt32BE() === 0) {
      return []
    }

    ret = await this._doRequestWithCache([13, 2, Buffer.concat([
      arrayID,
      Buffer.from([0x00, 0x00, 0x00, 0x00]),
      arrayLength,
    ])], 2000)
    // TODO: Error Handle
    if (ret.errorCode !== 0) {
      console.log('13, 2 errorCode - ' + ret.errorCode)
    }

    let arrayValues = []

    const mainType = ret.data.readInt8()
    let pos = 5

    for (let i = 0; i < ret.data.readInt32BE(1); i++) {
      let valuesData
      if (mainType === 0x4c) { // Objects
        valuesData = await this._getValueResponseData(ret.data.slice(pos))
        pos += valuesData.size
      } else {
        valuesData = await this._getValueResponseData(Buffer.concat([Buffer.from([mainType]), ret.data.slice(pos)]))
        pos += valuesData.size - 1
      }
      arrayValues.push(valuesData.value)
    }

    return arrayValues
  }

  _getPadreType (type) {
    switch (type) {
    case 'B':
    case 'C':
    case 'D':
    case 'F':
    case 'I':
    case 'S':
      return 'number'
    case 'Z':
      return 'boolean'
    case 'J':
    case 'l':
    case 's':
      return 'string'
    case 'L':
    case '[':
      return 'JSON'
    }
  }

  async _getFirstFrameID () {
    let ret = await this.javaProcess.request(11, 6, Buffer.concat([
      this._currentThreadID,
      Buffer.from([0x00, 0x00, 0x00, 0x00]),
      Buffer.from([0x00, 0x00, 0x00, 0x01]),
    ]))
    // TODO: Error Handle
    if (ret.errorCode !== 0) {
      console.log('11, 6 errorCode - ' + ret.errorCode)
    }

    return ret.data.slice(4, 12)
  }

  async _doRequestWithCache (args, timeout) {
    let ret = _.get(this._cache, JSON.stringify(args))

    if (ret && new Date().getTime() > ret.timeout) {
      delete this._cache[JSON.stringify(args)]
      ret = null
    }

    if (!ret) {
      ret = await this.javaProcess.request.apply(this.javaProcess, args)

      if (ret.errorCode !== 0) {
        // Don't cache with error, may not be an error later.
        return ret
      }

      this._cache[JSON.stringify(args)] = ret
      if (timeout) {
        this._cache[JSON.stringify(args)].timeout = new Date().getTime() + timeout
      }
    }

    return ret
  }

  // async _getClassPaths () {
  //   let ret = await this.javaProcess.request(1, 13)
  //   // TODO: Error Handle
  //   if (ret.errorCode !== 0) {
  //     console.log('Get class paths errorCode - ' + ret.errorCode)
  //   }
  //   const data = ret.data

  //   const baseClassPathSize = data.readInt32BE()
  //   // const baseClassPath = data.slice(4, 4 + baseClassPathSize).toString('utf-8')

  //   let pos = 4 + baseClassPathSize

  //   ret = this._getListFromData(data.slice(pos))
  //   pos += ret[0]
  //   const classPaths = ret[1]

  //   ret = this._getListFromData(data.slice(pos))
  //   pos += ret[0]
  //   const bootClassPaths = ret[1]

  //   return [classPaths, bootClassPaths]
  // }

  // // Takes a Buffer of that contains the following and returns a list of the data
  // //   4 byte integer of the total number of total items
  // //   Repeated data consisting of:
  // //     String length,
  // //     String data
  // _getListFromData (data) {
  //   const numElements = data.readInt32BE()

  //   let ret = []
  //   let pos = 4

  //   for (let i = 0; i < numElements; i++) {
  //     const elementLength = data.readInt32BE(pos)
  //     pos += 4
  //     ret.push(data.slice(pos, pos + elementLength).toString('utf-8'))
  //     pos += elementLength
  //   }

  //   return [pos, ret]
  // }
}

module.exports = {
  JavaDebugger
}
