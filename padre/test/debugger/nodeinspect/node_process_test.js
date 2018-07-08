'use strict'

const chai = require('chai')
const sinon = require('sinon')

const stream = require('stream')

const nodePty = require('node-pty')

const nodeProcess = require.main.require('src/debugger/nodeinspect/node_process')

describe('Test Spawning Node with Inspect', () => {
  let sandbox = null
  let spawnStub = null
  let exeStub = null
  let exePipeStub = null
  let nodePipeStub = null

  beforeEach(() => {
    sandbox = sinon.createSandbox()

    spawnStub = sandbox.stub(nodePty, 'spawn')
    exeStub = sandbox.stub()
    exePipeStub = sandbox.stub()

    spawnStub.onCall(0).returns(exeStub)

    exeStub.pipe = exePipeStub

    nodePipeStub = sandbox.stub()
    exePipeStub.onCall(0).returns({
      'pipe': nodePipeStub
    })
  })

  afterEach(() => {
    sandbox.restore()
  })

  it('should be a Transform stream', () => {
    const nodeTestProcess = new nodeProcess.NodeProcess('./test', ['--arg1'])

    for (let property in stream.Transform()) {
      chai.expect(nodeTestProcess).to.have.property(property)
    }
  })

  it('should successfully spawn and communicate with node using inspect', async () => {
    const nodeTestProcess = new nodeProcess.NodeProcess('./test', ['--arg1'])

    const nodeDebuggerEmitStub = sandbox.stub(nodeTestProcess, 'emit')
    nodeDebuggerEmitStub.callThrough()

    await nodeTestProcess.setup()

    chai.expect(spawnStub.callCount).to.equal(1)
    chai.expect(spawnStub.args[0]).to.deep.equal(['node', ['--inspect-brk', './test', '--arg1']])

    chai.expect(exePipeStub.callCount).to.equal(1)
    chai.expect(exePipeStub.args[0]).to.deep.equal([nodeTestProcess])

    chai.expect(nodePipeStub.callCount).to.equal(1)
    chai.expect(nodePipeStub.args[0]).to.deep.equal([exeStub])

    await nodeTestProcess.write('Debugger listening on ws://127.0.0.1:9229/abcd1234-abcd-1234-abcd-1234567890ab\r\nFor help, see: https://nodejs.org/en/docs/inspector')

    chai.expect(nodeDebuggerEmitStub.callCount).to.equal(1)
    chai.expect(nodeDebuggerEmitStub.args[0]).to.deep.equal(['nodestarted'])
  })
})
