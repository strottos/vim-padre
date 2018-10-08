'use strict'

const chai = require('chai')
const sinon = require('sinon')

const events = require('events')
const path = require('path')

const _ = require('lodash')
const walk = require('fs-walk')

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

    const walkFilesStub = this.sandbox.stub(walk, 'filesSync')
    walkFilesStub.callsArgWith(1, `test/data/src/com/padre/test/`, `SimpleJavaClass.java`)

    const pathStub = this.sandbox.stub(path, 'resolve')
    pathStub.withArgs(`test/data/src/com/padre/test//SimpleJavaClass.java`).returns(
        `test/data/src/com/padre/test/SimpleJavaClass.java`)
  })

  afterEach(() => {
    this.sandbox.restore()
    this.clock.restore()
  })

  it(`should successfully setup java`, async () => {
    const javaDebuggerEmitStub = this.sandbox.stub(this.javaDebugger, 'emit')
    javaDebuggerEmitStub.callThrough()

    this.javaDebugger.setup()

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal(['started'])
  })

  it(`should report errors from JavaProcess up`, async () => {
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
    it(`should be able to launch a java app and report it`, async () => {
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

    it(`should report a timeout launching a process`, async () => {
      const runPromise = this.javaDebugger.run()

      this.clock.tick(60010)

      let errorFound = null

      try {
        await runPromise
      } catch (error) {
        errorFound = error
      }

      chai.expect(errorFound).to.be.an('error')
    })

    it(`should report if the process exits`, async () => {
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
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x25]), // refTypeId
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

    it(`should report an error if the filename doesn't end in '.java'`, async () => {
      const javaDebuggerEmitStub = this.sandbox.stub(this.javaDebugger, 'emit')
      javaDebuggerEmitStub.callThrough()

      await this.javaDebugger.breakpointFileAndLine('Test', 4)

      chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
      chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal([
        'padre_log', 2, 'Bad Filename: Test'
      ])
    })

    it(`should set a breakpoint if the class is in the call for classes with generics`, async () => {
      const filename = '/home/me/code/padre/test/data/src/com/padre/test/SimpleJavaClass.java'
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

    it(`should delay setting a breakpoint if the class is not in the call for classes with generics`, async () => {
      const filename = '/home/me/code/padre/test/data/src/com/padre/test/AnotherJavaClass.java'
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

    it(`should set a pending breakpoint when the class is prepared`, async () => {
      const filename = '/home/me/code/padre/test/data/src/com/padre/test/AnotherJavaClass.java'
      const lineNum = 12

      this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
          [`com.padre.test.AnotherJavaClass`, 'main'])

      await this.javaDebugger.breakpointFileAndLine(filename, lineNum)

      chai.expect(_.keys(this.javaDebugger._pendingBreakpointMethodForClasses).length).to.equal(1)

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

      chai.expect(_.keys(this.javaDebugger._pendingBreakpointMethodForClasses).length).to.equal(0)

      chai.expect(this.javaProcessStubReturns.request.callCount).to.equal(3)
      chai.expect(this.javaProcessStubReturns.request.args[0]).to.deep.equal([
        2, 15, Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x25])
      ])
      chai.expect(this.javaProcessStubReturns.request.args[1][0]).to.equal(15)
      chai.expect(this.javaProcessStubReturns.request.args[1][1]).to.equal(1)
      chai.expect(this.javaProcessStubReturns.request.args[1][2].readInt8(0)).to.equal(2)
      chai.expect(this.javaProcessStubReturns.request.args[2]).to.deep.equal([1, 9])
    })

    it(`should report a timeout setting a breakpoint`, async () => {
      const filename = '/home/me/code/padre/test/data/src/com/padre/test/SimpleJavaClass.java'
      const lineNum = 12

      this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
          [`com.padre.test.SimpleJavaClass`, 'main'])

      const breakpointPromise = this.javaDebugger.breakpointFileAndLine(filename, lineNum)

      this.clock.tick(10010)

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
    it(`should step in successfully`, async () => {
      const threadID = this.javaDebugger._currentThreadID = Buffer.from([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x21
      ])

      const ret = await this.javaDebugger.stepIn()

      console.log(this.javaDebugger._currentThreadID)

      chai.expect(this.javaProcessStubReturns.request.callCount).to.equal(2)
      chai.expect(this.javaProcessStubReturns.request.args[0][0]).to.equal(15)
      chai.expect(this.javaProcessStubReturns.request.args[0][1]).to.equal(1)
      chai.expect(this.javaProcessStubReturns.request.args[0][2].readInt8(0)).to.equal(1)
      chai.expect(this.javaProcessStubReturns.request.args[0][2].slice(7, 15)).to.deep.equal(threadID)
      chai.expect(this.javaProcessStubReturns.request.args[0][2].readInt32BE(15)).to.equal(1)
      chai.expect(this.javaProcessStubReturns.request.args[0][2].readInt32BE(19)).to.equal(0)
      chai.expect(this.javaProcessStubReturns.request.args[1]).to.deep.equal([1, 9])

      chai.expect(ret).to.deep.equal({})
    })

    it(`should report a timeout continuing`, async () => {
      const stepInPromise = this.javaDebugger.stepIn()

      this.clock.tick(10010)

      let errorFound = null

      try {
        await stepInPromise
      } catch (error) {
        errorFound = error
      }

      chai.expect(errorFound).to.be.an('error')
    })
  })

  describe('should allow the debugger to step over in java', async () => {
    it(`should step over successfully`, async () => {
      const threadID = this.javaDebugger._currentThreadID = Buffer.from([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x21
      ])

      const ret = await this.javaDebugger.stepOver()

      console.log(this.javaDebugger._currentThreadID)

      chai.expect(this.javaProcessStubReturns.request.callCount).to.equal(2)
      chai.expect(this.javaProcessStubReturns.request.args[0][0]).to.equal(15)
      chai.expect(this.javaProcessStubReturns.request.args[0][1]).to.equal(1)
      chai.expect(this.javaProcessStubReturns.request.args[0][2].readInt8(0)).to.equal(1)
      chai.expect(this.javaProcessStubReturns.request.args[0][2].slice(7, 15)).to.deep.equal(threadID)
      chai.expect(this.javaProcessStubReturns.request.args[0][2].readInt32BE(15)).to.equal(1)
      chai.expect(this.javaProcessStubReturns.request.args[0][2].readInt32BE(19)).to.equal(1)
      chai.expect(this.javaProcessStubReturns.request.args[1]).to.deep.equal([1, 9])

      chai.expect(ret).to.deep.equal({})
    })

    it(`should report a timeout continuing`, async () => {
      const stepOverPromise = this.javaDebugger.stepOver()

      this.clock.tick(10010)

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
    it(`should continue successfully`, async () => {
      const continuePromise = this.javaDebugger.continue()

      chai.expect(this.javaProcessStubReturns.request.callCount).to.equal(1)
      chai.expect(this.javaProcessStubReturns.request.args[0]).to.deep.equal([1, 9])

      const ret = await continuePromise

      chai.expect(ret).to.deep.equal({})
    })

    it(`should report a timeout continuing`, async () => {
      const continuePromise = this.javaDebugger.continue()

      this.clock.tick(10010)

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
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x25]), // refTypeId
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

      this.javaProcessStubReturns.request.withArgs(11, 6, Buffer.concat([
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
        Buffer.from([0x00, 0x00, 0x00, 0x00]),
        Buffer.from([0x00, 0x00, 0x00, 0x01]),
      ])).returns({
        'errorCode': 0,
        'data': Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x01]), // One Frame
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00]), // frameID
          Buffer.from([0x01]), // refTypeTag
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23]), // classID
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43]), // methodID
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // codeIndex
        ])
      })

      this.javaSyntaxGetPositionDataAtLineStub = this.sandbox.stub(javaSyntax, 'getPositionDataAtLine')
    })

    describe('local primitive variables', async () => {
      const setVariableReturnValues = (signature, value) => {
        let signatureSize = Buffer.alloc(4)
        signatureSize.writeInt32BE(signature.length)

        this.javaProcessStubReturns.request.withArgs(6, 5, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43]),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x02]), // argCnt???
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 slot
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // code index
            Buffer.from([0x00, 0x00, 0x00, 0x03]), // Variable name
            Buffer.from(`abc`),
            signatureSize,
            Buffer.from(signature),
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic Signature
            Buffer.from([0x00, 0x00, 0x00, 0x07]), // Length
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // Slot
          ])
        })

        let type = signature
        let valueBuffer
        switch (type) {
        case 'I':
          valueBuffer = Buffer.alloc(4)
          valueBuffer.writeInt32BE(value)
          break
        case 'C':
        case 'S':
          valueBuffer = Buffer.alloc(2)
          valueBuffer.writeInt16BE(value)
          break
        case 'F':
          valueBuffer = Buffer.alloc(4)
          valueBuffer.writeFloatBE(value)
          break
        case 'D':
          valueBuffer = Buffer.alloc(8)
          valueBuffer.writeDoubleBE(value)
          break
        case 'J':
          valueBuffer = Buffer.from(value)
          break
        case 'Z':
        case 'B':
          valueBuffer = Buffer.alloc(1)
          valueBuffer.writeInt8(value)
          break
        case 'Ljava/lang/String;':
          type = 's'
          valueBuffer = value
          break
        }

        this.javaProcessStubReturns.request.withArgs(16, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
          Buffer.from(type),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 value
            Buffer.from(type), // Type
            valueBuffer, // Value
          ])
        })

        if (signature === 'Ljava/lang/String;') {
          this.javaProcessStubReturns.request.withArgs(10, 1, Buffer.concat([
            value
          ])).returns({
            'errorCode': 0,
            'data': Buffer.concat([
              Buffer.from([0x00, 0x00, 0x00, 0x0f]),
              Buffer.from(`testing strings`),
            ])
          })
        }
      }

      it(`should print 'int's`, async () => {
        const filename = 'test/data/src/com/padre/test/SimpleJavaClass.java'
        const lineNum = 123

        setVariableReturnValues('I', 1234567)

        this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
            [`com.padre.test.SimpleJavaClass`, 'main'])

        const ret = await this.javaDebugger.printVariable('abc', filename, lineNum)

        chai.expect(this.javaProcessStubReturns.request.args[3]).to.deep.equal([11, 6, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
        ])])

        chai.expect(ret).to.deep.equal({
          'type': 'number',
          'value': 1234567,
          'variable': 'abc',
        })
      })

      it(`should print local 'bytes's`, async () => {
        const filename = 'test/data/src/com/padre/test/SimpleJavaClass.java'
        const lineNum = 123

        setVariableReturnValues('B', 123)

        this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
            [`com.padre.test.SimpleJavaClass`, 'main'])

        const ret = await this.javaDebugger.printVariable('abc', filename, lineNum)

        chai.expect(this.javaProcessStubReturns.request.args[3]).to.deep.equal([11, 6, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
        ])])

        chai.expect(ret).to.deep.equal({
          'type': 'number',
          'value': 123,
          'variable': 'abc',
        })
      })

      // TODO: Unicode values showing for chars, let's just do a number for now
      it(`should print local 'char's`, async () => {
        const filename = 'test/data/src/com/padre/test/SimpleJavaClass.java'
        const lineNum = 123

        setVariableReturnValues('C', 12345)

        this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
            [`com.padre.test.SimpleJavaClass`, 'main'])

        const ret = await this.javaDebugger.printVariable('abc', filename, lineNum)

        chai.expect(this.javaProcessStubReturns.request.args[3]).to.deep.equal([11, 6, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
        ])])

        chai.expect(ret).to.deep.equal({
          'type': 'number',
          'value': 12345,
          'variable': 'abc',
        })
      })

      it(`should print local 'float's`, async () => {
        const filename = 'test/data/src/com/padre/test/SimpleJavaClass.java'
        const lineNum = 123

        let floatBuffer = Buffer.alloc(4)
        floatBuffer.writeFloatBE(123.45)
        const floatValue = floatBuffer.readFloatBE()

        setVariableReturnValues('F', floatValue)

        this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
            [`com.padre.test.SimpleJavaClass`, 'main'])

        const ret = await this.javaDebugger.printVariable('abc', filename, lineNum)

        chai.expect(this.javaProcessStubReturns.request.args[3]).to.deep.equal([11, 6, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
        ])])

        chai.expect(ret).to.deep.equal({
          'type': 'number',
          'value': floatValue,
          'variable': 'abc',
        })
      })

      it(`should print local 'double's`, async () => {
        const filename = 'test/data/src/com/padre/test/SimpleJavaClass.java'
        const lineNum = 123

        let doubleBuffer = Buffer.alloc(8)
        doubleBuffer.writeDoubleBE(958139585931.5839)
        const doubleValue = doubleBuffer.readDoubleBE()

        setVariableReturnValues('D', doubleValue)

        this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
            [`com.padre.test.SimpleJavaClass`, 'main'])

        const ret = await this.javaDebugger.printVariable('abc', filename, lineNum)

        chai.expect(this.javaProcessStubReturns.request.args[3]).to.deep.equal([11, 6, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
        ])])

        chai.expect(ret).to.deep.equal({
          'type': 'number',
          'value': doubleValue,
          'variable': 'abc',
        })
      })

      it(`should print local 'long's`, async () => {
        const filename = 'test/data/src/com/padre/test/SimpleJavaClass.java'
        const lineNum = 123

        const longValue = Buffer.from([0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07])

        setVariableReturnValues('J', longValue)

        this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
            [`com.padre.test.SimpleJavaClass`, 'main'])

        const ret = await this.javaDebugger.printVariable('abc', filename, lineNum)

        chai.expect(this.javaProcessStubReturns.request.args[3]).to.deep.equal([11, 6, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
        ])])

        chai.expect(ret).to.deep.equal({
          'type': 'string',
          'value': '283686952306183',
          'variable': 'abc',
        })
      })

      it(`should print local 'short's`, async () => {
        const filename = 'test/data/src/com/padre/test/SimpleJavaClass.java'
        const lineNum = 123

        setVariableReturnValues('S', 1234)

        this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
            [`com.padre.test.SimpleJavaClass`, 'main'])

        const ret = await this.javaDebugger.printVariable('abc', filename, lineNum)

        chai.expect(this.javaProcessStubReturns.request.args[3]).to.deep.equal([11, 6, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
        ])])

        chai.expect(ret).to.deep.equal({
          'type': 'number',
          'value': 1234,
          'variable': 'abc',
        })
      })

      it(`should print local 'boolean's`, async () => {
        const filename = 'test/data/src/com/padre/test/SimpleJavaClass.java'
        const lineNum = 123

        setVariableReturnValues('Z', 1)

        this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
            [`com.padre.test.SimpleJavaClass`, 'main'])

        const ret = await this.javaDebugger.printVariable('abc', filename, lineNum)

        chai.expect(ret).to.deep.equal({
          'type': 'boolean',
          'value': true,
          'variable': 'abc',
        })
      })

      it(`should print local strings`, async () => {
        const filename = 'test/data/src/com/padre/test/SimpleJavaClass.java'
        const lineNum = 123

        setVariableReturnValues('Ljava/lang/String;',
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x68]))

        this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
            [`com.padre.test.SimpleJavaClass`, 'main'])

        const ret = await this.javaDebugger.printVariable('abc', filename, lineNum)

        chai.expect(ret).to.deep.equal({
          'type': 'string',
          'value': 'testing strings',
          'variable': 'abc',
        })
      })
    })

    describe('class fields', async () => {
      beforeEach(() => {
        this.javaProcessStubReturns.request.withArgs(6, 5, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43]),
        ])).returns({
          'errorCode': 101
        })

        this.javaProcessStubReturns.request.withArgs(16, 3, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00])
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x01]), // Class
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x72]),
          ])
        })
      })

      // TODO: static fields

      it(`should print a field in the current object`, async () => {
        const filename = 'test/data/src/com/padre/test/SimpleJavaClass.java'
        const lineNum = 123

        this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
            [`com.padre.test.SimpleJavaClass`, 'main'])

        this.javaProcessStubReturns.request.withArgs(2, 14, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23])
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // One field
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x84]), // Field ID
            Buffer.from([0x00, 0x00, 0x00, 0x03]), // Name length
            Buffer.from(`abc`), // Name
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // Signature length
            Buffer.from(`I`), // Signature
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic signature length
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // modbits
          ])
        })

        this.javaProcessStubReturns.request.withArgs(9, 2, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x72]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x84]),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // One value
            Buffer.from(`I`), // Integer
            Buffer.from([0x00, 0x00, 0x00, 0x18]), // Value
          ])
        })

        const ret = await this.javaDebugger.printVariable('abc', filename, lineNum)

        chai.expect(ret).to.deep.equal({
          'type': 'number',
          'value': 0x18,
          'variable': 'abc',
        })
      })

      // TODO: inherited fields

      // TODO: Interfaces
    })

    describe('objects', async () => {
      it(`should print an object`, async () => {
        const filename = 'test/data/src/com/padre/test/SimpleJavaClass.java'
        const lineNum = 123

        this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
            [`com.padre.test.SimpleJavaClass`, 'main'])

        this.javaProcessStubReturns.request.withArgs(6, 5, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43]),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x02]), // argCnt???
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 slot
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // code index
            Buffer.from([0x00, 0x00, 0x00, 0x03]), // Variable name
            Buffer.from(`abc`),
            Buffer.from([0x00, 0x00, 0x00, 0x20]), // Signature
            Buffer.from(`Lcom/padre/test/SimpleJavaClass;`),
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic Signature
            Buffer.from([0x00, 0x00, 0x00, 0x07]), // Length
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // Slot
          ])
        })

        this.javaProcessStubReturns.request.withArgs(16, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
          Buffer.from('L'),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 value
            Buffer.from(`L`), // Type
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]), // Value
          ])
        })

        this.javaProcessStubReturns.request.withArgs(9, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]), // Object Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from(`L`), // Type
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x86]), // Class Id
          ])
        })

        this.javaProcessStubReturns.request.withArgs(2, 14, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x86]), // Class Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x03]), // 3 values
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01]), // Field Id
            Buffer.from([0x00, 0x00, 0x00, 0x0f]), // Field Name Length
            Buffer.from(`testChildObject`), // Field Name
            Buffer.from([0x00, 0x00, 0x00, 0x1a]), // Field Signature Length
            Buffer.from(`Lcom/padre/test/TestClass;`), // Field Signature
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic Signature
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Modbits
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02]), // Field Id
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // Field Name Length
            Buffer.from(`a`), // Field Name
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // Field Signature Length
            Buffer.from(`I`), // Field Signature
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic Signature
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Modbits
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x03]), // Field Id
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // Field Name Length
            Buffer.from(`b`), // Field Name
            Buffer.from([0x00, 0x00, 0x00, 0x12]), // Field Signature Length
            Buffer.from(`Ljava/lang/String;`), // Field Signature
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic Signature
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Modbits
          ])
        })

        this.javaProcessStubReturns.request.withArgs(9, 2, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]), // Object Id
          Buffer.from([0x00, 0x00, 0x00, 0x03]), // Number of Fields
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01]), // Field Id
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02]), // Field Id
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x03]), // Field Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x03]), // 3 values
            Buffer.from(`L`), // Tag
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x8a]), // Object Id
            Buffer.from(`I`), // Tag
            Buffer.from([0x00, 0x00, 0x00, 0x7b]), // Value: 123
            Buffer.from(`s`), // Tag
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x8b]), // Object Id
          ])
        })

        this.javaProcessStubReturns.request.withArgs(10, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x8b]), // Object Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x07]), // string length
            Buffer.from(`testing`), // value
          ])
        })

        this.javaProcessStubReturns.request.withArgs(9, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x8a]), // Object Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from(`L`), // Type
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x87]), // Class Id
          ])
        })

        this.javaProcessStubReturns.request.withArgs(2, 14, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x87]), // Class Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 value
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x04]), // Field Id
            Buffer.from([0x00, 0x00, 0x00, 0x10]), // Field Name Length
            Buffer.from(`testChildObject2`), // Field Name
            Buffer.from([0x00, 0x00, 0x00, 0x1b]), // Field Signature Length
            Buffer.from(`Lcom/padre/test/TestClass2;`), // Field Signature
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic Signature
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Modbits
          ])
        })

        this.javaProcessStubReturns.request.withArgs(9, 2, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x8a]), // Object Id
          Buffer.from([0x00, 0x00, 0x00, 0x01]), // Number of Fields
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x04]), // Field Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 values
            Buffer.from(`L`), // Tag
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x8c]), // Object Id
          ])
        })

        this.javaProcessStubReturns.request.withArgs(9, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x8c]), // Object Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from(`L`), // Type
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x88]), // Class Id
          ])
        })

        this.javaProcessStubReturns.request.withArgs(2, 14, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x88]), // Class Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 value
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x05]), // Field Id
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // Field Name Length
            Buffer.from(`c`), // Field Name
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // Field Signature Length
            Buffer.from(`I`), // Field Signature
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic Signature
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Modbits
          ])
        })

        this.javaProcessStubReturns.request.withArgs(9, 2, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x8c]), // Object Id
          Buffer.from([0x00, 0x00, 0x00, 0x01]), // Number of Fields
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x05]), // Field Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 values
            Buffer.from(`I`), // Tag
            Buffer.from([0x00, 0x00, 0x01, 0xc8]), // Value: 456
          ])
        })

        const ret = await this.javaDebugger.printVariable('abc', filename, lineNum)

        chai.expect(ret).to.deep.equal({
          'type': 'JSON',
          'value': {
            'testChildObject': {
              'testChildObject2': {
                'c': 456
              }
            },
            'a': 123,
            'b': 'testing',
          },
          'variable': 'abc',
        })
      })

      it(`should print an object that contains null objects`, async () => {
        const filename = 'test/data/src/com/padre/test/SimpleJavaClass.java'
        const lineNum = 123

        this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
            [`com.padre.test.SimpleJavaClass`, 'main'])

        this.javaProcessStubReturns.request.withArgs(6, 5, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43]),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x02]), // argCnt???
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 slot
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // code index
            Buffer.from([0x00, 0x00, 0x00, 0x03]), // Variable name
            Buffer.from(`abc`),
            Buffer.from([0x00, 0x00, 0x00, 0x20]), // Signature
            Buffer.from(`Lcom/padre/test/SimpleJavaClass;`),
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic Signature
            Buffer.from([0x00, 0x00, 0x00, 0x07]), // Length
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // Slot
          ])
        })

        this.javaProcessStubReturns.request.withArgs(16, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
          Buffer.from('L'),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 value
            Buffer.from(`L`), // Type
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]), // Value
          ])
        })

        this.javaProcessStubReturns.request.withArgs(9, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]), // Object Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from(`L`), // Type
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x86]), // Class Id
          ])
        })

        this.javaProcessStubReturns.request.withArgs(2, 14, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x86]), // Class Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 3 values
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01]), // Field Id
            Buffer.from([0x00, 0x00, 0x00, 0x0f]), // Field Name Length
            Buffer.from(`testChildObject`), // Field Name
            Buffer.from([0x00, 0x00, 0x00, 0x1a]), // Field Signature Length
            Buffer.from(`Lcom/padre/test/TestClass;`), // Field Signature
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic Signature
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Modbits
          ])
        })

        this.javaProcessStubReturns.request.withArgs(9, 2, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]), // Object Id
          Buffer.from([0x00, 0x00, 0x00, 0x01]), // Number of Fields
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01]), // Field Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 3 values
            Buffer.from(`L`), // Tag
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // Null Object
          ])
        })

        const ret = await this.javaDebugger.printVariable('abc', filename, lineNum)

        chai.expect(ret).to.deep.equal({
          'type': 'JSON',
          'value': {
            'testChildObject': null
          },
          'variable': 'abc',
        })
      })

      it(`should print an array of integers`, async () => {
        const filename = 'test/data/src/com/padre/test/SimpleJavaClass.java'
        const lineNum = 123

        this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
            [`com.padre.test.SimpleJavaClass`, 'main'])

        this.javaProcessStubReturns.request.withArgs(6, 5, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43]),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x02]), // argCnt???
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 slot
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // code index
            Buffer.from([0x00, 0x00, 0x00, 0x03]), // Variable name
            Buffer.from(`abc`),
            Buffer.from([0x00, 0x00, 0x00, 0x13]), // Signature
            Buffer.from(`[Ljava/lang/String;`),
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic Signature
            Buffer.from([0x00, 0x00, 0x00, 0x07]), // Length
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // Slot
          ])
        })

        this.javaProcessStubReturns.request.withArgs(16, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
          Buffer.from('['),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 value
            Buffer.from(`[`), // Type
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]), // Value
          ])
        })

        this.javaProcessStubReturns.request.withArgs(13, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x03]) // Length 3
          ])
        })

        this.javaProcessStubReturns.request.withArgs(13, 2, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]),
          Buffer.from([0x00, 0x00, 0x00, 0x00]),
          Buffer.from([0x00, 0x00, 0x00, 0x03]),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x49]), // Integers
            Buffer.from([0x00, 0x00, 0x00, 0x03]),
            Buffer.from([0x00, 0x00, 0x00, 0x01]),
            Buffer.from([0x00, 0x00, 0x00, 0x02]),
            Buffer.from([0x00, 0x00, 0x00, 0x03])
          ])
        })

        const ret = await this.javaDebugger.printVariable('abc', filename, lineNum)

        chai.expect(ret).to.deep.equal({
          'type': 'JSON',
          'value': [1, 2, 3],
          'variable': 'abc',
        })
      })

      it(`should print an array of strings`, async () => {
        const filename = 'test/data/src/com/padre/test/SimpleJavaClass.java'
        const lineNum = 123

        this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
            [`com.padre.test.SimpleJavaClass`, 'main'])

        this.javaProcessStubReturns.request.withArgs(6, 5, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43]),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x02]), // argCnt???
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 slot
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // code index
            Buffer.from([0x00, 0x00, 0x00, 0x03]), // Variable name
            Buffer.from(`abc`),
            Buffer.from([0x00, 0x00, 0x00, 0x13]), // Signature
            Buffer.from(`[Ljava/lang/String;`),
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic Signature
            Buffer.from([0x00, 0x00, 0x00, 0x07]), // Length
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // Slot
          ])
        })

        this.javaProcessStubReturns.request.withArgs(16, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
          Buffer.from('['),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 value
            Buffer.from(`[`), // Type
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]), // Value
          ])
        })

        this.javaProcessStubReturns.request.withArgs(13, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x03]) // Length 3
          ])
        })

        this.javaProcessStubReturns.request.withArgs(13, 2, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]),
          Buffer.from([0x00, 0x00, 0x00, 0x00]),
          Buffer.from([0x00, 0x00, 0x00, 0x03]),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x4c]), // Objects
            Buffer.from([0x00, 0x00, 0x00, 0x03]),
            Buffer.from([0x73]),
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x91]),
            Buffer.from([0x73]),
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x92]),
            Buffer.from([0x73]),
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x93]),
          ])
        })

        this.javaProcessStubReturns.request.withArgs(10, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x91]), // Object Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x05]), // string length
            Buffer.from(`test1`), // value
          ])
        })

        this.javaProcessStubReturns.request.withArgs(10, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x92]), // Object Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x05]), // string length
            Buffer.from(`test2`), // value
          ])
        })

        this.javaProcessStubReturns.request.withArgs(10, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x93]), // Object Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x05]), // string length
            Buffer.from(`test3`), // value
          ])
        })

        const ret = await this.javaDebugger.printVariable('abc', filename, lineNum)

        chai.expect(ret).to.deep.equal({
          'type': 'JSON',
          'value': ['test1', 'test2', 'test3'],
          'variable': 'abc',
        })
      })

      it(`should print an empty array`, async () => {
        const filename = 'test/data/src/com/padre/test/SimpleJavaClass.java'
        const lineNum = 123

        this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
            [`com.padre.test.SimpleJavaClass`, 'main'])

        this.javaProcessStubReturns.request.withArgs(6, 5, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43]),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x02]), // argCnt???
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 slot
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // code index
            Buffer.from([0x00, 0x00, 0x00, 0x03]), // Variable name
            Buffer.from(`abc`),
            Buffer.from([0x00, 0x00, 0x00, 0x13]), // Signature
            Buffer.from(`[Ljava/lang/String;`),
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic Signature
            Buffer.from([0x00, 0x00, 0x00, 0x07]), // Length
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // Slot
          ])
        })

        this.javaProcessStubReturns.request.withArgs(16, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
          Buffer.from('['),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 value
            Buffer.from(`[`), // Type
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]), // Value
          ])
        })

        this.javaProcessStubReturns.request.withArgs(13, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x00])
          ])
        })

        const ret = await this.javaDebugger.printVariable('abc', filename, lineNum)

        chai.expect(ret).to.deep.equal({
          'type': 'JSON',
          'value': [],
          'variable': 'abc',
        })
      })

      it(`should limit the number of children to 10`, async () => {
        const filename = 'test/data/src/com/padre/test/SimpleJavaClass.java'
        const lineNum = 123

        this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
            [`com.padre.test.SimpleJavaClass`, 'main'])

        this.javaProcessStubReturns.request.withArgs(6, 5, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43]),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x02]), // argCnt???
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 slot
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // code index
            Buffer.from([0x00, 0x00, 0x00, 0x03]), // Variable name
            Buffer.from(`abc`),
            Buffer.from([0x00, 0x00, 0x00, 0x20]), // Signature
            Buffer.from(`Lcom/padre/test/SimpleJavaClass;`),
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic Signature
            Buffer.from([0x00, 0x00, 0x00, 0x07]), // Length
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // Slot
          ])
        })

        this.javaProcessStubReturns.request.withArgs(16, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
          Buffer.from([0x00, 0x00, 0x00, 0x01]),
          Buffer.from('L'),
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 value
            Buffer.from(`L`), // Type
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]), // Value
          ])
        })

        this.javaProcessStubReturns.request.withArgs(9, 1, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]), // Object Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from(`L`), // Type
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x86]), // Class Id
          ])
        })

        this.javaProcessStubReturns.request.withArgs(2, 14, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x86]), // Class Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 3 values
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01]), // Field Id
            Buffer.from([0x00, 0x00, 0x00, 0x0f]), // Field Name Length
            Buffer.from(`testChildObject`), // Field Name
            Buffer.from([0x00, 0x00, 0x00, 0x1a]), // Field Signature Length
            Buffer.from(`Lcom/padre/test/TestClass;`), // Field Signature
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic Signature
            Buffer.from([0x00, 0x00, 0x00, 0x00]), // Modbits
          ])
        })

        this.javaProcessStubReturns.request.withArgs(9, 2, Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]), // Object Id
          Buffer.from([0x00, 0x00, 0x00, 0x01]), // Number of Fields
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01]), // Field Id
        ])).returns({
          'errorCode': 0,
          'data': Buffer.concat([
            Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 value
            Buffer.from(`L`), // Tag
            Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]), // Object Id
          ])
        })

        const ret = await this.javaDebugger.printVariable('abc', filename, lineNum)

        chai.expect(ret).to.deep.equal({
          'type': 'JSON',
          'value': {
            'testChildObject': {
              'testChildObject': {
                'testChildObject': {
                  'testChildObject': {
                    'testChildObject': {
                      'testChildObject': {
                        'testChildObject': {
                          'testChildObject': {
                            'testChildObject': {
                              'testChildObject': '...'
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          },
          'variable': 'abc',
        })
      })
    })

    it(`should report a timeout printing a variable`, async () => {
      const filename = '/home/me/code/padre/test/data/src/com/padre/test/SimpleJavaClass.java'
      const lineNum = 12

      this.javaSyntaxGetPositionDataAtLineStub.withArgs(filename, lineNum).returns(
          [`com.padre.test.SimpleJavaClass`, 'main'])

      this.javaProcessStubReturns.request.withArgs(6, 5, Buffer.concat([
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23]),
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43]),
      ])).returns({
        'errorCode': 0,
        'data': Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x02]), // argCnt???
          Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 slot
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // code index
          Buffer.from([0x00, 0x00, 0x00, 0x03]), // Variable name
          Buffer.from(`abc`),
          Buffer.from([0x00, 0x00, 0x00, 0x20]), // Signature
          Buffer.from(`Lcom/padre/test/SimpleJavaClass;`),
          Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic Signature
          Buffer.from([0x00, 0x00, 0x00, 0x07]), // Length
          Buffer.from([0x00, 0x00, 0x00, 0x01]), // Slot
        ])
      })

      this.javaProcessStubReturns.request.withArgs(16, 1, Buffer.concat([
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00]),
        Buffer.from([0x00, 0x00, 0x00, 0x01]),
        Buffer.from([0x00, 0x00, 0x00, 0x01]),
        Buffer.from('L'),
      ])).returns({
        'errorCode': 0,
        'data': Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x01]), // 1 value
          Buffer.from(`L`), // Type
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]), // Value
        ])
      })

      this.javaProcessStubReturns.request.withArgs(9, 1, Buffer.concat([
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]), // Object Id
      ])).returns({
        'errorCode': 0,
        'data': Buffer.concat([
          Buffer.from(`L`), // Type
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x86]), // Class Id
        ])
      })

      this.javaProcessStubReturns.request.withArgs(2, 14, Buffer.concat([
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x86])
      ])).returns({
        'errorCode': 0,
        'data': Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x01]), // One field
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x84]), // Field ID
          Buffer.from([0x00, 0x00, 0x00, 0x03]), // Name length
          Buffer.from(`abc`), // Name
          Buffer.from([0x00, 0x00, 0x00, 0x01]), // Signature length
          Buffer.from(`I`), // Signature
          Buffer.from([0x00, 0x00, 0x00, 0x00]), // Generic signature length
          Buffer.from([0x00, 0x00, 0x00, 0x00]), // modbits
        ])
      })

      this.javaProcessStubReturns.request.withArgs(9, 2, Buffer.concat([
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89]),
        Buffer.from([0x00, 0x00, 0x00, 0x01]),
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x84]),
      ])).returns({
        'errorCode': 0,
        'data': Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x01]), // One value
          Buffer.from(`I`), // Integer
          Buffer.from([0x00, 0x00, 0x00, 0x18]), // Value
        ])
      })

      const printVariablePromise = this.javaDebugger.printVariable('abc', filename, lineNum)

      this.clock.tick(10010)

      let errorFound = null

      try {
        await printVariablePromise
      } catch (error) {
        errorFound = error
      }

      chai.expect(errorFound).to.be.an('error')
    })
  })

  describe('should allow the debugger to set the current position in java', async () => {
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
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x25]), // refTypeId
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

      this.javaProcessStubReturns.request.withArgs(2, 7,
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23])).returns({
        'errorCode': 0,
        'data': Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x14]),
          Buffer.from(`SimpleJavaClass.java`),
        ])
      })

      this.javaProcessStubReturns.request.withArgs(2, 7,
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24])).returns({
        'errorCode': 0,
        'data': Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x14]),
          Buffer.from(`NotExistsClass.java`),
        ])
      })

      this.javaProcessStubReturns.request.withArgs(6, 1).returns({
        'errorCode': 0,
        'data': Buffer.concat([
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // Start from
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x51]), // End at
          Buffer.from([0x00, 0x00, 0x00, 0x09]), // Number of lines
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // Line Code Index
          Buffer.from([0x00, 0x00, 0x00, 0x0c]), // Line Number
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0a]), // Line Code Index
          Buffer.from([0x00, 0x00, 0x00, 0x0e]), // Line Number
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x17]), // Line Code Index
          Buffer.from([0x00, 0x00, 0x00, 0x0f]), // Line Number
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x27]), // Line Code Index
          Buffer.from([0x00, 0x00, 0x00, 0x10]), // Line Number
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x2f]), // Line Code Index
          Buffer.from([0x00, 0x00, 0x00, 0x11]), // Line Number
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x35]), // Line Code Index
          Buffer.from([0x00, 0x00, 0x00, 0x12]), // Line Number
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3b]), // Line Code Index
          Buffer.from([0x00, 0x00, 0x00, 0x13]), // Line Number
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3f]), // Line Code Index
          Buffer.from([0x00, 0x00, 0x00, 0x15]), // Line Number
          Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x49]), // Line Code Index
          Buffer.from([0x00, 0x00, 0x00, 0x16]), // Line Number
        ])
      })

      this.javaDebugger.setup()
    })

    it(`should report the root current position when reported by java`, async () => {
      const javaDebuggerEmitStub = this.sandbox.stub(this.javaDebugger, 'emit')
      javaDebuggerEmitStub.callThrough()

      const threadID = Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x21])

      await this.javaDebugger._handleJavaEventCommand(Buffer.concat([
        Buffer.from([0x02]), // Suspend all
        Buffer.from([0x00, 0x00, 0x00, 0x01]), // One event
        Buffer.from([0x01]), // SINGLE_STEP Event triggered
        Buffer.from([0x00, 0x00, 0x00, 0x02]), // Request ID
        threadID, // Thread ID
        // Location ID
        Buffer.from([0x01]), // Class refType
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23]), // refTypeID
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x42]), // methodID
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // location index
      ]))

      chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
      chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal([
        'process_position', 'test/data/src/com/padre/test/SimpleJavaClass.java', 12
      ])
      chai.expect(this.javaDebugger._currentThreadID).to.deep.equal(threadID)
    })

    it(`should report the current position when reported by java after a single step in`, async () => {
      const javaDebuggerEmitStub = this.sandbox.stub(this.javaDebugger, 'emit')
      javaDebuggerEmitStub.callThrough()

      const threadID = Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x21])

      await this.javaDebugger._handleJavaEventCommand(Buffer.concat([
        Buffer.from([0x02]), // Suspend all
        Buffer.from([0x00, 0x00, 0x00, 0x01]), // One event
        Buffer.from([0x01]), // SINGLE_STEP Event triggered
        Buffer.from([0x00, 0x00, 0x00, 0x03]), // Request ID
        threadID, // Thread ID
        // Location ID
        Buffer.from([0x01]), // Class refType
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23]), // refTypeID
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x42]), // methodID
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0a]), // location index
      ]))

      chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
      chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal([
        'process_position', 'test/data/src/com/padre/test/SimpleJavaClass.java', 14
      ])
      chai.expect(this.javaDebugger._currentThreadID).to.deep.equal(threadID)
    })

    it(`should not report the position when the file can't be found`, async () => {
      const javaDebuggerEmitStub = this.sandbox.stub(this.javaDebugger, 'emit')
      javaDebuggerEmitStub.callThrough()

      const threadID = Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x21])

      await this.javaDebugger._handleJavaEventCommand(Buffer.concat([
        Buffer.from([0x02]), // Suspend all
        Buffer.from([0x00, 0x00, 0x00, 0x01]), // One event
        Buffer.from([0x01]), // SINGLE_STEP Event triggered
        Buffer.from([0x00, 0x00, 0x00, 0x03]), // Request ID
        threadID, // Thread ID
        // Location ID
        Buffer.from([0x01]), // Class refType
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24]), // refTypeID
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x48]), // methodID
        Buffer.from([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // location index
      ]))

      chai.expect(javaDebuggerEmitStub.callCount).to.equal(0)
    })
  })
})
