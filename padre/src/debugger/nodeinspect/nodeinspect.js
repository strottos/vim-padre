'use strict'

const eventEmitter = require('events')
const fs = require('fs')
const _ = require('lodash')

const nodeProcess = require('./node_process')
const nodeWS = require('./node_ws')

class NodeInspect extends eventEmitter {
  constructor (progName, args, options) {
    super()

    this.nodeProcess = new nodeProcess.NodeProcess(progName, args)

    this._requests = {}
    this._properties = {}
    this._scripts = []

    this._handleDataWrite = this._handleDataWrite.bind(this)
    this._checkStarted = this._checkStarted.bind(this)
  }

  async setup () {
    await this.nodeProcess.setup()

    const that = this

    this.nodeProcess.on('nodestarted', async () => {
      that.nodeWS = new nodeWS.NodeWS(require('ws'))

      await that.nodeWS.setup()

      that.nodeWS.on('open', async () => {
        that.nodeWS.sendToDebugger({'method': 'Runtime.enable'})
        that.nodeWS.sendToDebugger({'method': 'Debugger.enable'})
        that.nodeWS.sendToDebugger({'method': 'Runtime.runIfWaitingForDebugger'})
      })

      that.nodeWS.on('data', this._handleDataWrite)

      that.nodeWS.on('padre_error', (error) => {
        that.emit('padre_error', error)
      })
    })
  }

  async run () {
    return new Promise((resolve, reject) => {
      resolve({
        'pid': 0
      })
    })
  }

  async breakpointFileAndLine (file, lineNum) {
    let fileFullPath
    try {
      fileFullPath = fs.realpathSync(file)
    } catch (error) {
      console.log(error)
      return
    }
    console.log('NodeInspect Input: Breakpoint')

    console.log('Scripts')
    console.log(this._scripts)

    const scriptId = _.get(this._scripts.filter((x) => {
      let xFullPath
      try {
        xFullPath = fs.realpathSync(x.location)
      } catch (error) {
        console.log(error)
        return false
      }
      return xFullPath === fileFullPath
    }), '0.id')

    this.nodeWS.sendToDebugger({
      'method': 'Debugger.setBreakpoint',
      'params': {
        'location': {
          'scriptId': scriptId,
          'lineNumber': lineNum,
        },
      },
    })

    const that = this

    return new Promise((resolve, reject) => {
      that.on('breakpoint', (breakpointId, fileName, lineNum) => {
        resolve({
          'breakpointId': breakpointId,
          'file': fileName,
          'line': lineNum,
        })
      })
    })
  }

  async stepIn () {
    console.log('NodeInspect Input: Step In')

    this.nodeWS.sendToDebugger({
      'method': 'Debugger.stepInto',
    })

    const that = this
    return new Promise((resolve, reject) => {
      that.on('stepIn', () => {
        resolve({})
      })
    })
  }

  async stepOver () {
    console.log('NodeInspect Input: Step Over')

    this.nodeWS.sendToDebugger({
      'method': 'Debugger.stepOver',
    })

    const that = this
    return new Promise((resolve, reject) => {
      that.on('stepOver', () => {
        resolve({})
      })
    })
  }

  async continue () {
    console.log('NodeInspect Input: Continue')

    this.nodeWS.sendToDebugger({
      'method': 'Debugger.resume',
    })

    const that = this
    return new Promise((resolve, reject) => {
      that.on('continue', () => {
        resolve({})
      })
    })
  }

  async printVariable (variable) {
    console.log('NodeInspect Input: Print Variable')

    this.nodeWS.sendToDebugger({
      'method': 'Debugger.evaluateOnCallFrame',
      'params': {
        'callFrameId': '{"ordinal":0,"injectedScriptId":1}',
        'expression': variable,
        'returnByValue': true,
      },
    })

    const that = this
    return new Promise((resolve, reject) => {
      that.on('printVariable', (type, variable, value) => {
        resolve({
          'type': type,
          'variable': variable,
          'value': value,
        })
      })
    })
  }

  _handleDataWrite (data) {
    console.log('Socket Data')
    console.log(data)

    let fileName = ''

    switch (data.method) {
    case 'Runtime.enable':
    case 'Runtime.runIfWaitingForDebugger':
      this._properties[data.method] = _.isObject(data.result) && _.isEmpty(data.result)
      this._checkStarted()
      break
    case 'Debugger.enable':
      this._properties['Debugger.enable'] = true
      this._properties['Debugger.id'] = data.result.debuggerId
      this._checkStarted()
      break
    case 'Debugger.setBreakpoint':
      fileName = fs.realpathSync(_.get(this._scripts.filter((x) => {
        return x.id === data.result.actualLocation.scriptId
      }), '0.location'))

      this.emit('breakpoint', data.result.breakpointId,
          fileName, data.result.actualLocation.lineNumber)
      break
    case 'Debugger.stepInto':
      this.emit('stepIn')
      break
    case 'Debugger.stepOver':
      this.emit('stepOver')
      break
    case 'Debugger.resume':
      this.emit('continue')
      break
    case 'Debugger.evaluateOnCallFrame':
      this.emit('printVariable', data.result.result.type,
          data.request.params.expression, data.result.result.value)
      break
    case 'Debugger.scriptParsed':
      this._scripts.push({
        'id': data.params.scriptId,
        'location': data.params.url,
      })
      break
    case 'Debugger.paused':
      if (_.isEmpty(data.params.callFrames[0].url)) {
        fileName = fs.realpathSync(_.get(this._scripts.filter((x) => {
          return x.id === data.result.actualLocation.scriptId
        }), '0.location'))
      } else {
        fileName = data.params.callFrames[0].url
      }

      this.emit('process_position',
          data.params.callFrames[0].location.lineNumber + 1, fileName)
      break
    }
  }

  _checkStarted () {
    if (this._properties['Debugger.enable'] &&
        this._properties['Runtime.enable'] &&
        this._properties['Runtime.runIfWaitingForDebugger'] &&
        this._properties.started !== true) {
      this.emit('started')
      this._properties.started = true
    }
  }
}

module.exports = {
  NodeInspect
}
