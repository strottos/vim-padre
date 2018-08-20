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
      'setup': sandbox.stub(),
      'request': sandbox.stub(),
      'on': sandbox.stub(),
      'getReferenceTypeIDSize': sandbox.stub()
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

  // it('should be able to launch a java app and report it', async () => {
  //   const javaDebuggerEmitStub = sandbox.stub(javaDebugger, 'emit')
  //   javaDebuggerEmitStub.callThrough()
  //
  //   javaDebugger.setup()
  //   const runPromise = javaDebugger.run()
  //
  //   const ret = await runPromise
  //
  //   chai.expect(ret).to.deep.equal({'pid': 0})
  // })

  describe('should allow the debugger to set a breakpoint in java', async () => {
    beforeEach(() => {
      javaProcessStubReturns.getReferenceTypeIDSize.withArgs().returns(8)
    })

    it('should check the current classes with generics for an existing class', async () => {
      javaProcessStubReturns.request.withArgs(1, 20).returns({
        'errorCode': 0,
        'data': Buffer.from([
          0x00, 0x00, 0x00, 0x00
        ]),
      })

      const ret = await javaDebugger.breakpointFileAndLine('Test.java', 4)

      chai.expect(javaProcessStubReturns.request.callCount).to.equal(1)
      chai.expect(javaProcessStubReturns.request.args[0]).to.deep.equal([1, 20])
    })

    it('should report an error if the filename doesn\'t end in `.java`', async () => {
      const javaDebuggerEmitStub = sandbox.stub(javaDebugger, 'emit')
      javaDebuggerEmitStub.callThrough()

      await javaDebugger.breakpointFileAndLine('Test', 4)

      chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
      chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal([
        'padre_log', 2, 'Bad Filename: Test'
      ])
    })

    it('should set a breakpoint if the class is in the call for classes with generics', async () => {
      const classSignature = 'Lcom/padre/test/SimpleJavaClass;'

      javaProcessStubReturns.request.withArgs(1, 20).returns({
        'errorCode': 0,
        'data': Buffer.concat([
          Buffer.from([
            0x00, 0x00, 0x00, 0x01, 0x01,
          ]),
          Buffer.from([
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
          ]),
          Buffer.from([
            0x00, 0x00, 0x00, classSignature.length,
          ]),
          Buffer.from(classSignature),
          Buffer.from([
            0x00, 0x00, 0x00, 0x00,
          ]),
          Buffer.from([
            0x00, 0x00, 0x00, 0x03,
          ]),
        ])
      })

      const ret = await javaDebugger.breakpointFileAndLine('Test.java', 4)
    })

    // TODO
    // it('should report an error if we get an error from ', async () => {
    // })
  })
})
