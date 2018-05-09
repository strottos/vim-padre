class Debugger {
  constructor (debugServer, connection) {
    this.debugServer = debugServer
    this.connection = connection
  }

  handle () {
    const that = this

    this.debugServer.on('started', function () {
      that.connection.on('data', function (data) {
        console.log('DebugServer Write')
        console.log(data)
        if (data.toString('utf-8').trim() === 'run') {
          that.debugServer.run()
        }
      })

      that.debugServer.on('process_spawn', function (pid) {
        that.connection.write('pid=12345\n')
      })

      that.debugServer.on('process_exit', function (exitCode) {
        that.connection.write(`exitcode=${exitCode}\n`)
      })

      // TODO: Socket termination
      // c.on('end', function() {
      //  console.log('server disconnected');
      // })
    })
  }
}

module.exports = {
  Debugger
}
