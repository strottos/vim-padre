'use strict'

const chai = require('chai')
const sinon = require('sinon')

const events = require('events')

const _ = require('lodash')

const java = require.main.require('src/debugger/java/java')
const javaProcess = require.main.require('src/debugger/java/java_process')
const javaSyntax = require.main.require('src/languages/java/syntax')

describe('Test Spawning and Debugging Java', () => {
  beforeEach(() => {
    this.sandbox = sinon.createSandbox()

    this.clock = sinon.useFakeTimers()

    const javaProcessStub = this.sandbox.stub(javaProcess, 'JavaProcess')
    this.javaProcessStubReturns = new events.EventEmitter()
    this.javaProcessStubReturns.run = this.sandbox.stub()
    this.javaProcessStubReturns.request = this.sandbox.stub()
    this.javaProcessStubReturns.getReferenceTypeIDSize = () => 8
    this.javaProcessStubReturns.getMethodIDSize = () => 8
    this.javaProcessStubReturns.getObjectIDSize = () => 8
    javaProcessStub.withArgs().returns(this.javaProcessStubReturns)

    this.javaDebugger = new java.JavaDebugger('java', ['-jar', 'Test.jar'])
  })

  afterEach(() => {
    this.sandbox.restore()
    this.clock.restore()
  })

  it('should successfully setup java', async () => {
    const javaDebuggerEmitStub = this.sandbox.stub(this.javaDebugger, 'emit')
    javaDebuggerEmitStub.callThrough()

    this.javaDebugger.setup()

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal(['started'])
  })

  it('should report errors from JavaProcess up', async () => {
    const javaDebuggerEmitStub = this.sandbox.stub(this.javaDebugger, 'emit')
    javaDebuggerEmitStub.callThrough()

    this.javaDebugger.setup()

    this.javaProcessStubReturns.emit('padre_log', 2, 'Test Error')

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(2)
    chai.expect(javaDebuggerEmitStub.args[1]).to.deep.equal([
      'padre_log', 2, 'Test Error'
    ])
  })

  describe('should allow the debugger to run java', async () => {
    it('should be able to launch a java app and report it', async () => {
      const runPromise = this.javaDebugger.run()

      chai.expect(this.javaProcessStubReturns.run.callCount).to.equal(1)
      chai.expect(this.javaProcessStubReturns.run.args[0]).to.deep.equal([])

      this.javaProcessStubReturns.emit('request', 64, 100, Buffer.from([
        0x02, 0x00, 0x00, 0x00, 0x01, 0x5a, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x01,
      ]))

      const ret = await runPromise

      chai.expect(ret).to.deep.equal({'pid': 0})
      chai.expect(this.javaProcessStubReturns.run.callCount).to.equal(1)
      chai.expect(this.javaProcessStubReturns.run.args[0]).to.deep.equal([])
    })

    it('should report a timeout launching a process', async () => {
      const runPromise = this.javaDebugger.run()

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
      const javaDebuggerEmitStub = this.sandbox.stub(this.javaDebugger, 'emit')
      javaDebuggerEmitStub.callThrough()

      const runPromise = this.javaDebugger.run()

      this.javaProcessStubReturns.emit('request', 64, 100, Buffer.from([
        0x02, 0x00, 0x00, 0x00, 0x01, 0x5a, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x01,
      ]))

      await runPromise

      this.javaProcessStubReturns.emit('request', 64, 100, Buffer.from([
        0x02, 0x00, 0x00, 0x00, 0x01, 0x63, 0x00, 0x00,
        0x00, 0x01,
      ]))

      chai.expect(javaDebuggerEmitStub.callCount).to.equal(2)
      chai.expect(javaDebuggerEmitStub.args[1]).to.deep.equal([
        'process_exit', 0, 0
      ])
    })
  })

  describe('should allow the debugger to set a breakpoint in java', async () => {
    beforeEach(() => {
      this.javaProcessStubReturns.request.withArgs(1, 20).returns({
        'errorCode': 0,
        'data': Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x03]), // Number of classes
          Buffer.from([0x01]), // refTypeTag
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23]), // refTypeId
          Buffer.from([0x00, 0x00, 0x00, 0x20]), // String length...
          Buffer.from('Lcom/padre/test/SimpleJavaClass;'), // ...and string
          Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic signature empty
          Buffer.from([0x00, 0x00, 0x00, 0x03]), // status
          Buffer.from([0x01]), // refTypeTag
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24]), // refTypeId
          Buffer.from([0x00, 0x00, 0x00, 0x1c]), // String length...
          Buffer.from('Lcom/padre/test/ExtraClass1;'), // ...and string
          Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic signature empty
          Buffer.from([0x00, 0x00, 0x00, 0x03]), // status
          Buffer.from([0x01]), // refTypeTag
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24]), // refTypeId
          Buffer.from([0x00, 0x00, 0x00, 0x1c]), // String length...
          Buffer.from('Lcom/padre/test/ExtraClass2;'), // ...and string
          Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic signature empty
          Buffer.from([0x00, 0x00, 0x00, 0x03]), // status
        ])
      })

      this.javaProcessStubReturns.request.withArgs(2, 15).returns({
        'errorCode': 0,
        'data': Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x02]), // 2 methods
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x42]), // first method id
          Buffer.from([0x00, 0x00, 0x00, 0x06]), // String length 6 for `<init>`
          Buffer.from(`<init>`),
          Buffer.from([0x00, 0x00, 0x00, 0x03]), // String length 3 for `()V`
          Buffer.from(`()V`),
          Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic signature empty
          Buffer.from([0x00, 0x00, 0x00, 0x01]), // modbits
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43]), // second method id
          Buffer.from([0x00, 0x00, 0x00, 0x04]), // String length 4 for `main`
          Buffer.from(`main`),
          Buffer.from([0x00, 0x00, 0x00, 0x16]), // String length 22
          Buffer.from(`([Ljava/lang/String;)V`),
          Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic signature empty
          Buffer.from([0x00, 0x00, 0x00, 0x09]), // modbits
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43]), // second method id
          Buffer.from([0x00, 0x00, 0x00, 0x0b]), // String length 11
          Buffer.from(`test_method`),
          Buffer.from([0x00, 0x00, 0x00, 0x04]), // String length 4
          Buffer.from(`(I)I`),
          Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic signature empty
          Buffer.from([0x00, 0x00, 0x00, 0x09]), // modbits
        ])
      })

      this.javaSyntaxGetPositionDataAtLineStub = this.sandbox.stub(javaSyntax, 'getPositionDataAtLine')
    })

    it('should report an error if the filename doesn\'t end in `.java`', async () => {
      const javaDebuggerEmitStub = this.sandbox.stub(this.javaDebugger, 'emit')
      javaDebuggerEmitStub.callThrough()

      await this.javaDebugger.breakpointFileAndLine('Test', 4)

      chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
      chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal([
        'padre_log', 2, 'Bad Filename: Test'
      ])
    })

    it('should set a breakpoint if the class is in the call for classes with generics', async () => {
      const filename = '/home/me/code/src/com/padre/test/data/src/com/padre/test/SimpleJavaClass.java'
      const lineNum = 12

      this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
          [`com.padre.test.SimpleJavaClass`, 'main'])

      const ret = await this.javaDebugger.breakpointFileAndLine(filename, lineNum)

      chai.expect(this.javaProcessStubReturns.request.callCount).to.equal(3)
      chai.expect(this.javaProcessStubReturns.request.args[0]).to.deep.equal([1, 20])
      chai.expect(this.javaProcessStubReturns.request.args[1]).to.deep.equal([
        2, 15, Buffer.from([
          0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23
        ])
      ])
      chai.expect(this.javaProcessStubReturns.request.args[2]).to.deep.equal([
        15, 1, Buffer.concat([
          Buffer.from([0x02, 0x02, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x07]),
          Buffer.from([0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
        ])
      ])

      chai.expect(ret).to.deep.equal({
        'status': 'OK'
      })
    })

    it('should delay setting a breakpoint if the class is not in the call for classes with generics', async () => {
      const filename = '/home/me/code/src/com/padre/test/data/src/com/padre/test/AnotherJavaClass.java'
      const lineNum = 12

      this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
          [`com.padre.test.AnotherJavaClass`, 'main'])

      const ret = await this.javaDebugger.breakpointFileAndLine(filename, lineNum)

      chai.expect(this.javaProcessStubReturns.request.callCount).to.equal(2)
      chai.expect(this.javaProcessStubReturns.request.args[0]).to.deep.equal([1, 20])
      chai.expect(this.javaProcessStubReturns.request.args[1]).to.deep.equal([
        15, 1, Buffer.concat([
          Buffer.from([0x08, 0x02]),
          Buffer.from([0x00, 0x00, 0x00, 0x02]),
          Buffer.from([0x05]),
          Buffer.from([0x00, 0x00, 0x00, 0x1f]),
          Buffer.from('com.padre.test.AnotherJavaClass'),
          Buffer.from([0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x01])
        ])
      ])

      chai.expect(ret).to.deep.equal({
        'status': 'PENDING'
      })
    })

    it('should set a pending breakpoint when the class is prepared', async () => {
      const filename = '/home/me/code/src/com/padre/test/data/src/com/padre/test/AnotherJavaClass.java'
      const lineNum = 12

      this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
          [`com.padre.test.AnotherJavaClass`, 'main'])

      await this.javaDebugger.breakpointFileAndLine(filename, lineNum)

      chai.expect(_.keys(this.javaDebugger._pendingBreakpointMethodForClassess).length).to.equal(1)

      this.javaProcessStubReturns.request.resetHistory()

      // TODO: For some reason can't emit a request here without it returning here sooner than hoped for??
      await this.javaDebugger._handleJavaEventCommand(Buffer.concat([
        Buffer.from([0x02]), // Suspend all
        Buffer.from([0x00, 0x00, 0x00, 0x01]), // One event
        Buffer.from([0x08]), // CLASS_PREPARE Event triggered
        Buffer.from([0x00, 0x00, 0x00, 0x02]), // Request ID
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]), // Thread ID
        Buffer.from([0x01]), // refTypeTag = CLASS
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x25]), // Reference Type ID
        Buffer.from([0x00, 0x00, 0x00, 0x21]), // Signature size
        Buffer.from('Lcom/padre/test/AnotherJavaClass;'),
        Buffer.from([0x00, 0x00, 0x00, 0x03]), // status
      ]))

      chai.expect(_.keys(this.javaDebugger._pendingBreakpointMethodForClassess).length).to.equal(0)

      chai.expect(this.javaProcessStubReturns.request.callCount).to.equal(3)
      chai.expect(this.javaProcessStubReturns.request.args[0]).to.deep.equal([
        2, 15, Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x25])
      ])
      chai.expect(this.javaProcessStubReturns.request.args[1][0]).to.equal(15)
      chai.expect(this.javaProcessStubReturns.request.args[1][1]).to.equal(1)
      chai.expect(this.javaProcessStubReturns.request.args[1][2].readInt8(0)).to.equal(2)
      chai.expect(this.javaProcessStubReturns.request.args[2]).to.deep.equal([1, 9])
    })

    it('should report a timeout setting a breakpoint', async () => {
      const filename = '/home/me/code/src/com/padre/test/data/src/com/padre/test/SimpleJavaClass.java'
      const lineNum = 12

      this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
          [`com.padre.test.SimpleJavaClass`, 'main'])

      const breakpointPromise = this.javaDebugger.breakpointFileAndLine(filename, lineNum)

      this.clock.tick(2010)

      let errorFound = null

      try {
        await breakpointPromise
      } catch (error) {
        errorFound = error
      }

      chai.expect(errorFound).to.be.an('error')
    })
  })

  describe('should allow the debugger to step in in java', async () => {
    beforeEach(() => {
      this.stepInPromise = this.javaDebugger.stepIn()
    })

    it('should step in successfully', async () => {
      const ret = await this.stepInPromise

      chai.expect(this.javaProcessStubReturns.request.callCount).to.equal(2)
      chai.expect(this.javaProcessStubReturns.request.args[0][0]).to.equal(15)
      chai.expect(this.javaProcessStubReturns.request.args[0][1]).to.equal(1)
      chai.expect(this.javaProcessStubReturns.request.args[0][2].readInt8(0)).to.equal(1)
      chai.expect(this.javaProcessStubReturns.request.args[0][2].readInt32BE(15)).to.equal(1)
      chai.expect(this.javaProcessStubReturns.request.args[1]).to.deep.equal([1, 9])

      chai.expect(ret).to.deep.equal({})
    })

    it('should report a timeout continuing', async () => {
      this.clock.tick(2010)

      let errorFound = null

      try {
        await this.stepInPromise
      } catch (error) {
        errorFound = error
      }

      chai.expect(errorFound).to.be.an('error')
    })
  })

  describe('should allow the debugger to step over in java', async () => {
    it('should step over successfully', async () => {
      const ret = await this.javaDebugger.stepOver()

      chai.expect(this.javaProcessStubReturns.request.callCount).to.equal(2)
      chai.expect(this.javaProcessStubReturns.request.args[0][0]).to.equal(15)
      chai.expect(this.javaProcessStubReturns.request.args[0][1]).to.equal(1)
      chai.expect(this.javaProcessStubReturns.request.args[0][2].readInt8(0)).to.equal(1)
      chai.expect(this.javaProcessStubReturns.request.args[0][2].readInt32BE(15)).to.equal(2)
      chai.expect(this.javaProcessStubReturns.request.args[1]).to.deep.equal([1, 9])

      chai.expect(ret).to.deep.equal({})
    })

    it('should report a timeout continuing', async () => {
      const stepOverPromise = this.javaDebugger.stepOver()

      this.clock.tick(2010)

      let errorFound = null

      try {
        await stepOverPromise
      } catch (error) {
        errorFound = error
      }

      chai.expect(errorFound).to.be.an('error')
    })
  })

  describe('should allow the debugger to continue in java', async () => {
    it('should continue successfully', async () => {
      const continuePromise = this.javaDebugger.continue()

      chai.expect(this.javaProcessStubReturns.request.callCount).to.equal(1)
      chai.expect(this.javaProcessStubReturns.request.args[0]).to.deep.equal([1, 9])

      const ret = await continuePromise

      chai.expect(ret).to.deep.equal({})
    })

    it('should report a timeout continuing', async () => {
      const continuePromise = this.javaDebugger.continue()

      this.clock.tick(2010)

      let errorFound = null

      try {
        await continuePromise
      } catch (error) {
        errorFound = error
      }

      chai.expect(errorFound).to.be.an('error')
    })
  })

  describe('should allow the debugger to print variables in java', async () => {
  })

  describe('should allow the debugger to set the current position in java', async () => {
    it('should report the current position when reported by java', async () => {
      const javaDebuggerEmitStub = this.sandbox.stub(this.javaDebugger, 'emit')
      javaDebuggerEmitStub.callThrough()

      chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
      chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal([
        'process_position', '/home/me/code/src/com/padre/test/data/src/com/padre/test/SimpleJavaClass.java', 40
      ])
    })
  })
})
