'use strict'

const process = require('process')

class Debugger {
  constructor (debugServer, connection) {
    this.debugServer = debugServer
    this.connection = connection

    this._breakpoints = []

    this._writeToPadre = this._writeToPadre.bind(this)
  }

  async setup () {
    const that = this

    that.debugServer.on('padre_log', (level, str) => {
      that._writeToPadre(`["call","padre#debugger#Log",[${level},"${str.replace(/"/g, '\\"')}"]]`)
    })

    this.debugServer.on('started', () => {
      that._writeToPadre(`["call","padre#debugger#SignalPADREStarted",[]]`)

      that.connection.on('data', async (data) => {
        await that._handleRequest(data)
      })

      that.debugServer.on('process_exit', (exitCode, pid) => {
        that._writeToPadre(`["call","padre#debugger#ProcessExited",[${exitCode},${pid}]]`)
      })

      that.debugServer.on('process_position', (fileName, lineNum) => {
        that._writeToPadre(`["call","padre#debugger#JumpToPosition",["${fileName}",${lineNum}]]`)
      })

      that.debugServer.on('breakpoint_set', (fileName, lineNum) => {
        that._writeToPadre(`["call","padre#debugger#BreakpointSet",["${fileName}",${lineNum}]]`)
      })

      process.stdin.setEncoding('utf8')
      process.stdin.on('readable', () => {
        const data = process.stdin.read()
        if (data) {
          that.debugServer.exe.write(data)
        }
      })

      // TODO: Socket termination
      // c.on('end', () => {
      //  console.log('server disconnected');
      // })
    })

    await this.debugServer.setup()
  }

  async _handleRequest (data) {
    // console.log("Handling Request")
    // console.log(data.toString('utf-8'))
    try {
      const message = this._interpret(data.toString('utf-8').trim())
      if (message.cmd === 'run') {
        const ret = await this.debugServer.run()
        this._writeToPadre(`[${message.id},"OK pid=${ret.pid}"]`)
      } else if (message.cmd === 'breakpoint') {
        if ('file' in message.args && 'line' in message.args) {
          const ret = await this.debugServer.breakpointFileAndLine(message.args.file, parseInt(message.args.line))
          this._writeToPadre(`[${message.id},"${ret.status}"]`)
        }
      } else if (message.cmd === 'stepIn') {
        await this.debugServer.stepIn()
        this._writeToPadre(`[${message.id},"OK"]`)
      } else if (message.cmd === 'stepOver') {
        await this.debugServer.stepOver()
        this._writeToPadre(`[${message.id},"OK"]`)
      } else if (message.cmd === 'continue') {
        await this.debugServer.continue()
        this._writeToPadre(`[${message.id},"OK"]`)
      } else if (message.cmd === 'print') {
        const ret = await this.debugServer.printVariable(message.args.variable)
        if (ret.type === 'number') {
          this._writeToPadre(`[${message.id},"OK variable=${ret.variable} ` +
              `value=${ret.value} type=${ret.type}"]`)
        } else if (ret.type === 'string') {
          this._writeToPadre(`[${message.id},"OK variable=${ret.variable} ` +
              `value='${ret.value.replace(/"/g, '\\"')}' type=${ret.type}"]`)
        } else if (ret.type === 'JSON') {
          this._writeToPadre(`[${message.id},"OK variable=${ret.variable} ` +
              `value='${JSON.stringify(ret.value).replace(/\\/g, '\\\\').replace(/"/g, '\\"')}' type=${ret.type}"]`)
        } else if (ret.type === 'null') {
          this._writeToPadre(`[${message.id},"OK variable=${ret.variable} ` +
              `value=${ret.value} type=${ret.type}"]`)
        } else if (ret.type === 'boolean') {
          this._writeToPadre(`[${message.id},"OK variable=${ret.variable} ` +
              `value=${ret.value} type=${ret.type}"]`)
        } else {
          this._writeToPadre(`[${message.id},"ERROR"]`)
          this._writeToPadre(`["call","padre#debugger#Log",[2,` +
              `"ERROR, can\'t understand: variable=${ret.variable} ` +
              `value='${JSON.stringify(ret.value).replace(/"/g, '\\"')}' type=${ret.type}"]]`)
        }
      }
    } catch (error) {
      this._writeToPadre(`["call","padre#debugger#Log",[2,` +
          `"${error.name.replace(/"/g, '\\"')}: ${error.message.replace(/"/g, '\\"')}"]]`)
      this._writeToPadre(`["call","padre#debugger#Log",[5,` +
          `"${error.stack.replace(/"/g, '\\"')}"]]`)
    }
  }

  _interpret (request) {
    const json = JSON.parse(request)
    const text = json[1].split(' ')
    const args = {}
    text.slice(1).forEach((x) => {
      let [key, val] = x.split('=')
      args[key] = val
    })
    return {
      id: parseInt(json[0]),
      cmd: text[0],
      args: args,
    }
  }

  _writeToPadre (data) {
    // console.log('Writing')
    // console.log(data)
    this.connection.write(data)
  }
}

module.exports = {
  Debugger
}
