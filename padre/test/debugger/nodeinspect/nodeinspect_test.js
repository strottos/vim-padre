'use strict'

const chai = require('chai')
const sinon = require('sinon')

const fs = require('fs')

const nodeinspect = require.main.require('src/debugger/nodeinspect/nodeinspect')
const nodeProcess = require.main.require('src/debugger/nodeinspect/node_process')
const nodeWS = require.main.require('src/debugger/nodeinspect/node_ws')

describe('Test Spawning and Debugging Node with Inspect', () => {
  let sandbox = null
  let nodeWSObj = null

  beforeEach(() => {
    sandbox = sinon.createSandbox()

    const nodeProcessStub = sandbox.stub(nodeProcess, 'NodeProcess')
    const nodeProcessStubOn = sandbox.stub()
    nodeProcessStub.withArgs().returns({
      setup: sandbox.stub(),
      on: nodeProcessStubOn,
    })
    nodeProcessStubOn.withArgs('nodestarted', sinon.match.any).callsArg(1)

    const nodeWSStub = sandbox.stub(nodeWS, 'NodeWS')
    nodeWSObj = {
      setup: sandbox.stub(),
      on: sandbox.stub(),
      sendToDebugger: sandbox.stub()
    }
    nodeWSStub.withArgs(sinon.match.any).returns(nodeWSObj)
  })

  afterEach(() => {
    sandbox.restore()
  })

  it('should successfully setup node inspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const nodeDebuggerEmitStub = sandbox.stub(nodeDebugger, 'emit')
    nodeDebuggerEmitStub.callThrough()

    nodeWSObj.on.withArgs('open', sinon.match.any).callsArg(1)

    await nodeDebugger.setup()

    chai.expect(nodeWSObj.setup.callCount).to.equal(1)
    chai.expect(nodeWSObj.setup.args[0]).to.deep.equal([])

    chai.expect(nodeWSObj.sendToDebugger.callCount).to.equal(3)
    chai.expect(nodeWSObj.sendToDebugger.args[0]).to.deep.equal([{'method': 'Runtime.enable'}])
    chai.expect(nodeWSObj.sendToDebugger.args[1]).to.deep.equal([{'method': 'Debugger.enable'}])
    chai.expect(nodeWSObj.sendToDebugger.args[2]).to.deep.equal([{'method': 'Runtime.runIfWaitingForDebugger'}])

    nodeDebugger._handleDataWrite({
      'method': 'Runtime.executionContextCreated',
      'params': {
        'context': {
          'id': 1,
          'origin': '',
          'name': 'node[12345]',
          'auxData': {
            'isDefault': true
          }
        }
      }
    })
    nodeDebugger._handleDataWrite({
      'method': 'Runtime.enable',
      'result': {}
    })
    nodeDebugger._handleDataWrite({
      'method': 'Debugger.enable',
      'result': {
        'debuggerId': '(ABCD1234ABCD1234ABCD1234ABCD1234)'
      }
    })
    nodeDebugger._handleDataWrite({
      'method': 'Runtime.runIfWaitingForDebugger',
      'result': {}
    })

    chai.expect(nodeDebugger._properties['Runtime.enable']).to.be.true
    chai.expect(nodeDebugger._properties['Debugger.enable']).to.be.true
    chai.expect(nodeDebugger._properties['Debugger.id']).to.equal('(ABCD1234ABCD1234ABCD1234ABCD1234)')
    chai.expect(nodeDebugger._properties['Runtime.runIfWaitingForDebugger']).to.be.true

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0]).to.deep.equal(['started'])

    // Check that we're not emmitting `started` every time
    await nodeWSObj.sendToDebugger({'method': 'Profiler.enable'})
    nodeDebugger._handleDataWrite({
      'method': 'Profiler.enable',
      'result': {}
    })

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
  })

  it('should record the scripts reported by nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    nodeDebugger._handleDataWrite({
      'method': 'Debugger.scriptParsed',
      'params': {
        'scriptId': '11',
        'url': 'internal/bootstrap/loaders.js',
        'startLine': 0,
        'startColumn': 0,
        'endLine': 304,
        'endColumn': 0,
        'executionContextId': 1,
        'hash': '0aec6b749f65bb445e6145d4816e12e006c7b3dd',
        'executionContextAuxData': {
          'isDefault': true
        },
        'isLiveEdit': false,
        'sourceMapURL': '',
        'hasSourceURL': false,
        'isModule': false,
        'length': 10214
      }
    })
    nodeDebugger._handleDataWrite({
      'method': 'Debugger.scriptParsed',
      'params': {
        'scriptId': '12',
        'url': '/Users/me/padre/padre',
        'startLine': 0,
        'startColumn': 0,
        'endLine': 73,
        'endColumn': 3,
        'executionContextId': 1,
        'hash': '150f6b9d1c0af7770d85d08aa052982f4676e587',
        'executionContextAuxData': {
          'isDefault': true
        },
        'isLiveEdit': false,
        'sourceMapURL': '',
        'hasSourceURL': false,
        'isModule': false,
        'length': 1921
      }
    })

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
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const runPromise = nodeDebugger.run()

    const ret = await runPromise

    chai.expect(ret).to.deep.equal({'pid': 0})
  })

  it('should allow the debugger to set a breakpoint in nodeinspect for an existing script', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])
    nodeWSObj.on.withArgs('open', sinon.match.any).callsArg(1)
    await nodeDebugger.setup()

    nodeWSObj.sendToDebugger.resetHistory()

    nodeDebugger._handleDataWrite({
      'method': 'Debugger.scriptParsed',
      'params': {
        'scriptId': '67',
        'url': `${process.cwd()}/index.js`,
        'startLine': 0,
        'startColumn': 0,
        'endLine': 304,
        'endColumn': 0,
        'executionContextId': 1,
        'hash': '0aec6b749f65bb445e6145d4816e12e006c7b3dd',
        'executionContextAuxData': {
          'isDefault': true
        },
        'isLiveEdit': false,
        'sourceMapURL': '',
        'hasSourceURL': false,
        'isModule': false,
        'length': 10214
      }
    })

    sandbox.stub(fs, 'realpathSync').returns(`${process.cwd()}/index.js`)

    const breakpointPromise = nodeDebugger.breakpointFileAndLine('index.js', 20)

    chai.expect(nodeWSObj.sendToDebugger.callCount).to.equal(1)
    chai.expect(nodeWSObj.sendToDebugger.args[0][0]).to.deep.equal({
      'method': 'Debugger.setBreakpoint',
      'params': {
        'location': {
          'scriptId': '67',
          'lineNumber': 20,
        },
      }
    })

    nodeDebugger._handleDataWrite({
      'method': 'Debugger.setBreakpoint',
      'result': {
        'breakpointId': '4:20:0:67',
        'actualLocation': {
          'scriptId': '67',
          'lineNumber': 31,
          'columnNumber': 3
        }
      }
    })

    const ret = await breakpointPromise

    chai.expect(ret).to.deep.equal({
      'breakpointId': '4:20:0:67',
      'line': 31,
      'file': `${process.cwd()}/index.js`,
    })
  })

  // TODO: May need to add support at the debugger level
  it('should allow the debugger to set a pending breakpoint in nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])
    nodeDebugger.setup()
  })

  it('should allow the debugger to step in in nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])
    nodeWSObj.on.withArgs('open', sinon.match.any).callsArg(1)
    await nodeDebugger.setup()

    nodeWSObj.sendToDebugger.resetHistory()

    const stepInPromise = nodeDebugger.stepIn()

    chai.expect(nodeWSObj.sendToDebugger.callCount).to.equal(1)
    chai.expect(nodeWSObj.sendToDebugger.args[0][0]).to.deep.equal({
      'method': 'Debugger.stepInto',
    })

    nodeDebugger._handleDataWrite({
      'method': 'Debugger.stepInto',
      'result': {}
    })

    const ret = await stepInPromise

    chai.expect(ret).to.deep.equal({})
  })

  it('should allow the debugger to step over in nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])
    nodeWSObj.on.withArgs('open', sinon.match.any).callsArg(1)
    await nodeDebugger.setup()

    nodeWSObj.sendToDebugger.resetHistory()

    const stepOverPromise = nodeDebugger.stepOver()

    chai.expect(nodeWSObj.sendToDebugger.callCount).to.equal(1)
    chai.expect(nodeWSObj.sendToDebugger.args[0][0]).to.deep.equal({
      'method': 'Debugger.stepOver',
    })

    nodeDebugger._handleDataWrite({
      'method': 'Debugger.stepOver',
      'result': {}
    })

    const ret = await stepOverPromise

    chai.expect(ret).to.deep.equal({})
  })

  it('should allow the debugger to continue in nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])
    nodeWSObj.on.withArgs('open', sinon.match.any).callsArg(1)
    await nodeDebugger.setup()

    nodeWSObj.sendToDebugger.resetHistory()

    const continuePromise = nodeDebugger.continue()

    chai.expect(nodeWSObj.sendToDebugger.callCount).to.equal(1)
    chai.expect(nodeWSObj.sendToDebugger.args[0][0]).to.deep.equal({
      'method': 'Debugger.resume',
    })

    nodeDebugger._handleDataWrite({
      'method': 'Debugger.resume',
      'result': {}
    })

    const ret = await continuePromise

    chai.expect(ret).to.deep.equal({})
  })

  it('should allow the debugger to print integers in nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])
    nodeWSObj.on.withArgs('open', sinon.match.any).callsArg(1)
    await nodeDebugger.setup()

    nodeWSObj.sendToDebugger.resetHistory()

    const printVariablePromise = nodeDebugger.printVariable('abc')

    chai.expect(nodeWSObj.sendToDebugger.callCount).to.equal(1)
    chai.expect(nodeWSObj.sendToDebugger.args[0][0]).to.deep.equal({
      'method': 'Debugger.evaluateOnCallFrame',
      'params': {
        'callFrameId': '{"ordinal":0,"injectedScriptId":1}',
        'expression': 'abc',
        'returnByValue': true,
      },
    })

    nodeDebugger._handleDataWrite({
      'method': 'Debugger.evaluateOnCallFrame',
      'request': {
        'params': {
          'callFrameId': '{"ordinal":0,"injectedScriptId":1}',
          'expression': 'abc',
          'returnByValue': true,
        }
      },
      'result': {
        'result': {
          'description': '123',
          'type': 'number',
          'value': 123
        }
      }
    })

    const ret = await printVariablePromise

    chai.expect(ret).to.deep.equal({
      'type': 'number',
      'value': 123,
      'variable': 'abc',
    })
  })

  it('should report the current position when reported by nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])
    nodeWSObj.on.withArgs('open', sinon.match.any).callsArg(1)
    await nodeDebugger.setup()

    const nodeDebuggerEmitStub = sandbox.stub(nodeDebugger, 'emit')

    nodeDebugger._handleDataWrite({
      'method': 'Debugger.paused',
      'params': {
        'callFrames': [
          {
            'callFrameId': '{\\"ordinal\\":0,\\"injectedScriptId\\":1}',
            'location': {
              'columnNumber': 0,
              'lineNumber': 39,
              'scriptId': '67'
            },
            'this': {
              'className': 'Object',
              'description': 'Object',
              'objectId': '{\\"injectedScriptId\\":1,\\"id\\":3}',
              'type': 'object'
            },
            'url': '/Users/stevent/code/personal/vim-padre/padre/padre'
          }
        ],
        'hitBreakpoints': [],
        'reason': 'Break on start'
      }
    })

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0]).to.deep.equal(['process_position', 40, '/Users/stevent/code/personal/vim-padre/padre/padre'])
  })
})

