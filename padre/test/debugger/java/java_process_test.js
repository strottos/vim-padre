'use strict'

const chai = require('chai')
const sinon = require('sinon')

const stream = require('stream')

const nodePty = require('node-pty')
const net = require('net')

const javaProcess = require.main.require('src/debugger/java/java_process')

describe('Test Spawning Java', () => {
  let sandbox = null
  let spawnStub = null
  let exeStub = null
  let exePipeStub = null
  let javaPipeStub = null
  let javaProcessTest = null

  beforeEach(() => {
    sandbox = sinon.createSandbox()

    spawnStub = sandbox.stub(nodePty, 'spawn')
    exeStub = sandbox.stub()
    exePipeStub = sandbox.stub()

    spawnStub.onCall(0).returns(exeStub)

    exeStub.pipe = exePipeStub

    javaPipeStub = sandbox.stub()
    exePipeStub.onCall(0).returns({
      'pipe': javaPipeStub
    })

    javaProcessTest = new javaProcess.JavaProcess('java', ['-jar', 'Test.jar'])
  })

  afterEach(() => {
    sandbox.restore()
  })

  it('should be a Transform stream', () => {
    for (let property in stream.Transform()) {
      chai.expect(javaProcessTest).to.have.property(property)
    }
  })

  it('should successfully spawn java with debug properties', () => {
    javaProcessTest.setup()

    chai.expect(spawnStub.callCount).to.equal(1)
    chai.expect(spawnStub.args[0]).to.deep.equal(['java',
      ['-agentlib:jdwp=transport=dt_socket,address=8457,server=y',
        '-jar', 'Test.jar']])

    chai.expect(exePipeStub.callCount).to.equal(1)
    chai.expect(exePipeStub.args[0]).to.deep.equal([javaProcessTest])

    chai.expect(javaPipeStub.callCount).to.equal(1)
    chai.expect(javaPipeStub.args[0]).to.deep.equal([exeStub])
  })

  it('should throw an error java if it\'s not a java process', () => {
    const javaProcessTest = new javaProcess.JavaProcess('test not java', ['-jar', 'Test.jar'])

    const javaDebuggerEmitStub = sandbox.stub(javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    javaProcessTest.setup()

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal([
      'padre_log', 1, 'Not a java process'])
  })

  it('should report any errors spawning java', () => {
    spawnStub.onCall(0).throws('Test Error')

    const javaDebuggerEmitStub = sandbox.stub(javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    javaProcessTest.setup()

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal([
      'padre_log', 1, 'Test Error'])
  })

  it('should request the ID sizes after starting and get a response', async () => {
    const javaProcessTestOnStub = sandbox.stub(javaProcessTest, 'on')
    javaProcessTestOnStub.withArgs('started').callsArg(1)

    const sendToDebuggerStub = sandbox.stub(javaProcessTest, 'request')
    sendToDebuggerStub.withArgs(1, 7).returns({
      'errorCode': 0,
      'data': Buffer.from([
        0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x08,
        0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x08,
        0x00, 0x00, 0x00, 0x08,
      ])
    })

    await javaProcessTest.setup()

    chai.expect(javaProcessTest._idSizes).to.deep.equal({
      'fieldIDSize': 8,
      'methodIDSize': 8,
      'objectIDSize': 8,
      'referenceTypeIDSize': 8,
      'frameIDSize': 8,
    })
  })

  // TODO
  // it('should report a critical error if the ID sizes get a bad response', async () => {
  // })
})

describe('Test Java Network Communication', () => {
  let sandbox = null
  let connectionStub = null
  let connectionStubReturns = null
  let javaProcessTest = null

  beforeEach(() => {
    sandbox = sinon.createSandbox()

    javaProcessTest = new javaProcess.JavaProcess('java', ['-jar', 'Test.jar'])
    connectionStub = sandbox.stub(net, 'createConnection')
    connectionStubReturns = {
      'on': sandbox.stub(),
      'write': sandbox.stub()
    }
  })

  afterEach(() => {
    sandbox.restore()
  })

  it('should successfully communicate with java', () => {
    connectionStub.withArgs(8457).returns(connectionStubReturns)

    connectionStubReturns.on.withArgs('connect').callsArg(1)

    javaProcessTest.write(`Listening for transport dt_socket at address: 8457\n`)

    chai.expect(connectionStub.callCount).to.equal(1)
    chai.expect(connectionStub.args[0]).to.deep.equal([8457])

    chai.expect(javaProcessTest.connection).to.equal(connectionStubReturns)

    chai.expect(connectionStubReturns.on.callCount).to.equal(3)
    chai.expect(connectionStubReturns.on.args[0][0]).to.equal('connect')
    chai.expect(connectionStubReturns.on.args[1][0]).to.equal('data')
    chai.expect(connectionStubReturns.on.args[2][0]).to.equal('error')

    chai.expect(connectionStubReturns.write.callCount).to.equal(1)
    chai.expect(connectionStubReturns.write.args[0]).to.deep.equal([
      'JDWP-Handshake'])
  })

  it('should start when the handshake has completed', () => {
    const javaDebuggerEmitStub = sandbox.stub(javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    javaProcessTest._handleSocketWrite(`JDWP-Handshake`)

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal(['started'])
  })

  it('should error if there is a connection error', () => {
    connectionStub.withArgs(8457).returns(connectionStubReturns)
    connectionStubReturns.on.withArgs('error').callsArg(1)

    const javaDebuggerEmitStub = sandbox.stub(javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    javaProcessTest.write(`Listening for transport dt_socket at address: 8457\n`)

    chai.expect(connectionStubReturns.on.callCount).to.equal(2)

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal([
      'padre_error', 'Connection Failed'])
  })

  it('should be possible to send data to the debugger', async () => {
    connectionStub.withArgs(8457).returns(connectionStubReturns)

    connectionStubReturns.on.withArgs('connect').callsArg(1)

    javaProcessTest.write(`Listening for transport dt_socket at address: 8457\n`)

    connectionStubReturns.write.resetHistory()

    await javaProcessTest._sendToDebugger(1, 1, Buffer.from([0x01, 0x02, 0x03]))

    chai.expect(connectionStubReturns.write.callCount).to.equal(1)
    chai.expect(connectionStubReturns.write.args[0]).to.deep.equal([
      Buffer.from([
        0x00, 0x00, 0x00, 0x0e, 0x00, 0x00, 0x00, 0x01,
        0x00, 0x01, 0x01, 0x01, 0x02, 0x03
      ])
    ])

    connectionStubReturns.write.resetHistory()

    await javaProcessTest._sendToDebugger(1, 2, Buffer.from([0x02, 0x03, 0x04, 0x05, 0x06]))

    chai.expect(connectionStubReturns.write.callCount).to.equal(1)
    chai.expect(connectionStubReturns.write.args[0]).to.deep.equal([
      Buffer.from([
        0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x02,
        0x00, 0x01, 0x02, 0x02, 0x03, 0x04, 0x05, 0x06,
      ])
    ])

    connectionStubReturns.write.resetHistory()

    await javaProcessTest._sendToDebugger(1, 7)

    chai.expect(connectionStubReturns.write.callCount).to.equal(1)
    chai.expect(connectionStubReturns.write.args[0]).to.deep.equal([
      Buffer.from([
        0x00, 0x00, 0x00, 0x0b, 0x00, 0x00, 0x00, 0x03,
        0x00, 0x01, 0x07
      ])
    ])
  })

  it('should return a response for a request', async () => {
    connectionStub.withArgs(8457).returns(connectionStubReturns)
    connectionStubReturns.on.withArgs('connect').callsArg(1)

    javaProcessTest.write(`Listening for transport dt_socket at address: 8457\n`)

    javaProcessTest._sendToDebugger(1, 2, Buffer.from([0x02, 0x03, 0x04, 0x05, 0x06]))

    const javaDebuggerEmitStub = sandbox.stub(javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    javaProcessTest._handleSocketWrite(Buffer.from([
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

  it('should an error for a bad response from a request', async () => {
    connectionStub.withArgs(8457).returns(connectionStubReturns)
    connectionStubReturns.on.withArgs('connect').callsArg(1)

    javaProcessTest.write(`Listening for transport dt_socket at address: 8457\n`)

    javaProcessTest._sendToDebugger(1, 2, Buffer.from([0x02, 0x03, 0x04, 0x05, 0x06]))

    const javaDebuggerEmitStub = sandbox.stub(javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    javaProcessTest._handleSocketWrite(Buffer.from([
      0x00, 0x00, 0x00, 0x0b, 0x00, 0x00, 0x00, 0x01,
      0x80, 0x01, 0x01
    ]))

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal(['response_1',
      257, Buffer.from([])
    ])
  })

  it('should send data from the socket', async () => {
    const javaDebuggerEmitStub = sandbox.stub(javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    javaProcessTest._handleSocketWrite(Buffer.from([
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
    const javaDebuggerEmitStub = sandbox.stub(javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    javaProcessTest._handleSocketWrite(Buffer.from([
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
    const javaDebuggerEmitStub = sandbox.stub(javaProcessTest, 'emit')
    javaDebuggerEmitStub.callThrough()

    javaProcessTest._handleSocketWrite(Buffer.from([
      0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00,
      0x80, 0x40, 0x64, 0x02, 0x00, 0x00, 0x00, 0x01,
    ]))

    chai.expect(javaDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(javaDebuggerEmitStub.args[0]).to.deep.equal([
      'padre_log', 1, `Can't understand data: id 0 but reply true`
    ])
  })

  it('should be able to send to the debugger and respond synchronously', async () => {
    connectionStub.withArgs(8457).returns(connectionStubReturns)
    connectionStubReturns.on.withArgs('connect').callsArg(1)

    javaProcessTest.write(`Listening for transport dt_socket at address: 8457\n`)

    const sendToDebuggerPromise = javaProcessTest.request(
        1, 2, Buffer.from([0x02, 0x03, 0x04, 0x05, 0x06]))

    javaProcessTest._handleSocketWrite(Buffer.from([
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
