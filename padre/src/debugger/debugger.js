'use strict'

class Debugger {
  constructor (debugServer, connection) {
    this.debugServer = debugServer
    this.connection = connection
  }

  async handle () {
    await this.debugServer.setup()

    const that = this

    this.debugServer.on('started', () => {
      that.connection.write(`["call","padre#debugger#SignalPADREStarted",[]]`)

      that.connection.on('data', async (data) => {
        await that._handleReadData(data)
      })

      that.debugServer.on('process_exit', function (exitCode, pid) {
        that.connection.write(`["call","padre#debugger#ProcessExited",[${exitCode},${pid}]]`)
      })

      that.debugServer.on('process_position', function (lineNum, fileName) {
        that.connection.write(`["call","padre#debugger#JumpToPosition",[${lineNum},"${fileName}"]]`)
      })

      that.debugServer.on('padre_log', function (level, string) {
        that.connection.write(`["call","padre#debugger#Log",[${level},"${string}"]]`)
      })

      // TODO: Socket termination
      // c.on('end', function() {
      //  console.log('server disconnected');
      // })
    })
  }

  async _handleReadData (data) {
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
          this.connection.write(`[${message.id},"OK line=${ret.line} file=${ret.file}"]`)
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
      this.connection.write(`["call","padre#debugger#Error",["${error}"]]`)
    }
  }

  _interpret (request) {
    const json = JSON.parse(request)
    const text = json[1].split(' ')
    const args = {}
    text.slice(1).forEach(function (x) {
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
