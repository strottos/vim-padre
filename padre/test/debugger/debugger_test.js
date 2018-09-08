'use strict'

const chai = require('chai')
const sinon = require('sinon')

const events = require('events')

const debugServer = require.main.require('src/debugger/debugger')

describe('Test the debugger', () => {
  beforeEach(() => {
    this.sandbox = sinon.createSandbox()

    this.testDebugServerStub = new events.EventEmitter()
    this.testDebugServerStub.setup = this.sandbox.stub()
    this.testDebugServerStub.run = this.sandbox.stub()
    this.testDebugServerStub.breakpointFileAndLine = this.sandbox.stub()
    this.testDebugServerStub.stepIn = this.sandbox.stub()
    this.testDebugServerStub.stepOut = this.sandbox.stub()
    this.testDebugServerStub.stepOver = this.sandbox.stub()
    this.testDebugServerStub.continue = this.sandbox.stub()
    this.testDebugServerStub.printVariable = this.sandbox.stub()
    this.testDebugServerStub.write = this.sandbox.stub()

    this.connectionStub = new events.EventEmitter()
    this.connectionStub.write = this.sandbox.stub()
  })

  afterEach(() => {
    this.sandbox.restore()
  })

  it('should be able to interpret a simple request', () => {
    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    const obj = testDebugger._interpret('[1,"breakpoint file=main.c line=20"]')

    chai.expect(obj.id).to.equal(1)
    chai.expect(obj.cmd).to.equal('breakpoint')
    chai.expect(obj.args).to.deep.equal({'line': '20', 'file': 'main.c'})
  })

  it('should be able to run the handler', async () => {
    const connectionStubOn = this.sandbox.stub(this.connectionStub, 'on')
    connectionStubOn.callThrough()

    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)
    testDebugger._handleRequest = sinon.stub() // Suppress unhandled promise warning

    await testDebugger.setup()

    testDebugger.debugServer.emit('started')
    testDebugger.connection.emit('data', Buffer.from('[2,"breakpoint file=main.c line=20"]'))

    chai.expect(this.testDebugServerStub.setup.withArgs().callCount).to.equal(1)

    chai.expect(this.connectionStub.write.callCount).to.equal(1)
    chai.expect(JSON.parse(this.connectionStub.write.args[0][0]))
        .to.deep.equal(['call', 'padre#debugger#SignalPADREStarted', []])

    chai.expect(connectionStubOn.callCount).to.equal(1)
    chai.expect(connectionStubOn.args[0][0]).to.equal('data')
  })

  it('should respond with the exit code when the process exits', async () => {
    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    await testDebugger.setup()

    testDebugger.debugServer.emit('started')
    testDebugger.debugServer.emit('process_exit', '0', '12345')

    chai.expect(this.connectionStub.write.callCount).to.equal(2)
    chai.expect(this.connectionStub.write.args[1]).to.deep.equal(['["call","padre#debugger#ProcessExited",[0,12345]]'])
  })

  it('should catch an error thrown by the debug server and report it', async () => {
    this.testDebugServerStub.run.rejects('Test " Error')

    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    await testDebugger._handleRequest(Buffer.from('[1,"run"]'))

    chai.expect(this.testDebugServerStub.run.callCount).to.equal(1)
    chai.expect(this.testDebugServerStub.run.args[0]).to.deep.equal([])

    chai.expect(this.connectionStub.write.callCount).to.equal(2)
    chai.expect(this.connectionStub.write.args[0]).to.deep.equal(['["call","padre#debugger#Log",[2,"Test \\" Error: "]]'])
    chai.expect(this.connectionStub.write.args[1].length).to.equal(1)
    chai.expect(this.connectionStub.write.args[1][0]).to.match(/^\["call","padre#debugger#Log",\[5,.*/)
  })

  it('should report an error when an error is emitted by the debug server', async () => {
    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    await testDebugger.setup()

    testDebugger.debugServer.emit('padre_log', 2, 'Test " Error')

    chai.expect(this.connectionStub.write.callCount).to.equal(1)
    chai.expect(this.connectionStub.write.args[0]).to.deep.equal(['["call","padre#debugger#Log",[2,"Test \\" Error"]]'])
  })

  it('should report the position when reported by the debugger', async () => {
    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    await testDebugger.setup()

    testDebugger.debugServer.emit('started')
    testDebugger.debugServer.emit('process_position', 'test.c', '10')

    chai.expect(this.connectionStub.write.callCount).to.equal(2)
    chai.expect(JSON.parse(this.connectionStub.write.args[1][0])).to.deep.equal(['call', 'padre#debugger#JumpToPosition', ['test.c', 10]])
  })

  it('should allow the user to run a process', async () => {
    this.testDebugServerStub.run.resolves({
      'pid': 12345,
    })

    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    await testDebugger._handleRequest(Buffer.from('[1,"run"]'))

    chai.expect(this.testDebugServerStub.run.callCount).to.equal(1)
    chai.expect(this.testDebugServerStub.run.args[0]).to.deep.equal([])

    chai.expect(this.connectionStub.write.callCount).to.equal(1)
    chai.expect(this.connectionStub.write.args[0]).to.deep.equal(['[1,"OK pid=12345"]'])
  })

  it('should allow the user to set a breakpoint', async () => {
    this.testDebugServerStub.breakpointFileAndLine.resolves({
      'status': 'OK'
    })

    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    await testDebugger._handleRequest(Buffer.from('[2,"breakpoint file=main.c line=20"]'))

    chai.expect(this.testDebugServerStub.breakpointFileAndLine.callCount).to.equal(1)
    chai.expect(this.testDebugServerStub.breakpointFileAndLine.args[0])
        .to.deep.equal(['main.c', 20])

    chai.expect(this.connectionStub.write.callCount).to.equal(1)
    chai.expect(this.connectionStub.write.args[0]).to.deep.equal(['[2,"OK"]'])
  })

  it('should report a breakpoint when set', async () => {
    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    await testDebugger.setup()

    testDebugger.debugServer.emit('started')
    testDebugger.debugServer.emit('breakpoint_set', 'test.c', '10')

    chai.expect(this.connectionStub.write.callCount).to.equal(2)
    chai.expect(JSON.parse(this.connectionStub.write.args[1][0])).to.deep.equal(['call', 'padre#debugger#BreakpointSet', ['test.c', 10]])
  })

  it('should allow the user to step in', async () => {
    this.testDebugServerStub.stepIn.resolves({})

    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    await testDebugger._handleRequest(Buffer.from('[3,"stepIn"]'))

    chai.expect(this.testDebugServerStub.stepIn.callCount).to.equal(1)
    chai.expect(this.testDebugServerStub.stepIn.args[0]).to.deep.equal([])

    chai.expect(this.connectionStub.write.callCount).to.equal(1)
    chai.expect(this.connectionStub.write.args[0]).to.deep.equal(['[3,"OK"]'])
  })

  it('should allow the user to step over', async () => {
    this.testDebugServerStub.stepOver.resolves({})

    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    await testDebugger._handleRequest(Buffer.from('[4,"stepOver"]'))

    chai.expect(this.testDebugServerStub.stepOver.callCount).to.equal(1)
    chai.expect(this.testDebugServerStub.stepOver.args[0]).to.deep.equal([])

    chai.expect(this.connectionStub.write.callCount).to.equal(1)
    chai.expect(this.connectionStub.write.args[0]).to.deep.equal(['[4,"OK"]'])
  })

  it('should allow the user to continue', async () => {
    this.testDebugServerStub.continue.resolves({})

    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    await testDebugger._handleRequest(Buffer.from('[5,"continue"]'))

    chai.expect(this.testDebugServerStub.continue.callCount).to.equal(1)
    chai.expect(this.testDebugServerStub.continue.args[0]).to.deep.equal([])

    chai.expect(this.connectionStub.write.callCount).to.equal(1)
    chai.expect(this.connectionStub.write.args[0]).to.deep.equal([`[5,"OK"]`])
  })

  it('should allow the user to print a numeric variable', async () => {
    this.testDebugServerStub.printVariable.resolves({
      'type': 'number',
      'variable': 'abc',
      'value': 123,
    })

    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    await testDebugger._handleRequest(Buffer.from('[6,"print variable=abc file=/home/me/padre/something line=123"]'))

    chai.expect(this.testDebugServerStub.printVariable.callCount).to.equal(1)
    chai.expect(this.testDebugServerStub.printVariable.args[0]).to.deep.equal(['abc', '/home/me/padre/something', 123])

    chai.expect(this.connectionStub.write.callCount).to.equal(1)
    chai.expect(this.connectionStub.write.args[0]).to.deep.equal([
      '[6,"OK variable=abc value=123 type=number"]'
    ])
  })

  it('should allow the user to print a string variable', async () => {
    this.testDebugServerStub.printVariable.resolves({
      'type': 'string',
      'variable': 'abc',
      'value': 'Test " String',
    })

    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    await testDebugger._handleRequest(Buffer.from('[6,"print variable=abc file=/home/me/padre/something line=123"]'))

    chai.expect(this.testDebugServerStub.printVariable.callCount).to.equal(1)
    chai.expect(this.testDebugServerStub.printVariable.args[0]).to.deep.equal(['abc', '/home/me/padre/something', 123])

    chai.expect(this.connectionStub.write.callCount).to.equal(1)
    chai.expect(this.connectionStub.write.args[0]).to.deep.equal([
      '[6,"OK variable=abc value=\'Test \\" String\' type=string"]'
    ])
  })

  it('should allow the user to print a JSON object variable', async () => {
    this.testDebugServerStub.printVariable.resolves({
      'type': 'JSON',
      'variable': 'abc',
      'value': {
        'test': 'Test " String',
      }
    })

    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    await testDebugger._handleRequest(Buffer.from('[6,"print variable=abc file=/home/me/padre/something line=123"]'))

    chai.expect(this.testDebugServerStub.printVariable.callCount).to.equal(1)
    chai.expect(this.testDebugServerStub.printVariable.args[0]).to.deep.equal(['abc', '/home/me/padre/something', 123])

    chai.expect(this.connectionStub.write.callCount).to.equal(1)
    chai.expect(this.connectionStub.write.args[0]).to.deep.equal([
      '[6,"OK variable=abc value=\'{\\"test\\":\\"Test \\\\\\" String\\"}\' type=JSON"]'
    ])
  })

  it('should allow the user to print a variable an null type', async () => {
    this.testDebugServerStub.printVariable.resolves({
      'type': 'null',
      'variable': 'abc',
      'value': 'undefined'
    })

    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    await testDebugger._handleRequest(Buffer.from('[6,"print variable=abc file=/home/me/padre/something line=123"]'))

    chai.expect(this.testDebugServerStub.printVariable.callCount).to.equal(1)
    chai.expect(this.testDebugServerStub.printVariable.args[0]).to.deep.equal(['abc', '/home/me/padre/something', 123])

    chai.expect(this.connectionStub.write.callCount).to.equal(1)
    chai.expect(this.connectionStub.write.args[0]).to.deep.equal([
      '[6,"OK variable=abc value=undefined type=null"]'
    ])
  })

  it('should allow the user to print a variable of boolean type', async () => {
    this.testDebugServerStub.printVariable.resolves({
      'type': 'boolean',
      'variable': 'abc',
      'value': 'true'
    })

    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    await testDebugger._handleRequest(Buffer.from('[6,"print variable=abc file=/home/me/padre/something line=123"]'))

    chai.expect(this.testDebugServerStub.printVariable.callCount).to.equal(1)
    chai.expect(this.testDebugServerStub.printVariable.args[0]).to.deep.equal(['abc', '/home/me/padre/something', 123])

    chai.expect(this.connectionStub.write.callCount).to.equal(1)
    chai.expect(this.connectionStub.write.args[0]).to.deep.equal([
      '[6,"OK variable=abc value=true type=boolean"]'
    ])
  })

  it('should report an error if it can\'t understand the return type', async () => {
    this.testDebugServerStub.printVariable.resolves({
      'type': 'test',
      'variable': 'abc',
      'value': 'abc'
    })

    const testDebugger = new debugServer.Debugger(this.testDebugServerStub, this.connectionStub)

    await testDebugger._handleRequest(Buffer.from('[6,"print variable=abc file=/home/me/padre/something line=123"]'))

    chai.expect(this.testDebugServerStub.printVariable.callCount).to.equal(1)
    chai.expect(this.testDebugServerStub.printVariable.args[0]).to.deep.equal(['abc', '/home/me/padre/something', 123])

    chai.expect(this.connectionStub.write.callCount).to.equal(2)
    chai.expect(this.connectionStub.write.args[0]).to.deep.equal([
      '[6,"ERROR"]'
    ])
    chai.expect(this.connectionStub.write.args[1]).to.deep.equal([
      `["call","padre#debugger#Log",[2,"ERROR, can't understand: variable=abc value='\\"abc\\"' type=test"]]`
    ])
  })
})