describe('Test Errors when Spawning and Debugging Node with Inspect', () => {
  let sandbox = null
  let nodeWSObj = null
  let nodeProcessStubOn

  beforeEach(() => {
    sandbox = sinon.createSandbox()

    const nodeProcessStub = sandbox.stub(nodeProcess, 'NodeProcess')
    nodeProcessStubOn = sandbox.stub()
    nodeProcessStub.withArgs().returns({
      setup: sandbox.stub(),
      on: nodeProcessStubOn,
    })
    nodeProcessStubOn.withArgs('nodestarted', sinon.match.any).callsArg(1)

    const nodeWSStub = sandbox.stub(nodeWS, 'NodeWS')
    nodeWSObj = {
      setup: sandbox.stub(),
      on: sandbox.stub(),
      sendToDebugger: sandbox.stub()
    }
    nodeWSStub.withArgs(sinon.match.any).returns(nodeWSObj)
  })

  afterEach(() => {
    sandbox.restore()
  })

  it('should report errors reported by NodeWS or NodeProcess', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const nodeDebuggerEmitStub = sandbox.stub(nodeDebugger, 'emit')
    nodeDebuggerEmitStub.callThrough()

    nodeWSObj.on.withArgs('padre_error', sinon.match.any).callsArgWith(1, 'test error')

    await nodeDebugger.setup()

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0]).to.deep.equal(['padre_log', 2, 'test error'])
  })

  it('should report errors reported by NodeWS or NodeProcess', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const nodeDebuggerEmitStub = sandbox.stub(nodeDebugger, 'emit')
    nodeDebuggerEmitStub.callThrough()

    nodeProcessStubOn.withArgs('padre_error', sinon.match.any).callsArgWith(1, 'test error')

    await nodeDebugger.setup()

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0]).to.deep.equal(['padre_error', 2, 'test error'])
  })

  it('should throw an error when it can\'t request to node inspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const nodeDebuggerEmitStub = sandbox.stub(nodeDebugger, 'emit')
    nodeDebuggerEmitStub.callThrough()

    await nodeDebugger.setup()

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0]).to.deep.equal(['padre_error', 2, 'Test Error'])
  })
})
