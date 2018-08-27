'use strict'

const chai = require('chai')
const sinon = require('sinon')

const events = require('events')

const axios = require('axios')
const WebSocket = require('ws')

const NodeWS = require.main.require('src/debugger/nodeinspect/ws').NodeWS

describe('Test Node Inspect Network Communication', () => {
  beforeEach(() => {
    this.sandbox = sinon.createSandbox()

    this.axiosGetStub = this.sandbox.stub(axios, 'get')
    this.axiosGetStub.withArgs('http://localhost:9229/json').returns({
      'status': 200,
      'statusText': 'OK',
      'data': [
        {
          'description': 'node.js instance',
          'devtoolsFrontendUrl': 'chrome-devtools://devtools/bundled/inspector.html?' +
              'experiments=true&v8only=true&ws=127.0.0.1:9229/abcd1234-abcd-1234-5678-abcd12345678',
          'faviconUrl': 'https://nodejs.org/static/favicon.ico',
          'id': 'abcd1234-abcd-1234-5678-abcd12345678',
          'title': 'index.js',
          'type': 'node',
          'url': 'file:///Users/me/code/node/index.js',
          'webSocketDebuggerUrl': 'ws://127.0.0.1:9229/abcd1234-abcd-1234-5678-abcd12345678'
        },
      ],
    })

    this.wsStub = this.sandbox.mock()
    this.wsStubReturns = new events.EventEmitter()
    this.wsStubReturns.send = this.sandbox.stub()
    this.wsStub.returns(this.wsStubReturns)
  })

  afterEach(() => {
    this.sandbox.restore()
  })

  it('should be possible to send strings to the debugger', async () => {
    const nodeWS = new NodeWS(this.wsStub)
    nodeWS.ws = this.wsStubReturns

    await nodeWS.sendToDebugger({'method': 'test'})

    chai.expect(nodeWS.ws.send.callCount).to.equal(1)
    chai.expect(nodeWS.ws.send.args[0]).to.deep.equal(['{"id":1,"method":"test"}'])

    chai.expect(nodeWS._requests).to.deep.equal({
      '1': {
        'method': 'test',
      },
    })

    await nodeWS.sendToDebugger({'method': 'test2'})

    chai.expect(nodeWS.ws.send.callCount).to.equal(2)
    chai.expect(nodeWS.ws.send.args[1]).to.deep.equal(['{"id":2,"method":"test2"}'])

    chai.expect(nodeWS._requests).to.deep.equal({
      '1': {
        'method': 'test',
      },
      '2': {
        'method': 'test2',
      },
    })
  })

  it('should successfully open a websocket connection to nodeinspect', async () => {
    const nodeWS = new NodeWS(this.wsStub)

    const nodeWSWsOnStub = this.sandbox.stub(this.wsStubReturns, 'on')
    nodeWSWsOnStub.callThrough()
    const nodeWSEmitStub = this.sandbox.stub(nodeWS, 'emit')

    await nodeWS.setup()

    nodeWS.ws.emit('open')

    chai.expect(this.axiosGetStub.callCount).to.equal(1)
    chai.expect(this.axiosGetStub.args[0]).to.deep.equal(['http://localhost:9229/json'])

    chai.expect(this.wsStub.calledWithNew()).to.be.true
    chai.expect(this.wsStub.args[0]).to.deep.equal(['ws://localhost:9229/abcd1234-abcd-1234-5678-abcd12345678'])

    chai.expect(this.wsStubReturns.on.callCount).to.equal(2)
    chai.expect(this.wsStubReturns.on.args[0][0]).to.equal('open')
    chai.expect(this.wsStubReturns.on.args[1][0]).to.equal('message')

    chai.expect(nodeWSEmitStub.callCount).to.equal(1)
    chai.expect(nodeWSEmitStub.args[0][0]).to.equal('open')
  })

  it('should return data from the WebSocket', async () => {
    const nodeWS = new NodeWS(this.wsStub)

    const nodeWSEmitStub = this.sandbox.stub(nodeWS, 'emit')

    const str = `{"method":"abc","params":{"abc":"123"}}`

    nodeWS._handleSocketWrite(str)

    chai.expect(nodeWSEmitStub.callCount).to.equal(1)
    chai.expect(nodeWSEmitStub.args[0]).to.deep.equal(['data', JSON.parse(str)])
  })

  it('should return a response for a request', async () => {
    const nodeWS = new NodeWS(this.wsStub)
    nodeWS.ws = this.wsStubReturns

    await nodeWS.setup()

    const nodeWSEmitStub = this.sandbox.stub(nodeWS, 'emit')

    const sendToDebuggerStub = this.sandbox.stub(nodeWS, 'sendToDebugger')
    sendToDebuggerStub.callThrough()

    nodeWS.sendToDebugger({
      'method': 'Runtime.enable',
      'params': {
        'abc': 123
      }
    })

    this.wsStubReturns.emit('message', `{"id":1,"result":{}}`)

    chai.expect(nodeWSEmitStub.callCount).to.equal(1)
    chai.expect(nodeWSEmitStub.args[0]).to.deep.equal(['data', {
      'method': 'Runtime.enable',
      'request': {
        'params': {
          'abc': 123
        }
      },
      'result': {}
    }])
  })
})

