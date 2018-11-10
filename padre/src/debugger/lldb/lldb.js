'use strict'

const stream = require('stream')

const nodePty = require('node-pty')

class LLDB extends stream.Transform {
  constructor (progName, args) {
    super()

    this._properties = {}

    this.args = args
    this.progName = progName
    if (!this.args) {
      this.args = []
    }
  }

  setup () {
    const exe = this.exe = nodePty.spawn('lldb', ['--', this.progName, ...this.args])

    exe.pipe(this).pipe(exe)

    exe.write(`settings set stop-line-count-after 0\n`)
    exe.write(`settings set stop-line-count-before 0\n`)
    exe.write(`settings set frame-format frame #\${frame.index}: {\${module.file.basename}{\`\${function.name-with-args}{\${frame.no-debug}\${function.pc-offset}}}}{ at \${line.file.fullpath}:\${line.number}}\\n\n`)
  }

  async run () {
    console.log('LLDB Input: Run')
    this.exe.write('break set --name main\n')
    this.exe.write('process launch\n')
    const that = this
    return new Promise((resolve, reject) => {
      that.on('process_spawn', (pid) => {
        resolve({
          'pid': pid
        })
      })
    })
  }

  async breakpointFileAndLine (file, line) {
    console.log('LLDB Input: Breakpoint')
    this.exe.write(`break set --file ${file} --line ${line}\n`)
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
    console.log('LLDB Input: Step In')
    this.exe.write('thread step-in\n')
    const that = this
    return new Promise((resolve, reject) => {
      that.on('stepIn', () => {
        resolve({})
      })
    })
  }

  async stepOver () {
    console.log('LLDB Input: Step Over')
    this.exe.write('thread step-over\n')
    const that = this
    return new Promise((resolve, reject) => {
      that.on('stepOver', () => {
        resolve({})
      })
    })
  }

  async continue () {
    console.log('LLDB Input: Continue')
    this.exe.write('thread continue\n')
    const that = this
    return new Promise((resolve, reject) => {
      that.on('continue', () => {
        resolve({})
      })
    })
  }

  async printVariable (variable) {
    console.log('LLDB Input: Print Variable')
    this.exe.write(`frame variable ${variable}\n`)
    const that = this
    return new Promise((resolve, reject) => {
      that.on('printVariable', (type, variable, value) => {
        if (type === 'int') {
          resolve({
            'type': type,
            'variable': variable,
            'value': parseInt(value),
          })
        }
      })
    })
  }

  _transform (chunk, encoding, callback) {
    console.log('LLDB Write')
    console.log(chunk.toString('utf-8'))

    let text = chunk.toString('utf-8').trim()

    for (let line of text.split('\r\n')) {
      for (let f of [
        this._checkStarted,
        this._checkPosition,
        this._checkProcessLaunched,
        this._checkProcessExited,
        this._checkBreakpointSet,
        this._checkStepIn,
        this._checkStepOver,
        this._checkContinue,
        this._checkVariable,
      ]) {
        if (f.call(this, line.trim())) {
          break
        }
      }
    }

    callback()
  }

  _checkStarted (line) {
    const match = line.match(/^Current executable set to '.*' \(.*\)\.$/)
    if (match) {
      console.log('LLDB Started')
      this.emit('started')
      return true
    }

    return false
  }

  _checkPosition (line) {
    const match = line.match(/^ *frame #\d: \S+`\S.* at (\S+):(\d+)$/)
    if (match) {
      console.log('Found position')
      const lineNum = parseInt(match[2])
      const fileName = match[1]
      this.emit('process_position', fileName, lineNum)
      return true
    }

    return false
  }

  _checkProcessLaunched (line) {
    const match = line.match(/^Process (\d+) launched: '.*' \((.*)\)$/)
    if (match) {
      console.log('LLDB Process Launched')
      this._properties.pid = match[1]
      this._properties.arch = match[2]
      this.emit('process_spawn', parseInt(this._properties.pid))
      return true
    }

    return false
  }

  _checkProcessExited (line) {
    const match = line.match(/^Process (\d+) exited with status = (\d+) \(0x[0-9a-f]*\)$/)
    if (match && match[1] === this._properties.pid) {
      console.log('LLDB Process Exited')
      this.emit('process_exit', match[2], match[1])
      return true
    }

    return false
  }

  _checkBreakpointSet (line) {
    const match = line.match(/^Breakpoint (\d+): where = \S+`\S+ \+ \d+ at (\S+):(\d+), address = 0x[0-9a-f]*$/)
    if (match) {
      console.log('LLDB Breakpoint set')
      this.emit('breakpoint', parseInt(match[1]), match[2], parseInt(match[3]))
      return true
    }

    return false
  }

  _checkStepIn (line) {
    const match = line.match(/^\* .* stop reason = step in$/)
    if (match) {
      console.log('LLDB Stepping In')
      this.emit('stepIn')
      return true
    }

    return false
  }

  _checkStepOver (line) {
    const match = line.match(/^\* .* stop reason = step over$/)
    if (match) {
      console.log('LLDB Stepping Over')
      this.emit('stepOver')
      return true
    }

    return false
  }

  _checkContinue (line) {
    const match = line.match(/^Process (\d+) resuming$/)
    if (match && match[1] === this._properties.pid) {
      console.log('LLDB Continuing')
      this.emit('continue')
      return true
    }

    return false
  }

  _checkVariable (line) {
    const match = line.match(/^\((\S+)\) (\S+) = (.*)$/)
    if (match) {
      console.log('LLDB Print')
      this.emit('printVariable', match[1], match[2], match[3])
      return true
    }

    return false
  }
}

module.exports = {
  LLDB
}
