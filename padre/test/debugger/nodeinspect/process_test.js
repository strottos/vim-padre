'use strict'

const chai = require('chai')
const sinon = require('sinon')

const stream = require('stream')

const nodePty = require('node-pty')

const nodeProcess = require.main.require('src/debugger/nodeinspect/process')

describe('Test Spawning Node with Inspect', () => {
  beforeEach(() => {
    this.sandbox = sinon.createSandbox()

    this.spawnStub = this.sandbox.stub(nodePty, 'spawn')
    this.exeStub = this.sandbox.stub()
    this.exePipeStub = this.sandbox.stub()

    this.spawnStub.onCall(0).returns(this.exeStub)

    this.exeStub.pipe = this.exePipeStub

    this.nodePipeStub = this.sandbox.stub()
    this.exePipeStub.onCall(0).returns({
      'pipe': this.nodePipeStub
    })
  })

  afterEach(() => {
    this.sandbox.restore()
  })

  it('should be a Transform stream', () => {
    const nodeTestProcess = new nodeProcess.NodeProcess('./test', ['--arg1'])

    for (let property in stream.Transform()) {
      chai.expect(nodeTestProcess).to.have.property(property)
    }
  })

  it('should successfully spawn node using inspect', async () => {
    const nodeTestProcess = new nodeProcess.NodeProcess('./test')

    await nodeTestProcess.run()

    chai.expect(this.spawnStub.callCount).to.equal(1)
    chai.expect(this.spawnStub.args[0]).to.deep.equal(['node', ['--inspect-brk', './test']])

    chai.expect(this.exePipeStub.callCount).to.equal(1)
    chai.expect(this.exePipeStub.args[0]).to.deep.equal([nodeTestProcess])

    chai.expect(this.nodePipeStub.callCount).to.equal(1)
    chai.expect(this.nodePipeStub.args[0]).to.deep.equal([this.exeStub])
  })

  it('should successfully communicate with node using inspect', async () => {
    const nodeTestProcess = new nodeProcess.NodeProcess('./test', ['--arg1'])

    const nodeDebuggerEmitStub = this.sandbox.stub(nodeTestProcess, 'emit')
    nodeDebuggerEmitStub.callThrough()

    await nodeTestProcess.write('Debugger listening on ws://127.0.0.1:9229/abcd1234-abcd-1234-abcd-1234567890ab\r\nFor help, see: https://nodejs.org/en/docs/inspector')

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0]).to.deep.equal(['inspectstarted'])
  })

  it('should report any errors spawning node with inspect', async () => {
    const nodeTestProcess = new nodeProcess.NodeProcess('./test', ['--arg1'])

    this.spawnStub.onCall(0).throws('Test Error', 'Test Message')

    const nodeDebuggerEmitStub = this.sandbox.stub(nodeTestProcess, 'emit')
    nodeDebuggerEmitStub.callThrough()

    await nodeTestProcess.run()

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0][0]).to.equal('inspect_error')
    chai.expect(nodeDebuggerEmitStub.args[0][1]).to.equal('Test Error: Test Message')
  })
})
