'use strict'

const chai = require('chai')
const sinon = require('sinon')

const java = require.main.require('src/debugger/java/java')
const javaProcess = require.main.require('src/debugger/java/java_process')

describe('Test Spawning and Debugging Java', () => {
  let sandbox = null
  let javaDebugger = null
  let javaProcessStub = null
  let javaProcessStubReturns = null

  beforeEach(() => {
    sandbox = sinon.createSandbox()

    javaProcessStub = sandbox.stub(javaProcess, 'JavaProcess')
    javaProcessStubReturns = {
      setup: sandbox.stub(),
      on: sandbox.stub(),
    }
    javaProcessStub.withArgs().returns(javaProcessStubReturns)

    javaDebugger = new java.JavaDebugger('java', ['-jar', 'Test.jar'])
  })

  afterEach(() => {
    sandbox.restore()
  })

  it('should successfully setup java', async () => {
    const javaDebuggerEmitStub = sandbox.stub(javaDebugger, 'emit')
    javaDebuggerEmitStub.callThrough()

    javaProcessStubReturns.on.withArgs('started', sinon.match.any).callsArg(1)

    javaDebugger.setup()

    chai.expect(javaProcessStubReturns.setup.callCount).to.equal(1)
    chai.expect(javaProcessStubReturns.setup.args[0]).to.deep.equal([])

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal(['started'])
  })

  it('should report errors from JavaProcess up', async () => {
    const javaDebuggerEmitStub = sandbox.stub(javaDebugger, 'emit')
    javaDebuggerEmitStub.callThrough()

    javaProcessStubReturns.on.withArgs('padre_log').callsArgWith(1, 2, 'Test Error')

    javaDebugger.setup()

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal([
      'padre_log', 2, 'Test Error'
    ])
  })

//  it('should be able to launch a java app and report it', async () => {
//    const javaDebuggerEmitStub = sandbox.stub(javaDebugger, 'emit')
//    javaDebuggerEmitStub.callThrough()
//
//    javaDebugger.setup()
//    const runPromise = javaDebugger.run()
//
//    const ret = await runPromise
//
//    chai.expect(ret).to.deep.equal({'pid': 0})
//  })
})
