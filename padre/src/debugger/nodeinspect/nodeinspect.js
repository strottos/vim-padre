'use strict'

const eventEmitter = require('events')
const fs = require('fs')
const _ = require('lodash')

const nodeProcess = require('./process')
const nodeWS = require('./ws')

class NodeInspect extends eventEmitter {
  constructor (progName, args, options) {
    super()

    this.nodeProcess = new nodeProcess.NodeProcess(progName, args)
    this.nodeWS = new nodeWS.NodeWS(require('ws'))

    this._requests = {}
    this._properties = {}
    this._scripts = []
    this._pendingBreakpoints = []

    this._handleWSDataWrite = this._handleWSDataWrite.bind(this)
    this._breakpointSet = this._breakpointSet.bind(this)
    this._scriptParsed = this._scriptParsed.bind(this)
    this._paused = this._paused.bind(this)
    this._printVariable = this._printVariable.bind(this)
  }

  setup () {
    const that = this

    this.nodeWS.on('inspect_error', (error, stack) => {
      that.emit('padre_log', 2, error)
      that.emit('padre_log', 5, stack)
    })

    this.nodeProcess.on('inspect_error', (error, stack) => {
      that.emit('padre_log', 2, error)
      that.emit('padre_log', 5, stack)
    })

    this.emit('started')
  }

