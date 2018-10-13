'use strict'

const stream = require('stream')

const nodePty = require('node-pty')

class PDB extends stream.Transform {
  constructor (progName, args) {
    super()

    this.progName = progName
    this.args = args
    if (!this.args) {
      this.args = []
    }

    this.printingVariable = null
  }

  setup () {
    this.emit('started')
  }

  async run () {
    return new Promise((resolve, reject) => {
      if (this.progName === 'python' || this.progName === 'python3') {
        this.exe = nodePty.spawn('python3', ['-m', 'pdb', ...this.args])
      } else {
        this.exe = nodePty.spawn('python3', ['-m', 'pdb', this.progName, ...this.args])
      }

      this.exe.pipe(this).pipe(this.exe)

      resolve({
        'pid': 0
      })
    })
  }

  async breakpointFileAndLine (file, line) {
    this.exe.write(`break ${file}:${line}\n`)
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
    this.exe.write('step\n')
    return {}
  }

  async stepOver () {
    this.exe.write('next\n')
    return {}
  }

  async continue () {
    this.exe.write('continue\n')
    return {}
  }

  async printVariable (variable) {
    this.printingVariable = true
    this.exe.write(`print(${variable})\n`)
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

  _transform (chunk, encoding, callback) {
    let text = chunk.toString('utf-8').trim()

    for (let line of text.split('\r\n')) {
      let match = line.match(/^> (.*)\((\d*)\)[<>\w]*\(\)$/)
      if (match) {
        const fileName = match[1]
        const lineNum = parseInt(match[2])
        this.emit('process_position', fileName, lineNum)
        continue
      }

      match = line.match(/^Breakpoint (\d*) at (.*):(\d*)$/)
      if (match) {
        this.emit('breakpoint', parseInt(match[1]), match[2], parseInt(match[3]))
        continue
      }

      match = line.match(/^print\((.*)\)$/)
      if (match && this.printingVariable === true) {
        this.printingVariable = match[1]
        continue
      }

      if (this.printingVariable) {
        this.emit('printVariable', 'string', this.printingVariable, line)
        this.printingVariable = null
        continue
      }

      match = line.match(/^The program finished and will be restarted$/)
      if (match) {
        this.emit('process_exit', '0', '0')
        if (process.env.NODE_ENV !== 'test') {
          process.exit(0)
        }
        continue
      }

      console.log(line)
    }

    callback()
  }
}

module.exports = {
  PDB
}
