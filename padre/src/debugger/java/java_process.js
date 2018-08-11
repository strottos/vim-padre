'use strict'

const stream = require('stream')

const _ = require('lodash')

const nodePty = require('node-pty')
const process = require('process')
const net = require('net')

class JavaProcess extends stream.Transform {
  constructor (progName, args) {
    super()

    this.progName = progName
    this.args = args
    if (!this.args) {
      this.args = []
    }

    this._id = 1

    this._idSizes = null

    this._handleSocketWrite = this._handleSocketWrite.bind(this)
  }

  setup () {
    if (this.progName !== 'java') {
      this.emit('padre_log', 1, 'Not a java process')
      return
    }
    this.args = [
      '-agentlib:jdwp=transport=dt_socket,address=8457,server=y',
      ...this.args] // TODO: Generic port

    try {
      const exe = this.exe = nodePty.spawn(this.progName, this.args)

      exe.pipe(this).pipe(exe)
    } catch (error) {
      this.emit('padre_log', 1, error.name)
    }

    const that = this

    this.on('started', async () => {
      const ret = await that.sendToDebugger(1, 7)

      this._idSizes = {
        'fieldIDSize': ret.data.readInt32BE(0),
        'methodIDSize': ret.data.readInt32BE(4),
        'objectIDSize': ret.data.readInt32BE(8),
        'referenceTypeIDSize': ret.data.readInt32BE(12),
        'frameIDSize': ret.data.readInt32BE(16),
      }
    })
  }

  _transform (chunk, encoding, callback) {
    let text = chunk.toString('utf-8')

    for (let line of text.trim().split('\r\n')) {
      const match = line.match(
          /^Listening for transport dt_socket at address: 8457$/)
      if (match) {
        this.connection = net.createConnection(8457)

        const that = this

        this.connection.on('connect', () => {
          that.connection.on('data', this._handleSocketWrite)

          that.connection.write('JDWP-Handshake')
        })

        this.connection.on('error', () => {
          this.emit('padre_error', 'Connection Failed')
        })
      } else {
        process.stdout.write(text + '\n')
      }
    }

    callback()
  }

  async sendToDebugger (commandSet, command, data) {
    const id = this._sendToDebugger(commandSet, command, data)

    return new Promise((resolve, reject) => {
      this.on(`response_${id}`, (errorCode, data) => {
        resolve({
          'errorCode': errorCode,
          'data': data
        })
      })
    })
  }

  _sendToDebugger (commandSet, command, data) {
    const id = this._id
    this._id += 1
    let length = 11 + _.get(data, 'length', 0)

    const buffer = Buffer.alloc(length)
    buffer.writeInt32BE(length, 0)
    buffer.writeInt32BE(id, 4)
    buffer.writeInt8(commandSet, 9)
    buffer.writeInt8(command, 10)
    if (data) {
      data.copy(buffer, 11, 0)
    }

    this.connection.write(buffer)

    return id
  }

  _handleSocketWrite (buffer) {
    if (buffer.toString('utf-8') === 'JDWP-Handshake') {
      this.emit('started')
      return
    }

    let currentBufferStart = 0
    while (currentBufferStart < buffer.length) {
      let length = buffer.readInt32BE(currentBufferStart)
      this._handleBuffer(buffer.slice(currentBufferStart, currentBufferStart + length))
      currentBufferStart += length
    }
  }

  _handleBuffer (buffer) {
    const id = buffer.readInt32BE(4)
    const isReply = !!buffer.readInt8(8)
    const data = buffer.slice(11)

    if (id === 0 && !isReply) {
      const commandSet = buffer.readInt8(9)
      const command = buffer.readInt8(10)

      console.log('Request:')
      console.log({
        commandSet: commandSet,
        command: command,
        data: data,
      })

      this.emit('request', commandSet, command, data)
    } else if (id !== 0 && isReply) {
      const errorCode = buffer.readInt16BE(9)

      this.emit(`response_${id}`, errorCode, data)
    } else {
      this.emit(`padre_log`, 1, `Can't understand data: id ${id} but reply ${isReply}`)
    }
  }
}

module.exports = {
  JavaProcess
}
