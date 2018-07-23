'use strict'

const eventEmitter = require('events')

const _ = require('lodash')
const axios = require('axios')

class NodeWS extends eventEmitter {
  constructor (wsLib) {
    super()

    this._requests = {}
    this._properties = {}
    this._wsLib = wsLib

    this._handleSocketWrite = this._handleSocketWrite.bind(this)
  }

  async setup () {
    let setup
  //  try {
    setup = await axios.get('http://localhost:9229/json')
  //  } catch (error) {
  //    this.emit('padre_error', error)
  //    return
  //  }

    this.ws = new this._wsLib(`ws://localhost:9229/${setup.data[0].id}`)

    const that = this

    this.ws.on('open', async () => {
      that.emit('open')
    })

    this.ws.on('message', this._handleSocketWrite)
  }

  async sendToDebugger (data) {
    const id = _.isEmpty(this._requests) ? 1 : Math.max.apply(null, Object.keys(this._requests)) + 1
    this._requests[id] = data
    return this.ws.send(JSON.stringify(Object.assign({}, {'id': id}, data)))
  }

  _handleSocketWrite (dataString) {
    const data = JSON.parse(dataString)

    if ('id' in data) {
      const request = this._requests[data.id]
      data.request = request
      delete data.id
      data.method = request.method
      delete data.request.method
    }

    this.emit('data', data)
  }
}

module.exports = {
  NodeWS
}
