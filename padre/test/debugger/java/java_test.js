'use strict'

const chai = require('chai')
const sinon = require('sinon')

const java = require.main.require('src/debugger/java/java')
const javaProcess = require.main.require('src/debugger/java/java_process')
const javaSyntax = require.main.require('src/languages/java/syntax')

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
      'getReferenceTypeIDSize': sandbox.stub(),
      'getMethodIDSize': sandbox.stub(),
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
    let javaSyntaxGetPositionDataAtLineStub = null

    beforeEach(() => {
      javaProcessStubReturns.getReferenceTypeIDSize.withArgs().returns(8)
      javaProcessStubReturns.getMethodIDSize.withArgs().returns(8)

      const classSignature = 'Lcom/padre/test/SimpleJavaClass;'

      javaProcessStubReturns.request.withArgs(1, 20).returns({
        'errorCode': 0,
        'data': Buffer.concat([
          Buffer.from([ // Number
            0x00, 0x00, 0x00, 0x01,
          ]),
          Buffer.from([ // refTypeTag
            0x01,
          ]),
          Buffer.from([ // refTypeId
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23,
          ]),
          Buffer.from([ // String length...
            0x00, 0x00, 0x00, classSignature.length,
          ]),
          Buffer.from(classSignature), // ...and string
          Buffer.from([ // Generic signature empty
            0x00, 0x00, 0x00, 0x00,
          ]),
          Buffer.from([ // status
            0x00, 0x00, 0x00, 0x03,
          ]),
        ])
      })

      javaProcessStubReturns.request.withArgs(2, 15).returns({
        'errorCode': 0,
        'data': Buffer.concat([
          Buffer.from([ // 2 methods
            0x00, 0x00, 0x00, 0x02,
          ]),
          Buffer.from([ // first method id
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x42,
          ]),
          Buffer.from([ // String length 6 for `<init>`
            0x00, 0x00, 0x00, 0x06,
          ]),
          Buffer.from(`<init>`),
          Buffer.from([ // String length 3 for `()V`
            0x00, 0x00, 0x00, 0x03,
          ]),
          Buffer.from(`()V`),
          Buffer.from([ // Generic signature empty
            0x00, 0x00, 0x00, 0x00,
          ]),
          Buffer.from([ // modbits
            0x00, 0x00, 0x00, 0x01,
          ]),
          Buffer.from([ // second method id
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43,
          ]),
          Buffer.from([ // String length 4 for `main`
            0x00, 0x00, 0x00, 0x04,
          ]),
          Buffer.from(`main`),
          Buffer.from([ // String length 22
            0x00, 0x00, 0x00, 0x16,
          ]),
          Buffer.from(`([Ljava/lang/String;)V`),
          Buffer.from([ // Generic signature empty
            0x00, 0x00, 0x00, 0x00,
          ]),
          Buffer.from([ // modbits
            0x00, 0x00, 0x00, 0x09,
          ]),
        ])
      })

      javaSyntaxGetPositionDataAtLineStub = sandbox.stub(javaSyntax, 'getPositionDataAtLine')
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
      const filename = '/home/me/code/src/com/padre/test/SimpleJavaClass.java'
      const lineNum = 12
      javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
          [`com.padre.test.SimpleJavaClass`, 'main'])

      const ret = await javaDebugger.breakpointFileAndLine(filename, lineNum)

      chai.expect(javaProcessStubReturns.request.callCount).to.equal(3)
      chai.expect(javaProcessStubReturns.request.args[0]).to.deep.equal([1, 20])
      chai.expect(javaProcessStubReturns.request.args[1]).to.deep.equal([
        2, 15, Buffer.from([
          0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23
        ])
      ])
      chai.expect(javaProcessStubReturns.request.args[2]).to.deep.equal([
        15, 1, Buffer.concat([
          Buffer.from([0x02, 0x02, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x07]),
          Buffer.from([0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
        ])
      ])
    })

    it('should delay setting a breakpoint if the class is not in the call for classes with generics', async () => {
      const filename = '/home/me/code/src/com/padre/test/SimpleJavaClassNotExists.java'
      const lineNum = 12
      javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
          [`com.padre.test.SimpleJavaClassNotExists`, `main`])

      const ret = await javaDebugger.breakpointFileAndLine(filename, lineNum)

      chai.expect(javaProcessStubReturns.request.callCount).to.equal(1)
      chai.expect(javaProcessStubReturns.request.args[0]).to.deep.equal([1, 20])
    })

    // TODO
    // it('should report an error if we get an error from ', async () => {
    // })
  })
})
