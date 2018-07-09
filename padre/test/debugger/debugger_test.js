'use strict'

const chai = require('chai')
const sinon = require('sinon')

const debugServer = require.main.require('src/debugger/debugger')

describe('Test the debugger', () => {
  let sandbox = null
  let testDebugServerStub = null
  let connectionStub = null

  beforeEach(() => {
    sandbox = sinon.createSandbox()
    testDebugServerStub = {
      'setup': sandbox.stub(),
      'on': sandbox.stub(),
      'run': sandbox.stub(),
      'breakpointFileAndLine': sandbox.stub(),
      'stepIn': sandbox.stub(),
      'stepOut': sandbox.stub(),
      'stepOver': sandbox.stub(),
      'continue': sandbox.stub(),
      'printVariable': sandbox.stub(),
      'write': sandbox.stub(),
    }
    connectionStub = {
      'on': sandbox.stub(),
      'write': sandbox.stub(),
    }
  })

  afterEach(() => {
    sandbox.restore()
  })

  it('should be able to interpret a simple request', () => {
    const testDebugger = new debugServer.Debugger(testDebugServerStub, connectionStub)

    const obj = testDebugger._interpret('[1,"breakpoint line=20 file=main.c"]')

    chai.expect(obj.id).to.equal(1)
    chai.expect(obj.cmd).to.equal('breakpoint')
    chai.expect(obj.args).to.deep.equal({'line': '20', 'file': 'main.c'})
  })

  it('should be able to run the handler', async () => {
    testDebugServerStub.on.withArgs('started', sinon.match.any).callsArg(1)
    connectionStub.on.withArgs('data', sinon.match.any)
        .callsArgWith(1, Buffer.from('[2,"breakpoint line=20 file=main.c"]'))

    const testDebugger = new debugServer.Debugger(testDebugServerStub, connectionStub)
    testDebugger._handleReadData = sinon.stub() // Suppress unhandled promise warning

    await testDebugger.handle()

    chai.expect(testDebugServerStub.setup.withArgs().callCount).to.equal(1)

    chai.expect(testDebugServerStub.on.withArgs('started', sinon.match.any).callCount).to.equal(1)
    chai.expect(testDebugServerStub.on.args[0][0]).to.equal('started')

    chai.expect(connectionStub.write.callCount).to.equal(1)
    chai.expect(JSON.parse(connectionStub.write.args[0][0]))
        .to.deep.equal(['call', 'padre#debugger#SignalPADREStarted', []])

    chai.expect(connectionStub.on.callCount).to.equal(1)
    chai.expect(connectionStub.on.args[0][0]).to.equal('data')
  })

  it('should allow the user to run a process', async () => {
    testDebugServerStub.run.resolves({
      'pid': 12345,
    })

    const testDebugger = new debugServer.Debugger(testDebugServerStub, connectionStub)

    await testDebugger._handleReadData(Buffer.from('[1,"run"]'))

    chai.expect(testDebugServerStub.run.callCount).to.equal(1)
    chai.expect(testDebugServerStub.run.args[0]).to.deep.equal([])

    chai.expect(connectionStub.write.callCount).to.equal(1)
    chai.expect(connectionStub.write.args[0]).to.deep.equal(['[1,"OK pid=12345"]'])
  })

  it('should respond with the exit code when the process exits', async () => {
    testDebugServerStub.on
        .withArgs('started', sinon.match.any).callsArg(1)
        .withArgs('process_exit', sinon.match.any).callsArgWith(1, '0', '12345')

    const testDebugger = new debugServer.Debugger(testDebugServerStub, connectionStub)

    await testDebugger.handle()

    chai.expect(connectionStub.write.callCount).to.equal(2)
    chai.expect(connectionStub.write.args[1]).to.deep.equal(['["call","padre#debugger#ProcessExited",[0,12345]]'])
  })

  it('should allow the user to set a breakpoint', async () => {
    testDebugServerStub.breakpointFileAndLine.resolves({
      'breakpointId': 1,
      'file': 'main.c',
      'line': 20,
    })

    const testDebugger = new debugServer.Debugger(testDebugServerStub, connectionStub)

    await testDebugger._handleReadData(Buffer.from('[2,"breakpoint line=20 file=main.c"]'))

    chai.expect(testDebugServerStub.breakpointFileAndLine.callCount).to.equal(1)
    chai.expect(testDebugServerStub.breakpointFileAndLine.args[0])
        .to.deep.equal(['main.c', 20])

    chai.expect(connectionStub.write.callCount).to.equal(1)
    chai.expect(connectionStub.write.args[0]).to.deep.equal(['[2,"OK line=20 file=main.c"]'])
  })

  it('should allow the user to step in', async () => {
    testDebugServerStub.stepIn.resolves({})

    const testDebugger = new debugServer.Debugger(testDebugServerStub, connectionStub)

    await testDebugger._handleReadData(Buffer.from('[3,"stepIn"]'))

    chai.expect(testDebugServerStub.stepIn.callCount).to.equal(1)
    chai.expect(testDebugServerStub.stepIn.args[0]).to.deep.equal([])

    chai.expect(connectionStub.write.callCount).to.equal(1)
    chai.expect(connectionStub.write.args[0]).to.deep.equal(['[3,"OK"]'])
  })

  it('should allow the user to step over', async () => {
    testDebugServerStub.stepOver.resolves({})

    const testDebugger = new debugServer.Debugger(testDebugServerStub, connectionStub)

    await testDebugger._handleReadData(Buffer.from('[4,"stepOver"]'))

    chai.expect(testDebugServerStub.stepOver.callCount).to.equal(1)
    chai.expect(testDebugServerStub.stepOver.args[0]).to.deep.equal([])

    chai.expect(connectionStub.write.callCount).to.equal(1)
    chai.expect(connectionStub.write.args[0]).to.deep.equal(['[4,"OK"]'])
  })

  it('should allow the user to continue', async () => {
    testDebugServerStub.continue.resolves({})

    const testDebugger = new debugServer.Debugger(testDebugServerStub, connectionStub)

    await testDebugger._handleReadData(Buffer.from('[5,"continue"]'))

    chai.expect(testDebugServerStub.continue.callCount).to.equal(1)
    chai.expect(testDebugServerStub.continue.args[0]).to.deep.equal([])

    chai.expect(connectionStub.write.callCount).to.equal(1)
    chai.expect(connectionStub.write.args[0]).to.deep.equal(['[5,"OK"]'])
  })

  it('should allow the user to print an integer variable', async () => {
    testDebugServerStub.printVariable.resolves({
      'type': 'int',
      'variable': 'abc',
      'value': 123,
    })

    const testDebugger = new debugServer.Debugger(testDebugServerStub, connectionStub)

    await testDebugger._handleReadData(Buffer.from('[6,"print variable=abc"]'))

    chai.expect(testDebugServerStub.printVariable.callCount).to.equal(1)
    chai.expect(testDebugServerStub.printVariable.args[0]).to.deep.equal(['abc'])

    chai.expect(connectionStub.write.callCount).to.equal(1)
    chai.expect(connectionStub.write.args[0]).to.deep.equal(['[6,"OK variable=abc value=123 type=int"]'])
  })

  it('should allow the user to print a string variable', async () => {
    testDebugServerStub.printVariable.resolves({
      'type': 'str',
      'variable': 'abc',
      'value': 'test',
    })

    const testDebugger = new debugServer.Debugger(testDebugServerStub, connectionStub)

    await testDebugger._handleReadData(Buffer.from('[6,"print variable=abc"]'))

    chai.expect(testDebugServerStub.printVariable.callCount).to.equal(1)
    chai.expect(testDebugServerStub.printVariable.args[0]).to.deep.equal(['abc'])

    chai.expect(connectionStub.write.callCount).to.equal(1)
    chai.expect(connectionStub.write.args[0]).to.deep.equal(['[6,"OK variable=abc value=test type=str"]'])
  })

  it('should report the position when reported by the debugger', async () => {
    testDebugServerStub.on
        .withArgs('started', sinon.match.any).callsArg(1)
        .withArgs('process_position', sinon.match.any).callsArgWith(1, '10', 'test.c')

    const testDebugger = new debugServer.Debugger(testDebugServerStub, connectionStub)

    await testDebugger.handle()

    chai.expect(connectionStub.write.callCount).to.equal(2)
    console.log(connectionStub.write.args[1][0])
    chai.expect(JSON.parse(connectionStub.write.args[1][0])).to.deep.equal(['call', 'padre#debugger#JumpToPosition', [10, 'test.c']])
  })

  it('should catch an error thrown by the debug server and report it', async () => {
    testDebugServerStub.run.rejects('Test Error')

    const testDebugger = new debugServer.Debugger(testDebugServerStub, connectionStub)

    await testDebugger._handleReadData(Buffer.from('[1,"run"]'))

    chai.expect(testDebugServerStub.run.callCount).to.equal(1)
    chai.expect(testDebugServerStub.run.args[0]).to.deep.equal([])

    chai.expect(connectionStub.write.callCount).to.equal(1)
    chai.expect(connectionStub.write.args[0]).to.deep.equal(['["call","padre#debugger#Error",["Test Error"]]'])
  })

  it('should report an error when an error is emitted by the debug server', async () => {
    testDebugServerStub.on
        .withArgs('started', sinon.match.any).callsArg(1)
        .withArgs('padre_error', sinon.match.any).callsArgWith(1, 'Test Error')

    const testDebugger = new debugServer.Debugger(testDebugServerStub, connectionStub)

    await testDebugger.handle()

    chai.expect(connectionStub.write.callCount).to.equal(2)
    chai.expect(connectionStub.write.args[1]).to.deep.equal(['["call","padre#debugger#Error",["Test Error"]]'])
  })
})
