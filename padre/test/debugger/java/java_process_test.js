'use strict'

const chai = require('chai')
const sinon = require('sinon')

const events = require('events')
const stream = require('stream')

const nodePty = require('node-pty')
const net = require('net')

const javaProcess = require.main.require('src/debugger/java/java_process')

describe('Test Spawning Java', () => {
  beforeEach(() => {
    this.sandbox = sinon.createSandbox()

    this.spawnStub = this.sandbox.stub(nodePty, 'spawn')
    this.exeStub = this.sandbox.stub()
    this.exePipeStub = this.sandbox.stub()

    this.spawnStub.onCall(0).returns(this.exeStub)

    this.exeStub.pipe = this.exePipeStub

    this.javaPipeStub = this.sandbox.stub()
    this.exePipeStub.onCall(0).returns({
      'pipe': this.javaPipeStub
    })

    this.javaProcessTest = new javaProcess.JavaProcess('java', ['-jar', 'Test.jar'])
  })

  afterEach(() => {
    this.sandbox.restore()
  })

  it('should be a Transform stream', () => {
    for (let property in stream.Transform()) {
      chai.expect(this.javaProcessTest).to.have.property(property)
    }
  })

  it('should successfully spawn java with debug properties', () => {
    this.javaProcessTest.run()

    chai.expect(this.spawnStub.callCount).to.equal(1)
    chai.expect(this.spawnStub.args[0]).to.deep.equal(['java', [
      '-agentlib:jdwp=transport=dt_socket,address=8457,server=y',
      '-jar', 'Test.jar'
    ]])

    chai.expect(this.exePipeStub.callCount).to.equal(1)
    chai.expect(this.exePipeStub.args[0]).to.deep.equal([this.javaProcessTest])

    chai.expect(this.javaPipeStub.callCount).to.equal(1)
    chai.expect(this.javaPipeStub.args[0]).to.deep.equal([this.exeStub])
  })

  it('should successfully spawn maven with debug properties', () => {
    this.javaProcessTest = new javaProcess.JavaProcess('mvn', ['clean', 'test'])

    this.javaProcessTest.run()

    chai.expect(this.spawnStub.callCount).to.equal(1)
    chai.expect(this.spawnStub.args[0]).to.deep.equal(['mvn',
      ['clean', 'test']])

    chai.expect(process.env.MAVEN_DEBUG_OPTS).to.equal(
        '-agentlib:jdwp=transport=dt_socket,address=8457,server=y')

    chai.expect(this.exePipeStub.callCount).to.equal(1)
    chai.expect(this.exePipeStub.args[0]).to.deep.equal([this.javaProcessTest])

    chai.expect(this.javaPipeStub.callCount).to.equal(1)
    chai.expect(this.javaPipeStub.args[0]).to.deep.equal([this.exeStub])
  })

  it('should throw an error if it\'s not a recognised java process', () => {
    const javaProcessTest = new javaProcess.JavaProcess('test not java', ['-jar', 'Test.jar'])

    const javaDebuggerEmitStub = this.sandbox.stub(javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    javaProcessTest.run()

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal([
      'padre_log', 1, 'Not a java process'])
  })

  it('should report any errors spawning java', () => {
    this.spawnStub.onCall(0).throws('Test Error')

    const javaDebuggerEmitStub = this.sandbox.stub(this.javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    this.javaProcessTest.run()

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal([
      'padre_log', 1, 'Test Error'])
  })

  it('should request the ID sizes after starting and get a response', async () => {
    const sendToDebuggerStub = this.sandbox.stub(this.javaProcessTest, 'request')
    sendToDebuggerStub.withArgs(1, 7).returns({
      'errorCode': 0,
      'data': Buffer.from([
        0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x08,
        0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x08,
        0x00, 0x00, 0x00, 0x08,
      ])
    })

    this.javaProcessTest.run()

    await this.javaProcessTest.emit('javadebuggerstarted')

    chai.expect(this.javaProcessTest._idSizes).to.deep.equal({
      'fieldIDSize': 8,
      'methodIDSize': 8,
      'objectIDSize': 8,
      'referenceTypeIDSize': 8,
      'frameIDSize': 8,
    })

    chai.expect(this.javaProcessTest.getFieldIDSize()).to.equal(8)
    chai.expect(this.javaProcessTest.getMethodIDSize()).to.equal(8)
    chai.expect(this.javaProcessTest.getObjectIDSize()).to.equal(8)
    chai.expect(this.javaProcessTest.getReferenceTypeIDSize()).to.equal(8)
    chai.expect(this.javaProcessTest.getFrameIDSize()).to.equal(8)
  })

  // TODO
  // it('should report a critical error if the ID sizes get a bad response', async () => {
  // })
})

describe('Test Java Network Communication', () => {
  beforeEach(() => {
    this.sandbox = sinon.createSandbox()

    this.javaProcessTest = new javaProcess.JavaProcess('java', ['-jar', 'Test.jar'])
    this.connectionStub = this.sandbox.stub(net, 'createConnection')
    this.connectionStubReturns = new events.EventEmitter()
    this.connectionStubReturns.write = this.sandbox.stub()
    this.connectionStub.withArgs(8457).returns(this.connectionStubReturns)
  })

  afterEach(() => {
    this.sandbox.restore()
  })

  it('should successfully communicate with java', () => {
    const connectionStubReturnsOn = this.sandbox.stub(this.connectionStubReturns, 'on')
    connectionStubReturnsOn.callThrough()

    this.javaProcessTest.write(`Listening for transport dt_socket at address: 8457\n`)

    this.connectionStubReturns.emit('connect')

    chai.expect(this.connectionStub.callCount).to.equal(1)
    chai.expect(this.connectionStub.args[0]).to.deep.equal([8457])

    chai.expect(this.javaProcessTest.connection).to.equal(this.connectionStubReturns)

    chai.expect(this.connectionStubReturns.on.callCount).to.equal(3)
    chai.expect(this.connectionStubReturns.on.args[0][0]).to.equal('connect')
    chai.expect(this.connectionStubReturns.on.args[1][0]).to.equal('error')
    chai.expect(this.connectionStubReturns.on.args[2][0]).to.equal('data')

    chai.expect(this.connectionStubReturns.write.callCount).to.equal(1)
    chai.expect(this.connectionStubReturns.write.args[0]).to.deep.equal([
      'JDWP-Handshake'
    ])
  })

  it('should start when the handshake has completed', () => {
    const javaDebuggerEmitStub = this.sandbox.stub(this.javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    this.javaProcessTest._handleSocketWrite(`JDWP-Handshake`)

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal(['javadebuggerstarted'])
  })

  it('should error if there is a connection error', () => {
    const javaDebuggerEmitStub = this.sandbox.stub(this.javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    this.javaProcessTest.write(`Listening for transport dt_socket at address: 8457\n`)

    this.connectionStubReturns.emit('error')

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal([
      'padre_error', 'Connection Failed'])
  })

  it('should be possible to send data to the debugger', async () => {
    this.javaProcessTest.write(`Listening for transport dt_socket at address: 8457\n`)

    this.connectionStubReturns.emit('connect')

    this.connectionStubReturns.write.resetHistory()

    await this.javaProcessTest._sendToDebugger(1, 1, Buffer.from([0x01, 0x02, 0x03]))

    chai.expect(this.connectionStubReturns.write.callCount).to.equal(1)
    chai.expect(this.connectionStubReturns.write.args[0]).to.deep.equal([
      Buffer.from([
        0x00, 0x00, 0x00, 0x0e, 0x00, 0x00, 0x00, 0x01,
        0x00, 0x01, 0x01, 0x01, 0x02, 0x03
      ])
    ])

    this.connectionStubReturns.write.resetHistory()

    await this.javaProcessTest._sendToDebugger(1, 2, Buffer.from([0x02, 0x03, 0x04, 0x05, 0x06]))

    chai.expect(this.connectionStubReturns.write.callCount).to.equal(1)
    chai.expect(this.connectionStubReturns.write.args[0]).to.deep.equal([
      Buffer.from([
        0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x02,
        0x00, 0x01, 0x02, 0x02, 0x03, 0x04, 0x05, 0x06,
      ])
    ])

    this.connectionStubReturns.write.resetHistory()

    await this.javaProcessTest._sendToDebugger(1, 7)

    chai.expect(this.connectionStubReturns.write.callCount).to.equal(1)
    chai.expect(this.connectionStubReturns.write.args[0]).to.deep.equal([
      Buffer.from([
        0x00, 0x00, 0x00, 0x0b, 0x00, 0x00, 0x00, 0x03,
        0x00, 0x01, 0x07
      ])
    ])
  })

  it('should return a response for a simple request', async () => {
    const javaDebuggerEmitStub = this.sandbox.stub(this.javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    this.javaProcessTest._handleSocketWrite(Buffer.from([
      0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x01,
      0x80, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
    ]))

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal(['response_1',
      0, Buffer.from([
        0x01, 0x02, 0x03, 0x04, 0x05,
      ])
    ])
  })

  it('should wait for a full response from a request before returning the response', async () => {
    const javaDebuggerEmitStub = this.sandbox.stub(this.javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    this.javaProcessTest._handleSocketWrite(Buffer.concat([
      Buffer.from('JDWP-Handshake'),
      Buffer.from([
        0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x01,
        0x80, 0x00, 0x00, 0x00,
      ])
    ]))

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)

    this.javaProcessTest._handleSocketWrite(Buffer.from([
      0x00, 0x00, 0x00, 0x01,
    ]))

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(2)
    chai.expect(javaDebuggerEmitStub.args[1]).to.deep.equal(['response_1',
      0, Buffer.from([0x00, 0x00, 0x00, 0x00, 0x01])
    ])
  })

  it('should send data from the socket', async () => {
    const javaDebuggerEmitStub = this.sandbox.stub(this.javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    this.javaProcessTest._handleSocketWrite(Buffer.from([
      0x00, 0x00, 0x00, 0x1d, 0x00, 0x00, 0x00, 0x00,
      0x00, 0x40, 0x64, 0x02, 0x00, 0x00, 0x00, 0x01,
      0x5a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
      0x00, 0x00, 0x00, 0x00, 0x01,
    ]))

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal(['request',
      64, 100, Buffer.from([
        0x02, 0x00, 0x00, 0x00, 0x01, 0x5a, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x01,
      ])
    ])
  })

  it('should be possible to handle multiple requests in one write', async () => {
    const javaDebuggerEmitStub = this.sandbox.stub(this.javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    this.javaProcessTest._handleSocketWrite(Buffer.from([
      0x00, 0x00, 0x00, 0x1d, 0x00, 0x00, 0x00, 0x00,
      0x00, 0x40, 0x64, 0x02, 0x00, 0x00, 0x00, 0x01,
      0x5a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
      0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
      0x1f, 0x00, 0x00, 0x00, 0x01, 0x80, 0x00, 0x00,
      0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x08,
      0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x08,
      0x00, 0x00, 0x00, 0x08,
    ]))

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(2)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal(['request',
      64, 100, Buffer.from([
        0x02, 0x00, 0x00, 0x00, 0x01, 0x5a, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x01,
      ])
    ])

    chai.expect(javaDebuggerEmitStub.args[1]).to.deep.equal(['response_1',
      0, Buffer.from([
        0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x08,
        0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x08,
        0x00, 0x00, 0x00, 0x08,
      ])
    ])
  })

  it('should report being a reply without an id as a critical error', async () => {
    const javaDebuggerEmitStub = this.sandbox.stub(this.javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    this.javaProcessTest._handleSocketWrite(Buffer.from([
      0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00,
      0x80, 0x40, 0x64, 0x02, 0x00, 0x00, 0x00, 0x01,
    ]))

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal([
      'padre_log', 1, `Can't understand data: id 0 but reply true`
    ])
  })

  it('should be able to send to the debugger and respond synchronously', async () => {
    this.javaProcessTest.write(`Listening for transport dt_socket at address: 8457\n`)

    this.connectionStubReturns.emit('connect')

    const sendToDebuggerPromise = this.javaProcessTest.request(
        1, 2, Buffer.from([0x02, 0x03, 0x04, 0x05, 0x06]))

    this.javaProcessTest._handleSocketWrite(Buffer.from([
      0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x01,
      0x80, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
    ]))

    const ret = await sendToDebuggerPromise

    chai.expect(ret).to.deep.equal({
      'errorCode': 0,
      'data': Buffer.from([
        0x01, 0x02, 0x03, 0x04, 0x05,
      ])
    })
  })
})
