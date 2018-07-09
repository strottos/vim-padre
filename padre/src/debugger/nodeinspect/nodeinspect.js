'use strict'

const eventEmitter = require('events')
const path = require('path')
const _ = require('lodash')

const axios = require('axios')

const nodeProcess = require('./node_process')

class NodeInspect extends eventEmitter {
  constructor (progName, args, options) {
    super()

    this.nodeProcess = new nodeProcess.NodeProcess(progName, args)

    this._requests = {}
    this._properties = {}
    this._scripts = []
    this._wsLib = options.wsLib

    this._handleSocketWrite = this._handleSocketWrite.bind(this)
    this._checkStarted = this._checkStarted.bind(this)
  }

  async setup () {
    await this.nodeProcess.setup()

    const that = this

    console.log('here')
    this.nodeProcess.on('nodestarted', async () => {
      let setup
      try {
        console.log('here2')
        setup = await axios.get('http://localhost:9229/json')
      } catch (error) {
        console.log('here3')
        that.emit('padre_error', error)
        return
      }

      that.ws = new that._wsLib(`ws://localhost:9229/${setup.data[0].id}`)

      that.ws.on('open', async () => {
        that.sendToDebugger({'method': 'Runtime.enable'})
        that.sendToDebugger({'method': 'Debugger.enable'})
        that.sendToDebugger({'method': 'Runtime.runIfWaitingForDebugger'})
      })

      that.ws.on('message', this._handleSocketWrite)
    })
  }

  async sendToDebugger (data) {
    const id = _.isEmpty(this._requests) ? 1 : Math.max.apply(null, Object.keys(this._requests)) + 1
    this._requests[id] = data
    console.log('Sending to debugger')
    console.log(Object.assign({}, {'id': id}, data))
    return this.ws.send(JSON.stringify(Object.assign({}, {'id': id}, data)))
  }

  async run () {
    return new Promise((resolve, reject) => {
      resolve({
        'pid': 0
      })
    })
  }

  async breakpointFileAndLine (file, lineNum) {
    console.log('NodeInspect Input: Breakpoint')

    console.log('Scripts')
    console.log(this._scripts)

    const scriptId = _.get(this._scripts.filter((x) => {
      return path.resolve(x.location) === path.resolve(file)
    }), '0.id')

    console.log(scriptId)

    this.sendToDebugger({
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

    this.sendToDebugger({
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

    this.sendToDebugger({
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

    this.sendToDebugger({
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

    this.sendToDebugger({
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

  _handleSocketWrite (data) {
    console.log('Socket Data')
    console.log(data)
    const response = JSON.parse(data)

    if ('id' in response) {
      const request = this._requests[response.id]

      if (_.indexOf(['Runtime.enable', 'Runtime.runIfWaitingForDebugger'], request.method) !== -1) {
        this._properties[request.method] = _.isObject(response.result) && _.isEmpty(response.result)
        this._checkStarted()
      } else if (request.method === 'Debugger.enable') {
        this._properties['Debugger.enable'] = true
        this._properties['Debugger.id'] = response.result.debuggerId
        this._checkStarted()
      } else if (request.method === 'Debugger.setBreakpoint') {
        const fileName = path.resolve(_.get(this._scripts.filter((x) => {
          return x.id === response.result.actualLocation.scriptId
        }), '0.location'))

        this.emit('breakpoint', response.result.breakpointId,
            fileName, response.result.actualLocation.lineNumber)
      } else if (request.method === 'Debugger.stepInto') {
        this.emit('stepIn')
      } else if (request.method === 'Debugger.stepOver') {
        this.emit('stepOver')
      } else if (request.method === 'Debugger.resume') {
        this.emit('continue')
      } else if (request.method === 'Debugger.evaluateOnCallFrame') {
        this.emit('printVariable', response.result.result.type,
            request.params.expression, response.result.result.value)
      }
    } else {
      if (response.method === 'Debugger.scriptParsed') {
        this._scripts.push({
          'id': response.params.scriptId,
          'location': response.params.url,
        })
      } else if (response.method === 'Debugger.paused') {
        let fileName = ''

        if (_.isEmpty(response.params.callFrames[0].url)) {
          fileName = path.resolve(_.get(this._scripts.filter((x) => {
            return x.id === response.result.actualLocation.scriptId
          }), '0.location'))
        } else {
          fileName = response.params.callFrames[0].url
        }

        this.emit('process_position',
            response.params.callFrames[0].location.lineNumber + 1, fileName)
      }
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