describe('Test Errors when Spawning and Debugging Node with Inspect', () => {
  beforeEach(() => {
    this.sandbox = sinon.createSandbox()

    this.axiosGetStub = this.sandbox.stub(axios, 'get')

    this.wsStub = this.sandbox.mock()
    this.wsStubReturns = this.sandbox.createStubInstance(WebSocket)
    this.wsStub.returns(this.wsStubReturns)
  })

  afterEach(() => {
    this.sandbox.restore()
  })

  it('should throw an error when it can\'t do this initial HTTP request to node inspect', async () => {
    const nodeWS = new NodeWS(this.wsStub)

    const nodeWSEmitStub = this.sandbox.stub(nodeWS, 'emit')

    this.axiosGetStub.withArgs('http://localhost:9229/json')
        .rejects('Test Error', 'Test Message')

    await nodeWS.setup()

    chai.expect(nodeWSEmitStub.callCount).to.equal(1)
    chai.expect(nodeWSEmitStub.args[0][0]).to.equal('inspect_error')
    chai.expect(nodeWSEmitStub.args[0][1]).to.equal('Test Error: Test Message')
    chai.expect(nodeWSEmitStub.args[0][2]).to.match(/^Test Error.*/)
  })

  it('should throw an error when it can\'t request over the WS', async () => {
    const nodeWS = new NodeWS(this.wsStub)
    nodeWS.ws = this.wsStubReturns

    const nodeWSEmitStub = this.sandbox.stub(nodeWS, 'emit')

    nodeWS.ws.send.rejects('Test Error', 'Test Message')

    await nodeWS.sendToDebugger({
      'method': 'doesNotExist'
    })

    chai.expect(nodeWSEmitStub.callCount).to.equal(1)
    chai.expect(nodeWSEmitStub.args[0][0]).to.equal('inspect_error')
    chai.expect(nodeWSEmitStub.args[0][1]).to.equal('Test Error: Test Message')
    chai.expect(nodeWSEmitStub.args[0][2]).to.match(/^Test Error.*/)
  })

  it('should throw an error when it can\'t read JSON', async () => {
    const nodeWS = new NodeWS(this.wsStub)

    const nodeWSEmitStub = this.sandbox.stub(nodeWS, 'emit')

    const str = `{"method":"abc","params":{"abc":"123"`

    nodeWS._handleSocketWrite(str)

    chai.expect(nodeWSEmitStub.callCount).to.equal(1)
    chai.expect(nodeWSEmitStub.args[0][0]).to.equal('inspect_error')
    chai.expect(nodeWSEmitStub.args[0][1]).to.equal(
        'SyntaxError: Unexpected end of JSON input')
    chai.expect(nodeWSEmitStub.args[0][2]).to.match(/^SyntaxError.*/)
  })
})
