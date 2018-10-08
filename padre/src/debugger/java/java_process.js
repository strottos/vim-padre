'use strict'

const stream = require('stream')
const net = require('net')
const process = require('process')

const _ = require('lodash')

const nodePty = require('node-pty')
const getPort = require('get-port')

class JavaProcess extends stream.Transform {
  constructor (progName, args) {
    super()

    this.progName = progName
    this.args = args
    if (!this.args) {
      this.args = []
    }

    this._port = null

    this._id = 1

    this._idSizes = null

    this._socketData = null

    this._handleSocketWrite = this._handleSocketWrite.bind(this)

    this._text = ''
  }

  async run () {
    this._port = await getPort()

    if (this.progName === 'java') {
      this.args = [
        `-agentlib:jdwp=transport=dt_socket,address=${this._port},server=y`,
        ...this.args
      ] // TODO: Generic port
    } else if (this.progName === 'mvn') {
      if (this.args.indexOf('test') !== -1) {
        process.env.MAVEN_DEBUG_OPTS = `-Dmaven.surefire.debug="-agentlib:jdwp=transport=dt_socket,address=${this._port},server=y"`
        console.log('Debugging tests')
      } else if (this.args.indexOf('verify') !== -1) {
        process.env.MAVEN_DEBUG_OPTS = `-Dmaven.failsafe.debug="-agentlib:jdwp=transport=dt_socket,address=${this._port},server=y"`
        console.log('Debugging tests')
      } else {
        process.env.MAVEN_DEBUG_OPTS = `-agentlib:jdwp=transport=dt_socket,address=${this._port},server=y`
      }
      this.args.push('--batch-mode')
    } else {
      this.emit('padre_log', 1, 'Not a java process')
      return
    }

    try {
      const exe = this.exe = nodePty.spawn(this.progName, this.args)

      exe.pipe(this).pipe(exe)
    } catch (error) {
      this.emit('padre_log', 1, error.name)
    }

    const that = this

    this.on('javadebuggerstarted', async () => {
      const ret = await that.request(1, 7)

      this._idSizes = {
        'fieldIDSize': ret.data.readInt32BE(0),
        'methodIDSize': ret.data.readInt32BE(4),
        'objectIDSize': ret.data.readInt32BE(8),
        'referenceTypeIDSize': ret.data.readInt32BE(12),
        'frameIDSize': ret.data.readInt32BE(16),
      }
    })
  }

  getFieldIDSize () {
    return this._idSizes.fieldIDSize
  }

  getMethodIDSize () {
    return this._idSizes.methodIDSize
  }

  getObjectIDSize () {
    return this._idSizes.objectIDSize
  }

  getReferenceTypeIDSize () {
    return this._idSizes.referenceTypeIDSize
  }

  getFrameIDSize () {
    return this._idSizes.frameIDSize
  }

  async request (commandSet, command, data) {
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

  _transform (chunk, encoding, callback) {
    let text = chunk.toString('utf-8')

    process.stdout.write(text)

    if (this._text) {
      text = this._text + text
    }

    while (text.length) {
      const newLineCharacters = []
      if (text.indexOf('\r') !== -1) {
        newLineCharacters.push(text.indexOf('\r'))
      } else if (text.indexOf('\n') !== -1) {
        newLineCharacters.push(text.indexOf('\n'))
      } else {
        this._text += text
        break
      }
      this._text = ''

      const lineLength = Math.min.apply(null, newLineCharacters)
      const line = text.slice(0, lineLength)
      const match = line.match(
          /Listening for transport dt_socket at address: /)
      if (match) {
        this.connection = net.createConnection(this._port)

        const that = this

        this.connection.on('connect', () => {
          that.connection.on('data', this._handleSocketWrite)

          that.connection.write('JDWP-Handshake')
        })

        this.connection.on('error', () => {
          this.emit('padre_error', 'Connection Failed')
        })
      }

      if (text.slice(lineLength).indexOf('\r\n')) {
        text = text.slice(lineLength + 2)
      } else {
        text = text.slice(lineLength + 1)
      }
    }

    callback()
  }

  _sendToDebugger (commandSet, command, data) {
    // console.log('Sending')
    // console.log({
    //   'commandSet': commandSet,
    //   'command': command,
    //   'data': data
    // })
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
    if (this._socketData) {
      buffer = Buffer.concat([
        this._socketData,
        buffer
      ])

      this._socketData = null
    }

    let currentBufferStart = 0

    const match = buffer.toString('utf-8').match(/^JDWP-Handshake/)
    if (match) {
      this.emit('javadebuggerstarted')
      currentBufferStart += 14
    }

    while (currentBufferStart < buffer.length) {
      let length
      try {
        length = buffer.readInt32BE(currentBufferStart)
      } catch (error) {
        console.log(error)
        console.log(JSON.stringify(buffer))
      }

      if (buffer.length - currentBufferStart < length) {
        this._socketData = buffer.slice(currentBufferStart)
        return
      }

      this._handleBuffer(buffer.slice(currentBufferStart, currentBufferStart + length))
      currentBufferStart += length
    }
  }

  _handleBuffer (buffer) {
    const id = buffer.readInt32BE(4)
    const isReply = !!buffer.readInt8(8)
    const data = buffer.slice(11)

    if (!isReply) {
      const commandSet = buffer.readInt8(9)
      const command = buffer.readInt8(10)

      // console.log('Request:')
      // console.log({
      //   commandSet: commandSet,
      //   command: command,
      //   data: data,
      // })

      this.emit('request', commandSet, command, data)
    } else if (id !== 0 && isReply) {
      const errorCode = buffer.readInt16BE(9)

      // console.log(`Response ${id}:`)
      // console.log({
      //   errorCode: errorCode,
      //   data: data,
      // })

      this.emit(`response_${id}`, errorCode, data)
    } else {
      this.emit(`padre_log`, 1, `Can't understand data: id ${id} but reply ${isReply}`)
    }
  }
}

module.exports = {
  JavaProcess
}
