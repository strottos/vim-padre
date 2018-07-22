'use strict'

const chai = require('chai')
const sinon = require('sinon')

const axios = require('axios')
const WebSocket = require('ws')

const NodeWS = require.main.require('src/debugger/nodeinspect/node_ws').NodeWS

describe('Test Spawning and Debugging Node with Inspect', () => {
  let sandbox = null
  let axiosGetStub = null
  let wsStub = null
  let wsStubReturns = null

  beforeEach(() => {
    sandbox = sinon.createSandbox()

    axiosGetStub = sandbox.stub(axios, 'get')
    axiosGetStub.withArgs('http://localhost:9229/json').returns({
      'status': 200,
      'statusText': 'OK',
      'data': [
        {
          'description': 'node.js instance',
          'devtoolsFrontendUrl': 'chrome-devtools://devtools/bundled/inspector.html?experiments=true&v8only=true&ws=127.0.0.1:9229/abcd1234-abcd-1234-5678-abcd12345678',
          'faviconUrl': 'https://nodejs.org/static/favicon.ico',
          'id': 'abcd1234-abcd-1234-5678-abcd12345678',
          'title': 'index.js',
          'type': 'node',
          'url': 'file:///Users/me/code/node/index.js',
          'webSocketDebuggerUrl': 'ws://127.0.0.1:9229/abcd1234-abcd-1234-5678-abcd12345678'
        },
      ],
    })

    wsStub = sandbox.mock()
    wsStubReturns = sandbox.createStubInstance(WebSocket)
    wsStub.returns(wsStubReturns)
  })

  afterEach(() => {
    sandbox.restore()
  })

  it('should be possible to send strings to the debugger', async () => {
    const nodeWS = new NodeWS(wsStub)
    nodeWS.ws = wsStubReturns

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
    const nodeWS = new NodeWS(wsStub)

    const nodeWSEmitStub = sandbox.stub(nodeWS, 'emit')

    wsStubReturns.on.withArgs('open', sinon.match.any).callsArg(1)

    await nodeWS.setup()

    chai.expect(axiosGetStub.callCount).to.equal(1)
    chai.expect(axiosGetStub.args[0]).to.deep.equal(['http://localhost:9229/json'])

    chai.expect(wsStub.calledWithNew()).to.be.true
    chai.expect(wsStub.args[0]).to.deep.equal(['ws://localhost:9229/abcd1234-abcd-1234-5678-abcd12345678'])

    chai.expect(wsStubReturns.on.callCount).to.equal(2)
    chai.expect(wsStubReturns.on.args[0][0]).to.equal('open')
    chai.expect(wsStubReturns.on.args[1][0]).to.equal('message')

    chai.expect(nodeWSEmitStub.callCount).to.equal(1)
    chai.expect(nodeWSEmitStub.args[0][0]).to.equal('open')
  })

  it('should return data from the WebSocket', async () => {
    const nodeWS = new NodeWS(wsStub)

    const nodeWSEmitStub = sandbox.stub(nodeWS, 'emit')

    const str = `{"method":"abc","params":{"abc":"123"}}`

    nodeWS._handleSocketWrite(str)

    chai.expect(nodeWSEmitStub.callCount).to.equal(1)
    chai.expect(nodeWSEmitStub.args[0]).to.deep.equal(['data', JSON.parse(str)])
  })

  it('should return a response for a request', async () => {
    const nodeWS = new NodeWS(wsStub)
    nodeWS.ws = wsStubReturns
    nodeWS.ws.send

    const nodeWSEmitStub = sandbox.stub(nodeWS, 'emit')

    const sendToDebuggerStub = sandbox.stub(nodeWS, 'sendToDebugger')
    sendToDebuggerStub.callThrough()

    nodeWS.sendToDebugger({
      'method': 'Runtime.enable',
      'params': {
        'abc': 123
      }
    })

    nodeWS._handleSocketWrite(`{"id":1,"result":{}}`)

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

//describe('Test Errors when Spawning and Debugging Node with Inspect', () => {
//  let sandbox = null
//  let axiosGetStub = null
//  let nodeProcessStub = null
//  let nodeProcessStubOn = null
//  let wsStub = null
//  let wsStubReturns = null
//
//  beforeEach(() => {
//    sandbox = sinon.createSandbox()
//
//    nodeProcessStub = sandbox.stub(nodeProcess, 'NodeProcess')
//    nodeProcessStubOn = sandbox.stub()
//    nodeProcessStub.withArgs().returns({
//      setup: sandbox.stub(),
//      on: nodeProcessStubOn,
//    })
//    nodeProcessStubOn.withArgs('nodestarted', sinon.match.any).callsArg(1)
//
//    axiosGetStub = sandbox.stub(axios, 'get')
//
//    wsStub = sandbox.mock()
//    wsStubReturns = sandbox.createStubInstance(WebSocket)
//    wsStub.returns(wsStubReturns)
//  })
//
//  afterEach(() => {
//    sandbox.restore()
//  })
//
//  it('should throw an error when it can\'t request to node inspect', async () => {
//    const nodeWS = new nodeinspect.NodeInspect('./test', ['--arg1'], {wsLib: wsStub})
//
//    const nodeWSEmitStub = sandbox.stub(nodeWS, 'emit')
//    nodeWSEmitStub.callThrough()
//
//    axiosGetStub.withArgs('http://localhost:9229/json')
//        .returns({then: x => undefined, catch: x => x(new Error('Test Error'))})
//
//    console.log(nodeWSEmitStub.callCount)
//    await nodeWS.setup()
//
//    console.log(nodeWSEmitStub.callCount)
//    chai.expect(nodeWSEmitStub.callCount).to.equal(1)
//    chai.expect(nodeWSEmitStub.args[0]).to.deep.equal(['padre_error', 'Test Error'])
//  })
//})
