'use strict'

const chai = require('chai')
const sinon = require('sinon')

const axios = require('axios')
const WebSocket = require('ws')

const nodeinspect = require.main.require('src/debugger/nodeinspect/nodeinspect')
const nodeProcess = require.main.require('src/debugger/nodeinspect/node_process')

describe('Test Spawning and Debugging Node with Inspect', () => {
  let sandbox = null
  let axiosGetStub = null
  let nodeProcessStub = null
  let nodeProcessStubOn = null
  let wsStub = null
  let wsStubReturns = null

  beforeEach(() => {
    sandbox = sinon.createSandbox()

    nodeProcessStub = sandbox.stub(nodeProcess, 'NodeProcess')
    nodeProcessStubOn = sandbox.stub()
    nodeProcessStub.withArgs().returns({
      setup: sandbox.stub(),
      on: nodeProcessStubOn,
    })
    nodeProcessStubOn.withArgs('nodestarted', sinon.match.any).callsArg(1)

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
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'], {wsLib: wsStub})
    nodeDebugger.ws = wsStubReturns

    await nodeDebugger.sendToDebugger({'method': 'test'})

    chai.expect(nodeDebugger.ws.send.callCount).to.equal(1)
    chai.expect(nodeDebugger.ws.send.args[0]).to.deep.equal(['{"id":1,"method":"test"}'])

    chai.expect(nodeDebugger._requests).to.deep.equal({
      '1': {
        'method': 'test',
      },
    })

    await nodeDebugger.sendToDebugger({'method': 'test2'})

    chai.expect(nodeDebugger.ws.send.callCount).to.equal(2)
    chai.expect(nodeDebugger.ws.send.args[1]).to.deep.equal(['{"id":2,"method":"test2"}'])

    chai.expect(nodeDebugger._requests).to.deep.equal({
      '1': {
        'method': 'test',
      },
      '2': {
        'method': 'test2',
      },
    })
  })

  it('should successfully open a websocket connection to nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'], {wsLib: wsStub})

    wsStubReturns.on.withArgs('open', sinon.match.any).callsArg(1)

    sandbox.stub(nodeDebugger, 'sendToDebugger')

    await nodeDebugger.setup()

    chai.expect(axiosGetStub.callCount).to.equal(1)
    chai.expect(axiosGetStub.args[0]).to.deep.equal(['http://localhost:9229/json'])

    chai.expect(wsStub.calledWithNew()).to.be.true
    chai.expect(wsStub.args[0]).to.deep.equal(['ws://localhost:9229/abcd1234-abcd-1234-5678-abcd12345678'])

    chai.expect(wsStubReturns.on.callCount).to.equal(2)
    chai.expect(wsStubReturns.on.args[0][0]).to.equal('open')
    chai.expect(wsStubReturns.on.args[1][0]).to.equal('message')
  })

  it('should successfully setup the new websocket', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'], {wsLib: wsStub})

    const nodeDebuggerEmitStub = sandbox.stub(nodeDebugger, 'emit')
    nodeDebuggerEmitStub.callThrough()

    wsStubReturns.on.withArgs('open', sinon.match.any).callsArg(1)

    const sendToDebuggerStub = sandbox.stub(nodeDebugger, 'sendToDebugger')
    sendToDebuggerStub.callThrough()

    await nodeDebugger.setup()

    chai.expect(sendToDebuggerStub.callCount).to.equal(3)
    chai.expect(sendToDebuggerStub.args[0]).to.deep.equal([{'method': 'Runtime.enable'}])
    chai.expect(sendToDebuggerStub.args[1]).to.deep.equal([{'method': 'Debugger.enable'}])
    chai.expect(sendToDebuggerStub.args[2]).to.deep.equal([{'method': 'Runtime.runIfWaitingForDebugger'}])

    nodeDebugger._handleSocketWrite(`{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"origin":"","name":"node[12345]","auxData":{"isDefault":true}}}}`)
    nodeDebugger._handleSocketWrite(`{"id":1,"result":{}}`)
    nodeDebugger._handleSocketWrite(`{"id":2,"result":{"debuggerId":"(ABCD1234ABCD1234ABCD1234ABCD1234)"}}`)
    nodeDebugger._handleSocketWrite(`{"id":3,"result":{}}`)

    chai.expect(nodeDebugger._properties['Runtime.enable']).to.be.true
    chai.expect(nodeDebugger._properties['Debugger.enable']).to.be.true
    chai.expect(nodeDebugger._properties['Debugger.id']).to.equal('(ABCD1234ABCD1234ABCD1234ABCD1234)')
    chai.expect(nodeDebugger._properties['Runtime.runIfWaitingForDebugger']).to.be.true

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0]).to.deep.equal(['started'])

    // Check that we're not calling every time
    await nodeDebugger.sendToDebugger({'method': 'Profiler.enable'})
    nodeDebugger._handleSocketWrite(`{"id":4,"result":{}}`)

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0]).to.deep.equal(['started'])
  })

  it('should record the scripts reported by nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'], {wsLib: wsStub})

    nodeDebugger._handleSocketWrite(`{"method":"Debugger.scriptParsed","params":{"scriptId":"11","url":"internal/bootstrap/loaders.js","startLine":0,"startColumn":0,"endLine":304,"endColumn":0,"executionContextId":1,"hash":"0aec6b749f65bb445e6145d4816e12e006c7b3dd","executionContextAuxData":{"isDefault":true},"isLiveEdit":false,"sourceMapURL":"","hasSourceURL":false,"isModule":false,"length":10214}}`)
    nodeDebugger._handleSocketWrite(`{"method":"Debugger.scriptParsed","params":{"scriptId":"12","url":"/Users/me/padre/padre","startLine":0,"startColumn":0,"endLine":73,"endColumn":3,"executionContextId":1,"hash":"150f6b9d1c0af7770d85d08aa052982f4676e587","executionContextAuxData":{"isDefault":true},"isLiveEdit":false,"sourceMapURL":"","hasSourceURL":false,"isModule":false,"length":1921}}`)

    chai.expect(nodeDebugger._scripts).to.deep.equal([
      {
        'id': '11',
        'location': 'internal/bootstrap/loaders.js'
      },
      {
        'id': '12',
        'location': '/Users/me/padre/padre'
      },
    ])
  })

  it('should be able to launch a process and report it', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'], {wsLib: wsStub})
    nodeDebugger.setup()

    const sendToDebuggerStub = sandbox.stub(nodeDebugger, 'sendToDebugger')
    sendToDebuggerStub.callThrough()

    const runPromise = nodeDebugger.run()

    const ret = await runPromise

    chai.expect(ret).to.deep.equal({'pid': 0})
  })

  it('should allow the debugger to set a breakpoint in nodeinspect for an existing script', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'], {wsLib: wsStub})
    nodeDebugger.ws = wsStubReturns
    nodeDebugger.setup()

    const sendToDebuggerStub = sandbox.stub(nodeDebugger, 'sendToDebugger')
    sendToDebuggerStub.callThrough()
    nodeDebugger._handleSocketWrite(`{"method":"Debugger.scriptParsed","params":{"scriptId":"67","url":"${process.cwd()}/index.js","startLine":0,"startColumn":0,"endLine":304,"endColumn":0,"executionContextId":1,"hash":"0aec6b749f65bb445e6145d4816e12e006c7b3dd","executionContextAuxData":{"isDefault":true},"isLiveEdit":false,"sourceMapURL":"","hasSourceURL":false,"isModule":false,"length":10214}}`)

    const breakpointPromise = nodeDebugger.breakpointFileAndLine('index.js', 20)

    chai.expect(sendToDebuggerStub.callCount).to.equal(1)
    chai.expect(sendToDebuggerStub.args[0][0]).to.deep.equal({
      'method': 'Debugger.setBreakpoint',
      'params': {
        'location': {
          'scriptId': '67',
          'lineNumber': 20,
        },
      }
    })

    nodeDebugger._handleSocketWrite(`{"id":1,"result":{"breakpointId":"4:20:0:67","actualLocation":{"scriptId":"67","lineNumber":31,"columnNumber":3}}}`)

    const ret = await breakpointPromise

    chai.expect(ret).to.deep.equal({
      'breakpointId': '4:20:0:67',
      'line': 31,
      'file': `${process.cwd()}/index.js`,
    })
  })

  it('should allow the debugger to set a pending breakpoint in nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'], {wsLib: wsStub})
    nodeDebugger.setup()

    const sendToDebuggerStub = sandbox.stub(nodeDebugger, 'sendToDebugger')
    sendToDebuggerStub.callThrough()

    // TODO: May need to add support at the debugger level
  })

  it('should allow the debugger to step in in nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'], {wsLib: wsStub})
    nodeDebugger.ws = wsStubReturns
    nodeDebugger.setup()

    const sendToDebuggerStub = sandbox.stub(nodeDebugger, 'sendToDebugger')
    sendToDebuggerStub.callThrough()

    const stepInPromise = nodeDebugger.stepIn()

    chai.expect(sendToDebuggerStub.callCount).to.equal(1)
    chai.expect(sendToDebuggerStub.args[0][0]).to.deep.equal({
      'method': 'Debugger.stepInto',
    })

    nodeDebugger._handleSocketWrite(`{"id":1,"result":{}}`)

    const ret = await stepInPromise

    chai.expect(ret).to.deep.equal({})
  })

  it('should allow the debugger to step over in nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'], {wsLib: wsStub})
    nodeDebugger.ws = wsStubReturns
    nodeDebugger.setup()

    const sendToDebuggerStub = sandbox.stub(nodeDebugger, 'sendToDebugger')
    sendToDebuggerStub.callThrough()

    const stepOverPromise = nodeDebugger.stepOver()

    chai.expect(sendToDebuggerStub.callCount).to.equal(1)
    chai.expect(sendToDebuggerStub.args[0][0]).to.deep.equal({
      'method': 'Debugger.stepOver',
    })

    nodeDebugger._handleSocketWrite(`{"id":1,"result":{}}`)

    const ret = await stepOverPromise

    chai.expect(ret).to.deep.equal({})
  })

  it('should allow the debugger to continue in nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'], {wsLib: wsStub})
    nodeDebugger.ws = wsStubReturns
    nodeDebugger.setup()

    const sendToDebuggerStub = sandbox.stub(nodeDebugger, 'sendToDebugger')
    sendToDebuggerStub.callThrough()

    const continuePromise = nodeDebugger.continue()

    chai.expect(sendToDebuggerStub.callCount).to.equal(1)
    chai.expect(sendToDebuggerStub.args[0][0]).to.deep.equal({
      'method': 'Debugger.resume',
    })

    nodeDebugger._handleSocketWrite(`{"id":1,"result":{}}`)

    const ret = await continuePromise

    chai.expect(ret).to.deep.equal({})
  })

  it('should allow the debugger to print integers in nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'], {wsLib: wsStub})
    nodeDebugger.ws = wsStubReturns
    nodeDebugger.setup()

    const sendToDebuggerStub = sandbox.stub(nodeDebugger, 'sendToDebugger')
    sendToDebuggerStub.callThrough()

    const printVariablePromise = nodeDebugger.printVariable('abc')

    chai.expect(sendToDebuggerStub.callCount).to.equal(1)
    chai.expect(sendToDebuggerStub.args[0][0]).to.deep.equal({
      'method': 'Debugger.evaluateOnCallFrame',
      'params': {
        'callFrameId': '{"ordinal":0,"injectedScriptId":1}',
        'expression': 'abc',
        'returnByValue': true,
      },
    })

    nodeDebugger._handleSocketWrite(`{
      "id": 1,
      "result": {
        "result": {
          "description": "123",
          "type": "number",
          "value": 123
        }
      }
    }`)

    const ret = await printVariablePromise

    chai.expect(ret).to.deep.equal({
      'type': 'number',
      'value': 123,
      'variable': 'abc',
    })
  })

  it('should report the current position when reported by nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'], {wsLib: wsStub})
    nodeDebugger.setup()

    const nodeDebuggerEmitStub = sandbox.stub(nodeDebugger, 'emit')

    nodeDebugger._handleSocketWrite(`{
      "method": "Debugger.paused",
      "params": {
        "callFrames": [
          {
            "callFrameId": "{\\"ordinal\\":0,\\"injectedScriptId\\":1}",
            "location": {
              "columnNumber": 0,
              "lineNumber": 39,
              "scriptId": "67"
            },
            "this": {
              "className": "Object",
              "description": "Object",
              "objectId": "{\\"injectedScriptId\\":1,\\"id\\":3}",
              "type": "object"
            },
            "url": "/Users/stevent/code/personal/vim-padre/padre/padre"
          }
        ],
        "hitBreakpoints": [],
        "reason": "Break on start"
      }
    }`)

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0]).to.deep.equal(['process_position', 40, '/Users/stevent/code/personal/vim-padre/padre/padre'])
  })
})

