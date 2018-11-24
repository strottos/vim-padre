'use strict'

const chai = require('chai')
const sinon = require('sinon')

const events = require('events')
const fs = require('fs')

const nodeinspect = require.main.require('src/debugger/nodeinspect/nodeinspect')
const nodeProcess = require.main.require('src/debugger/nodeinspect/process')
const nodeWS = require.main.require('src/debugger/nodeinspect/ws')

describe('Test Spawning and Debugging Node with Inspect', () => {
  beforeEach(() => {
    this.sandbox = sinon.createSandbox()

    this.clock = sinon.useFakeTimers()

    const nodeProcessStub = this.sandbox.stub(nodeProcess, 'NodeProcess')
    this.nodeProcessReturns = new events.EventEmitter()
    this.nodeProcessReturns.run = this.sandbox.stub()
    nodeProcessStub.returns(this.nodeProcessReturns)

    const nodeWSStub = this.sandbox.stub(nodeWS, 'NodeWS')
    this.nodeWSObjReturns = new events.EventEmitter()
    this.nodeWSObjReturns.setup = this.sandbox.stub()
    this.nodeWSObjReturns.sendToDebugger = this.sandbox.stub()
    nodeWSStub.returns(this.nodeWSObjReturns)
  })

  afterEach(() => {
    this.sandbox.restore()
    this.clock.restore()
  })

  it('should successfully start node inspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const nodeDebuggerEmitStub = this.sandbox.stub(nodeDebugger, 'emit')
    nodeDebuggerEmitStub.callThrough()

    await nodeDebugger.setup()

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0]).to.deep.equal(['started'])
  })

  it('should report errors reported by NodeWS', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])
    const nodeDebuggerEmitStub = this.sandbox.stub(nodeDebugger, 'emit')

    await nodeDebugger.setup()

    this.nodeWSObjReturns.emit('inspect_error', 'error', 'stack')

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(3)
    chai.expect(nodeDebuggerEmitStub.args[1]).to.deep.equal(['padre_log', 2, 'error'])
    chai.expect(nodeDebuggerEmitStub.args[2]).to.deep.equal(['padre_log', 5, 'stack'])
  })

  it('should report errors reported or NodeProcess', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])
    const nodeDebuggerEmitStub = this.sandbox.stub(nodeDebugger, 'emit')

    await nodeDebugger.setup()

    this.nodeProcessReturns.emit('inspect_error', 'error', 'stack')

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(3)
    chai.expect(nodeDebuggerEmitStub.args[1]).to.deep.equal(['padre_log', 2, 'error'])
    chai.expect(nodeDebuggerEmitStub.args[2]).to.deep.equal(['padre_log', 5, 'stack'])
  })

  // TODO: Extra describe for sections?
  it('should be able to launch a process successfully', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const runPromise = nodeDebugger.run()

    chai.expect(this.nodeProcessReturns.run.callCount).to.equal(1)

    this.nodeWSObjReturns.emit('open')
    this.nodeProcessReturns.emit('inspectstarted')
    nodeDebugger.emit('nodestarted')

    await runPromise

    chai.expect(this.nodeWSObjReturns.setup.callCount).to.equal(1)
    chai.expect(this.nodeWSObjReturns.setup.args[0]).to.deep.equal([])

    chai.expect(this.nodeWSObjReturns.sendToDebugger.callCount).to.equal(3)
    chai.expect(this.nodeWSObjReturns.sendToDebugger.args[0]).to.deep.equal([{'method': 'Runtime.enable'}])
    chai.expect(this.nodeWSObjReturns.sendToDebugger.args[1]).to.deep.equal([{'method': 'Debugger.enable'}])
    chai.expect(this.nodeWSObjReturns.sendToDebugger.args[2]).to.deep.equal([{'method': 'Runtime.runIfWaitingForDebugger'}])
  })

  it('should be able to report a launched process', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const runPromise = nodeDebugger.run()

    this.nodeWSObjReturns.emit('open')
    this.nodeProcessReturns.emit('inspectstarted')

    nodeDebugger._handleWSDataWrite({
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
    nodeDebugger._handleWSDataWrite({
      'method': 'Runtime.enable',
      'result': {}
    })
    nodeDebugger._handleWSDataWrite({
      'method': 'Debugger.enable',
      'result': {
        'debuggerId': '(ABCD1234ABCD1234ABCD1234ABCD1234)'
      }
    })
    nodeDebugger._handleWSDataWrite({
      'method': 'Runtime.runIfWaitingForDebugger',
      'result': {}
    })

    const ret = await runPromise

    chai.expect(nodeDebugger._properties['Runtime.enable']).to.be.true
    chai.expect(nodeDebugger._properties['Debugger.enable']).to.be.true
    chai.expect(nodeDebugger._properties['Debugger.id']).to.equal('(ABCD1234ABCD1234ABCD1234ABCD1234)')
    chai.expect(nodeDebugger._properties['Runtime.runIfWaitingForDebugger']).to.be.true

    chai.expect(ret).to.deep.equal({'pid': 12345})
  })

  it('should report a timeout launching a process', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const runPromise = nodeDebugger.run()

    this.clock.tick(2010)

    let errorFound = null

    try {
      await runPromise
    } catch (error) {
      errorFound = error
    }

    chai.expect(errorFound).to.be.an('error')
  })

  it('should report if the process exits', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    nodeDebugger._pid = 12345

    const nodeDebuggerEmitStub = this.sandbox.stub(nodeDebugger, 'emit')
    nodeDebuggerEmitStub.callThrough()

    nodeDebugger._handleWSDataWrite({
      'method': 'Runtime.executionContextDestroyed',
      'params': {
        'executionContextId': 1
      }
    })

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0]).to.deep.equal([
      'process_exit', 0, 12345
    ])
  })

  it('should record the scripts reported by nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    nodeDebugger._handleWSDataWrite({
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
    nodeDebugger._handleWSDataWrite({
      'method': 'Debugger.scriptParsed',
      'params': {
        'scriptId': '12',
        'url': 'file:///home/me/padre/padre',
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
        'location': '/home/me/padre/padre'
      },
    ])
  })

  // TODO
  it('should report exceptions', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    nodeDebugger._handleWSDataWrite({
    })
  })

  it('should allow the debugger to set a breakpoint for an existing script', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    nodeDebugger._handleWSDataWrite({
      'method': 'Debugger.scriptParsed',
      'params': {
        'scriptId': '67',
        'url': `file:///home/me/padre/index.js`,
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

    this.sandbox.stub(fs, 'realpathSync').returns(`/home/me/padre/index.js`)

    const breakpointPromise = nodeDebugger.breakpointFileAndLine('index.js', 20)

    chai.expect(this.nodeWSObjReturns.sendToDebugger.callCount).to.equal(1)
    chai.expect(this.nodeWSObjReturns.sendToDebugger.args[0][0]).to.deep.equal({
      'method': 'Debugger.setBreakpoint',
      'params': {
        'location': {
          'scriptId': '67',
          'lineNumber': 19,
        },
      }
    })

    nodeDebugger._handleWSDataWrite({
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
      'status': 'OK'
    })
  })

  it('should allow the debugger to set a pending breakpoint', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    this.sandbox.stub(fs, 'realpathSync').returns(`/home/me/padre/index.js`)

    const ret = await nodeDebugger.breakpointFileAndLine('index.js', 20)

    chai.expect(this.nodeWSObjReturns.sendToDebugger.callCount).to.equal(0)
    chai.expect(ret).to.deep.equal({
      'status': 'PENDING'
    })

    nodeDebugger._handleWSDataWrite({
      'method': 'Debugger.scriptParsed',
      'params': {
        'scriptId': '67',
        'url': `file:///home/me/padre/index.js`,
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

    chai.expect(this.nodeWSObjReturns.sendToDebugger.callCount).to.equal(1)
    chai.expect(this.nodeWSObjReturns.sendToDebugger.args[0][0]).to.deep.equal({
      'method': 'Debugger.setBreakpoint',
      'params': {
        'location': {
          'scriptId': '67',
          'lineNumber': 19,
        },
      }
    })
  })

  it('should report setting a breakpoint', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const nodeDebuggerEmitStub = this.sandbox.stub(nodeDebugger, 'emit')
    nodeDebuggerEmitStub.callThrough()

    this.sandbox.stub(fs, 'realpathSync').returns(`/home/me/padre/index.js`)

    nodeDebugger._handleWSDataWrite({
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

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0]).to.deep.equal([
      'breakpoint_set', '/home/me/padre/index.js', 32
    ])
  })

  it('should report an error setting a breakpoint when no such file exists', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    this.sandbox.stub(fs, 'realpathSync').throws(
        `ENOENT: no such file or directory, open 'index.js'`)

    let errorFound = null

    try {
      await nodeDebugger.breakpointFileAndLine('index.js', 20)
    } catch (error) {
      errorFound = error
    }

    chai.expect(errorFound).to.be.an('error')
  })

  it('should report a timeout when setting a breakpoint', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    this.sandbox.stub(fs, 'realpathSync').returns(`/home/me/padre/index.js`)

    nodeDebugger._handleWSDataWrite({
      'method': 'Debugger.scriptParsed',
      'params': {
        'scriptId': '67',
        'url': `file:///home/me/padre/index.js`,
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

    const breakpointPromise = nodeDebugger.breakpointFileAndLine('index.js', 20)

    this.clock.tick(2010)

    let errorFound = null

    try {
      await breakpointPromise
    } catch (error) {
      errorFound = error
    }

    chai.expect(errorFound).to.be.an('error')
  })

  it('should an error when setting a breakpoint in an existing file but a non-existent line number', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const nodeDebuggerEmitStub = this.sandbox.stub(nodeDebugger, 'emit')
    nodeDebuggerEmitStub.callThrough()

    this.sandbox.stub(fs, 'realpathSync').returns(`/home/me/padre/index.js`)

    nodeDebugger._handleWSDataWrite({
      'method': 'Debugger.scriptParsed',
      'params': {
        'scriptId': '67',
        'url': `file:///home/me/padre/index.js`
      }
    })

    nodeDebugger._handleWSDataWrite({
      'error': {
        'code': -32000,
        'message': 'Could not resolve breakpoint'
      },
      'method': 'Debugger.setBreakpoint',
      'request': {
        'params': {
          'location': {
            'lineNumber': 3999,
            'scriptId': '67'
          }
        }
      }
    })

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0]).to.deep.equal([
      'padre_log', 2, 'Couldn\'t set breakpoint at /home/me/padre/index.js, ' +
          'line 4000: Error -32000, Could not resolve breakpoint'
    ])
  })

  it('should allow the debugger to step in', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const stepInPromise = nodeDebugger.stepIn()

    chai.expect(this.nodeWSObjReturns.sendToDebugger.callCount).to.equal(1)
    chai.expect(this.nodeWSObjReturns.sendToDebugger.args[0][0]).to.deep.equal({
      'method': 'Debugger.stepInto',
    })

    nodeDebugger._handleWSDataWrite({
      'method': 'Debugger.stepInto',
      'result': {}
    })

    const ret = await stepInPromise

    chai.expect(ret).to.deep.equal({})
  })

  it('should report a timeout stepping in', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const stepInPromise = nodeDebugger.stepIn()

    this.clock.tick(2010)

    let errorFound = null

    try {
      await stepInPromise
    } catch (error) {
      errorFound = error
    }

    chai.expect(errorFound).to.be.an('error')
  })

  it('should allow the debugger to step over', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const stepOverPromise = nodeDebugger.stepOver()

    chai.expect(this.nodeWSObjReturns.sendToDebugger.callCount).to.equal(1)
    chai.expect(this.nodeWSObjReturns.sendToDebugger.args[0][0]).to.deep.equal({
      'method': 'Debugger.stepOver',
    })

    nodeDebugger._handleWSDataWrite({
      'method': 'Debugger.stepOver',
      'result': {}
    })

    const ret = await stepOverPromise

    chai.expect(ret).to.deep.equal({})
  })

  it('should report a timeout stepping over', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const stepOverPromise = nodeDebugger.stepOver()

    this.clock.tick(2010)

    let errorFound = null

    try {
      await stepOverPromise
    } catch (error) {
      errorFound = error
    }

    chai.expect(errorFound).to.be.an('error')
  })

  it('should allow the debugger to continue', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const continuePromise = nodeDebugger.continue()

    chai.expect(this.nodeWSObjReturns.sendToDebugger.callCount).to.equal(1)
    chai.expect(this.nodeWSObjReturns.sendToDebugger.args[0][0]).to.deep.equal({
      'method': 'Debugger.resume',
    })

    nodeDebugger._handleWSDataWrite({
      'method': 'Debugger.resume',
      'result': {}
    })

    const ret = await continuePromise

    chai.expect(ret).to.deep.equal({})
  })

  it('should report a timeout continuing', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const continuePromise = nodeDebugger.continue()

    this.clock.tick(2010)

    let errorFound = null

    try {
      await continuePromise
    } catch (error) {
      errorFound = error
    }

    chai.expect(errorFound).to.be.an('error')
  })

  it('should allow the debugger to print numbers', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const printVariablePromise = nodeDebugger.printVariable('abc')

    chai.expect(this.nodeWSObjReturns.sendToDebugger.callCount).to.equal(1)
    chai.expect(this.nodeWSObjReturns.sendToDebugger.args[0][0]).to.deep.equal({
      'method': 'Debugger.evaluateOnCallFrame',
      'params': {
        'callFrameId': '{"ordinal":0,"injectedScriptId":1}',
        'expression': 'abc',
        'returnByValue': true,
      },
    })

    nodeDebugger._handleWSDataWrite({
      'result': {
        'result': {
          'type': 'number',
          'value': 123,
          'description': '1'
        }
      },
      'request': {
        'params': {
          'callFrameId': '{"ordinal":0,"injectedScriptId":1}',
          'expression': 'abc',
          'returnByValue': true,
        }
      },
      'method': 'Debugger.evaluateOnCallFrame'
    })

    const ret = await printVariablePromise

    chai.expect(ret).to.deep.equal({
      'type': 'number',
      'value': 123,
      'variable': 'abc',
    })
  })

  it('should allow the debugger to print strings', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const printVariablePromise = nodeDebugger.printVariable('abc')

    nodeDebugger._handleWSDataWrite({
      'result': {
        'result': {
          'type': 'string',
          'value': 'Test String',
          'description': '1'
        }
      },
      'request': {
        'params': {
          'callFrameId': '{"ordinal":0,"injectedScriptId":1}',
          'expression': 'abc',
          'returnByValue': true,
        }
      },
      'method': 'Debugger.evaluateOnCallFrame'
    })

    const ret = await printVariablePromise

    chai.expect(ret).to.deep.equal({
      'type': 'string',
      'value': 'Test String',
      'variable': 'abc',
    })
  })

  it('should allow the debugger to print undefined types', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const printVariablePromise = nodeDebugger.printVariable('abc')

    nodeDebugger._handleWSDataWrite({
      'result': {
        'result': {
          'type': 'undefined'
        }
      },
      'request': {
        'params': {
          'callFrameId': '{"ordinal":0,"injectedScriptId":1}',
          'expression': 'abc',
          'returnByValue': true,
        }
      },
      'method': 'Debugger.evaluateOnCallFrame'
    })

    const ret = await printVariablePromise

    chai.expect(ret).to.deep.equal({
      'type': 'null',
      'value': 'undefined',
      'variable': 'abc',
    })
  })

  it('should allow the debugger to print null types', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const printVariablePromise = nodeDebugger.printVariable('abc')

    nodeDebugger._handleWSDataWrite({
      'result': {
        'result': {
          'type': 'object',
          'subtype': 'null',
          'value': null
        }
      },
      'request': {
        'params': {
          'callFrameId': '{"ordinal":0,"injectedScriptId":1}',
          'expression': 'abc',
          'returnByValue': true,
        }
      },
      'method': 'Debugger.evaluateOnCallFrame'
    })

    const ret = await printVariablePromise

    chai.expect(ret).to.deep.equal({
      'type': 'null',
      'value': 'null',
      'variable': 'abc',
    })
  })

  it('should allow the debugger to print boolean types', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const printVariablePromise = nodeDebugger.printVariable('abc')

    nodeDebugger._handleWSDataWrite({
      'result': {
        'result': {
          'type': 'boolean',
          'value': true
        }
      },
      'request': {
        'params': {
          'callFrameId': '{"ordinal":0,"injectedScriptId":1}',
          'expression': 'abc',
          'returnByValue': true,
        }
      },
      'method': 'Debugger.evaluateOnCallFrame'
    })

    const ret = await printVariablePromise

    chai.expect(ret).to.deep.equal({
      'type': 'boolean',
      'value': true,
      'variable': 'abc',
    })
  })

  it('should allow the debugger to print object types', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const printVariablePromise = nodeDebugger.printVariable('abc')

    nodeDebugger._handleWSDataWrite({
      'result': {
        'result': {
          'type': 'object',
          'value': {
            'testList': [1, 2, 3, 4]
          }
        }
      },
      'request': {
        'params': {
          'callFrameId': '{"ordinal":0,"injectedScriptId":1}',
          'expression': 'abc',
          'returnByValue': true,
        }
      },
      'method': 'Debugger.evaluateOnCallFrame'
    })

    const ret = await printVariablePromise

    chai.expect(ret).to.deep.equal({
      'type': 'JSON',
      'value': {
        'testList': [1, 2, 3, 4]
      },
      'variable': 'abc',
    })
  })

  // TODO
  it('should report an error when we get a referenceError on printing variables', async () => {
  })

  it('should report a timeout printing a variable', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const printVariablePromise = nodeDebugger.printVariable('abc')

    this.clock.tick(2010)

    let errorFound = null

    try {
      await printVariablePromise
    } catch (error) {
      errorFound = error
    }

    chai.expect(errorFound).to.be.an('error')
  })

  it('should report the current position when reported by nodeinspect', async () => {
    const nodeDebugger = new nodeinspect.NodeInspect('./test', ['--arg1'])

    const nodeDebuggerEmitStub = this.sandbox.stub(nodeDebugger, 'emit')

    nodeDebugger._handleWSDataWrite({
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
            'url': 'file:///home/me/code/vim-padre/padre/padre'
          }
        ],
        'hitBreakpoints': [],
        'reason': 'Break on start'
      }
    })

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0]).to.deep.equal(['process_position', '/home/me/code/vim-padre/padre/padre', 40])
  })
})
