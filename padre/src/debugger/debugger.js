'use strict'

class Debugger {
  constructor (debugServer, connection) {
    this.debugServer = debugServer
    this.connection = connection

    this._breakpoints = []
  }

  async setup () {
    const that = this

    that.debugServer.on('padre_log', (level, str) => {
      that.connection.write(`["call","padre#debugger#Log",[${level},"${str.replace('"', '\\"')}"]]`)
    })

    this.debugServer.on('started', () => {
      that.connection.write(`["call","padre#debugger#SignalPADREStarted",[]]`)

      that.connection.on('data', async (data) => {
        await that._handleRequest(data)
      })

      that.debugServer.on('process_exit', (exitCode, pid) => {
        that.connection.write(`["call","padre#debugger#ProcessExited",[${exitCode},${pid}]]`)
      })

      that.debugServer.on('process_position', (fileName, lineNum) => {
        that.connection.write(`["call","padre#debugger#JumpToPosition",["${fileName}",${lineNum}]]`)
      })

      that.debugServer.on('breakpoint_set', (fileName, lineNum) => {
        that.connection.write(`["call","padre#debugger#BreakpointSet",["${fileName}",${lineNum}]]`)
      })

      // TODO: Socket termination
      // c.on('end', () => {
      //  console.log('server disconnected');
      // })
    })

    await this.debugServer.setup()
  }

  async _handleRequest (data) {
    console.log('DebugServer Write')
    console.log(data)
    try {
      const message = this._interpret(data.toString('utf-8').trim())
      if (message.cmd === 'run') {
        const ret = await this.debugServer.run()
        this.connection.write(`[${message.id},"OK pid=${ret.pid}"]`)
      } else if (message.cmd === 'breakpoint') {
        if ('file' in message.args && 'line' in message.args) {
          const ret = await this.debugServer.breakpointFileAndLine(message.args.file, parseInt(message.args.line))
          this.connection.write(`[${message.id},"${ret.status}"]`)
        }
      } else if (message.cmd === 'stepIn') {
        await this.debugServer.stepIn()
        this.connection.write(`[${message.id},"OK"]`)
      } else if (message.cmd === 'stepOver') {
        await this.debugServer.stepOver()
        this.connection.write(`[${message.id},"OK"]`)
      } else if (message.cmd === 'continue') {
        await this.debugServer.continue()
        this.connection.write(`[${message.id},"OK"]`)
      } else if (message.cmd === 'print') {
        if ('variable' in message.args) {
          const ret = await this.debugServer.printVariable(message.args.variable)
          this.connection.write(`[${message.id},"OK variable=${ret.variable} value=${ret.value} type=${ret.type}"]`)
        }
      }
    } catch (error) {
      this.connection.write(`["call","padre#debugger#Log",[2,"${error.name.replace('"', '\\"')}"]]`)
      this.connection.write(`["call","padre#debugger#Log",[5,"${error.stack.replace('"', '\\"')}"]]`)
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
}

module.exports = {
  Debugger
}
