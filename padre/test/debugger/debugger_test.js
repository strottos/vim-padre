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
      'on': sandbox.stub(),
      'run': sandbox.stub(),
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

  it('should be able to run a process', () => {
    testDebugServerStub.on.withArgs('started', sinon.match.any).callsArg(1)
    connectionStub.on.withArgs('data', sinon.match.any).callsArgWith(1, Buffer.from('run'))

    const testDebugger = new debugServer.Debugger(testDebugServerStub, connectionStub)

    testDebugger.handle()

    chai.expect(testDebugServerStub.run.callCount).to.equal(1)
  })

  it('should respond with the pid when the process is launched', () => {
    testDebugServerStub.on
        .withArgs('started', sinon.match.any).callsArg(1)
        .withArgs('process_spawn', sinon.match.any).callsArgWith(1, '12345')

    const testDebugger = new debugServer.Debugger(testDebugServerStub, connectionStub)

    testDebugger.handle()

    chai.expect(connectionStub.write.callCount).to.equal(1)
    chai.expect(connectionStub.write.args[0]).to.deep.equal(['pid=12345\n'])
  })

  it('should respond with the exit code when the process exits', () => {
    testDebugServerStub.on
        .withArgs('started', sinon.match.any).callsArg(1)
        .withArgs('process_exit', sinon.match.any).callsArgWith(1, '0')

    console.log('here1')
    const testDebugger = new debugServer.Debugger(testDebugServerStub, connectionStub)

    console.log('here2')
    testDebugger.handle()
    console.log('here3')

    chai.expect(connectionStub.write.callCount).to.equal(1)
    chai.expect(connectionStub.write.args[0]).to.deep.equal(['exitcode=0\n'])
  })
})
