'use strict'

const chai = require('chai')
const sinon = require('sinon')

const stream = require('stream')

const nodePty = require('node-pty')

const pdb = require.main.require('src/debugger/python/pdb')

describe('Test Spawning PDB', () => {
  beforeEach(() => {
    this.sandbox = sinon.createSandbox()

    this.spawnStub = this.sandbox.stub(nodePty, 'spawn')
    this.exeStub = this.sandbox.stub()
    this.exePipeStub = this.sandbox.stub()
    this.exeStub.pipe = this.exePipeStub
    this.spawnStub.onCall(0).returns(this.exeStub)

    this.pdbPipeStub = this.sandbox.stub()
    this.exePipeStub.onCall(0).returns({
      'pipe': this.pdbPipeStub
    })

    this.exeStub.write = this.sandbox.stub()
  })

  afterEach(() => {
    this.sandbox.restore()
  })

  it('should be a Transform stream', () => {
    const pdbDebugger = new pdb.PDB('./test')

    for (let property in stream.Transform()) {
      chai.expect(pdbDebugger).to.have.property(property)
    }
  })

  it('should report it has started successfully', async () => {
    const pdbDebugger = new pdb.PDB('./test')

    const pdbEmitStub = this.sandbox.stub(pdbDebugger, 'emit')

    pdbDebugger.setup()

    chai.expect(pdbEmitStub.callCount).to.equal(1)
    chai.expect(pdbEmitStub.args[0]).to.deep.equal(['started'])
  })

  it('should successfully spawn a standalone program with python and pdb', async () => {
    const pdbDebugger = new pdb.PDB('./test')

    await pdbDebugger.run()

    chai.expect(this.spawnStub.callCount).to.equal(1)
    chai.expect(this.spawnStub.args[0]).to.deep.equal(['python3', ['-m', 'pdb', './test']]) // TODO: Python 2?

    chai.expect(this.exePipeStub.callCount).to.equal(1)
    chai.expect(this.exePipeStub.args[0]).to.deep.equal([pdbDebugger])

    chai.expect(this.pdbPipeStub.callCount).to.equal(1)
    chai.expect(this.pdbPipeStub.args[0]).to.deep.equal([this.exeStub])
  })

  it('should successfully spawn a python executable with python and pdb', async () => {
    const pdbDebugger = new pdb.PDB('python3', ['test.py'])

    await pdbDebugger.run()

    chai.expect(this.spawnStub.callCount).to.equal(1)
    chai.expect(this.spawnStub.args[0]).to.deep.equal(['python3', ['-m', 'pdb', 'test.py']]) // TODO: Python 2?

    chai.expect(this.exePipeStub.callCount).to.equal(1)
    chai.expect(this.exePipeStub.args[0]).to.deep.equal([pdbDebugger])

    chai.expect(this.pdbPipeStub.callCount).to.equal(1)
    chai.expect(this.pdbPipeStub.args[0]).to.deep.equal([this.exeStub])
  })

  it('should correctly spawn python with pdb when arguments are used', async () => {
    const pdbDebugger = new pdb.PDB('./test', ['--arg1', '--arg2=test', '-a', '--', 'testing'])
    await pdbDebugger.run()

    chai.expect(this.spawnStub.callCount).to.equal(1)
    chai.expect(this.spawnStub.args[0]).to.deep.equal(['python3', ['-m', 'pdb', './test', '--arg1', '--arg2=test', '-a', '--', 'testing']])
  })

  it('should successfully report a spawned python script', async () => {
    const pdbDebugger = new pdb.PDB('python3', ['test.py'])

    const runPromise = pdbDebugger.run()

    pdbDebugger.write(`> /home/strotter/virtualenvs/bdd/bin/pytest(4)<module>()\n`)
    pdbDebugger.write(`-> import re\n`)

    const ret = await runPromise

    chai.expect(ret).to.deep.equal({'pid': 0})
  })

  it('should report a process finishing', () => {
    const pdbDebugger = new pdb.PDB('./test')
    pdbDebugger.setup()

    const pdbEmitStub = this.sandbox.stub(pdbDebugger, 'emit')

    pdbDebugger.write(`The program finished and will be restarted\n`)

    chai.expect(pdbEmitStub.callCount).to.equal(1)
    chai.expect(pdbEmitStub.args[0]).to.deep.equal(['process_exit', '0', '0'])
  })

  it('should allow the debugger to set a breakpoint in pdb', async () => {
    const pdbDebugger = new pdb.PDB('./test')

    await pdbDebugger.run()

    const breakpointPromise = pdbDebugger.breakpointFileAndLine('./test', 20)

    chai.expect(this.exeStub.write.callCount).to.equal(1)
    chai.expect(this.exeStub.write.args[0]).to.deep.equal(['break ./test:20\n'])

    pdbDebugger.write(`Breakpoint 1 at /home/me/test:20`)

    const ret = await breakpointPromise

    chai.expect(ret).to.deep.equal({
      'breakpointId': 1,
      'line': 20,
      'file': '/home/me/test',
    })
  })

  it('should allow the debugger to step in in pdb', async () => {
    const pdbDebugger = new pdb.PDB('./test')

    await pdbDebugger.run()

    const ret = await pdbDebugger.stepIn()

    chai.expect(pdbDebugger.exe.write.callCount).to.equal(1)
    chai.expect(pdbDebugger.exe.write.args[0]).to.deep.equal(['step\n'])

    chai.expect(ret).to.deep.equal({})
  })

  it('should allow the debugger to step over in pdb', async () => {
    const pdbDebugger = new pdb.PDB('./test')

    await pdbDebugger.run()

    const ret = await pdbDebugger.stepOver()

    chai.expect(pdbDebugger.exe.write.callCount).to.equal(1)
    chai.expect(pdbDebugger.exe.write.args[0]).to.deep.equal(['next\n'])

    chai.expect(ret).to.deep.equal({})
  })

  it('should allow the debugger to continue in pdb', async () => {
    const pdbDebugger = new pdb.PDB('./test')

    await pdbDebugger.run()

    const ret = await pdbDebugger.continue()

    chai.expect(pdbDebugger.exe.write.callCount).to.equal(1)
    chai.expect(pdbDebugger.exe.write.args[0]).to.deep.equal(['continue\n'])

    chai.expect(ret).to.deep.equal({})
  })

  it('should allow the debugger to print variables in pdb', async () => {
    const pdbDebugger = new pdb.PDB('./test')

    await pdbDebugger.run()

    const printVariablePromise = pdbDebugger.printVariable('abc')

    chai.expect(pdbDebugger.exe.write.callCount).to.equal(1)
    chai.expect(pdbDebugger.exe.write.args[0]).to.deep.equal(['print(abc)\n'])

    pdbDebugger.write(`print(abc)`)
    pdbDebugger.write(`123`)

    const ret = await printVariablePromise

    chai.expect(ret).to.deep.equal({
      'variable': 'abc',
      'value': '123',
      'type': 'string',
    })
  })

  it('should report the current position when reported by pdb', async () => {
    const pdbDebugger = new pdb.PDB('python3', ['test.py'])

    const pdbEmitStub = this.sandbox.stub(pdbDebugger, 'emit')

    pdbDebugger.write(`> /home/me/test.py(46)main()`)

    chai.expect(pdbEmitStub.callCount).to.equal(1)
    chai.expect(pdbEmitStub.args[0]).to.deep.equal(['process_position', '/home/me/test.py', 46])
  })
})