  async run () {
    const that = this

    this.nodeWS.on('open', async () => {
      that.nodeWS.sendToDebugger({'method': 'Runtime.enable'})
      that.nodeWS.sendToDebugger({'method': 'Debugger.enable'})
      that.nodeWS.sendToDebugger({'method': 'Runtime.runIfWaitingForDebugger'})
    })

    this.nodeWS.on('data', this._handleWSDataWrite)

    this.nodeProcess.run()

    this.exe = this.nodeProcess.exe

    this.nodeProcess.on('inspectstarted', async () => {
      await that.nodeWS.setup()
    })

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Can\'t start node process'))
      }, 2000)

      that.on('nodestarted', async (pid) => {
        clearTimeout(timeout)

        resolve({
          'pid': pid
        })
      })
    })
  }

  async breakpointFileAndLine (file, lineNum) {
    let fileFullPath
    fileFullPath = fs.realpathSync(file)

    const scriptId = _.get(this._scripts.filter((x) => {
      let xFullPath
      try {
        xFullPath = fs.realpathSync(x.location)
      } catch (error) {
        return false
      }
      return xFullPath === fileFullPath
    }), '0.id')

    if (!scriptId) {
      this._pendingBreakpoints.push({
        'fileName': fileFullPath,
        'lineNum': lineNum,
      })

      return {
        'status': 'PENDING'
      }
    }

    this.nodeWS.sendToDebugger({
      'method': 'Debugger.setBreakpoint',
      'params': {
        'location': {
          'scriptId': scriptId,
          'lineNumber': lineNum - 1,
        },
      },
    })

    const that = this

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Can\'t set breakpoint'))
      }, 2000)

      that.on('breakpoint_set', (fileName, lineNum) => {
        clearTimeout(timeout)

        resolve({
          'status': 'OK'
        })
      })
    })
  }

  async stepIn () {
    this.nodeWS.sendToDebugger({
      'method': 'Debugger.stepInto',
    })

    const that = this
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Can\'t step in'))
      }, 2000)

      that.on('stepIn', () => {
        clearTimeout(timeout)

        resolve({})
      })
    })
  }

  async stepOver () {
    this.nodeWS.sendToDebugger({
      'method': 'Debugger.stepOver',
    })

    const that = this
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Can\'t step out'))
      }, 2000)

      that.on('stepOver', () => {
        clearTimeout(timeout)

        resolve({})
      })
    })
  }

  async continue () {
    this.nodeWS.sendToDebugger({
      'method': 'Debugger.resume',
    })

    const that = this
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Can\'t continue'))
      }, 2000)

      that.on('continue', () => {
        clearTimeout(timeout)

        resolve({})
      })
    })
  }

  async printVariable (variable) {
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
      const timeout = setTimeout(() => {
        reject(new Error('Can\'t print variable'))
      }, 2000)

      that.on('printVariable', (type, variable, value) => {
        clearTimeout(timeout)

        resolve({
          'type': type,
          'variable': variable,
          'value': value,
        })
      })
    })
  }

  _handleWSDataWrite (data) {
    switch (data.method) {
    case 'Runtime.executionContextCreated':
      this._runStarted(data)
      break

    case 'Runtime.enable':
    case 'Runtime.runIfWaitingForDebugger':
      this._properties[data.method] = _.isObject(data.result) && _.isEmpty(data.result)
      break

    case 'Runtime.executionContextDestroyed':
      this.emit('process_exit', 0, this._pid)
      break

    case 'Debugger.enable':
      this._properties['Debugger.enable'] = true
      this._properties['Debugger.id'] = data.result.debuggerId
      break

    case 'Debugger.setBreakpoint':
      this._breakpointSet(data)
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
      this._printVariable(data)
      break

    case 'Debugger.scriptParsed':
      this._scriptParsed(data)
      break

    case 'Debugger.paused':
      this._paused(data)
      break
    }
  }

  _scriptParsed (data) {
    const script = {
      'id': data.params.scriptId,
      'location': data.params.url.replace('file://', ''),
    }

    this._scripts.push(script)

    const breakpoints = this._pendingBreakpoints.filter(x => x.fileName === script.location)

    for (let breakpoint of breakpoints) {
      this.nodeWS.sendToDebugger({
        'method': 'Debugger.setBreakpoint',
        'params': {
          'location': {
            'scriptId': script.id,
            'lineNumber': breakpoint.lineNum - 1,
          },
        },
      })
    }
  }

  _runStarted (data) {
    this._pid = 0
    const match = data.params.context.name.match(/^node\[(\d*)\]$/)
    if (match) {
      this._pid = parseInt(match[1])
    }
    this.emit('nodestarted', this._pid)
  }

  _breakpointSet (data) {
    if (data.error) {
      const fileName = fs.realpathSync(_.get(this._scripts.filter((x) => {
        return x.id === data.request.params.location.scriptId
      }), '0.location'))

      this.emit('padre_log', 2,
          `Couldn't set breakpoint at ${fileName}, ` +
              `line ${data.request.params.location.lineNumber + 1}: ` +
              `Error ${data.error.code}, ${data.error.message}`)
      return
    }

    const fileName = fs.realpathSync(_.get(this._scripts.filter((x) => {
      return x.id === data.result.actualLocation.scriptId
    }), '0.location'))

    this.emit('breakpoint_set', fileName, data.result.actualLocation.lineNumber + 1)
  }

  _paused (data) {
    let fileName = null

    if (_.isEmpty(data.params.callFrames[0].url)) {
      fileName = fs.realpathSync(_.get(this._scripts.filter((x) => {
        return (data.result && x.id === data.result.actualLocation.scriptId) || x.id === data.params.callFrames[0].location.scriptId // Hack, not sure why I need to do this at present for a single file script
      }), '0.location'))
    } else {
      fileName = data.params.callFrames[0].url.replace('file://', '')
    }

    this.emit('process_position',
        fileName, data.params.callFrames[0].location.lineNumber + 1)
  }

  _printVariable (data) {
    const expression = data.request.params.expression
    let type = data.result.result.type
    let value = data.result.result.value
    if (type === 'undefined') {
      value = 'undefined'
      type = 'null'
    } else if (type === 'object') {
      if (data.result.result.subtype === 'null') {
        value = 'null'
        type = 'null'
      } else {
        type = 'JSON'
      }
    }
    this.emit('printVariable', type, expression, value)
  }
}

module.exports = {
  NodeInspect
}