describe('Test Errors when Spawning and Debugging Node with Inspect', () => {
  let sandbox = null
  let axiosGetStub = null
  let nodeProcessStub = null
  let nodeProcessStubOn = null
  let wsStub = null
  let wsStubReturns = null

  beforeEach(() => {
    sandbox = sinon.createSandbox()

    nodeProcessStub = sandbox.stub(nodeProcess, 'NodeProcess')
    nodeProcessStubOn = sandbox.stub()
    nodeProcessStub.withArgs().returns({
      setup: sandbox.stub(),
      on: nodeProcessStubOn,
    })
    nodeProcessStubOn.withArgs('nodestarted', sinon.match.any).callsArg(1)

    axiosGetStub = sandbox.stub(axios, 'get')

    wsStub = sandbox.mock()
    wsStubReturns = sandbox.createStubInstance(WebSocket)
    wsStub.returns(wsStubReturns)
  })

  afterEach(() => {
    sandbox.restore()
  })

  it('should throw an error when it can\'t request to node inspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'], {wsLib: wsStub})

    const nodeDebuggerEmitStub = sandbox.stub(nodeDebugger, 'emit')
    nodeDebuggerEmitStub.callThrough()

    axiosGetStub.withArgs('http://localhost:9229/json')
        .returns({then: x => undefined, catch: x => x(new Error('Test Error'))})

    console.log(nodeDebuggerEmitStub.callCount)
    await nodeDebugger.setup()

    console.log(nodeDebuggerEmitStub.callCount)
    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0]).to.deep.equal(['padre_error', 'Test Error'])
  })
})
