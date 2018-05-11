const chai = require('chai')
const sinon = require('sinon')

const stream = require('stream')

const nodePty = require('node-pty')

const lldb = require.main.require('src/debugger/lldb/lldb')

describe('Test Spawning LLDB', () => {
  let sandbox = null
  let spawnStub = null
  let exeStub = null
  let exePipeStub = null
  let lldbPipeStub = null

  beforeEach(() => {
    sandbox = sinon.createSandbox()

    spawnStub = sandbox.stub(nodePty, 'spawn')
    exeStub = sandbox.stub()
    exePipeStub = sandbox.stub()
    exeStub.pipe = exePipeStub
    spawnStub.onCall(0).returns(exeStub)

    lldbPipeStub = sandbox.stub()
    exePipeStub.onCall(0).returns({
      'pipe': lldbPipeStub
    })
  })

  afterEach(() => {
    sandbox.restore()
  })

  it('should be a Transform stream', () => {
    const lldbDebugger = new lldb.LLDB('./test')

    for (let property in stream.Transform()) {
      chai.expect(lldbDebugger).to.have.property(property)
    }
  })

  it('should successfully spawn and communicate with LLDB', () => {
    const lldbDebugger = new lldb.LLDB('./test')

    chai.expect(spawnStub.callCount).to.equal(1)
    chai.expect(spawnStub.args[0]).to.deep.equal(['lldb', ['--', './test']])

    chai.expect(exePipeStub.callCount).to.equal(1)
    chai.expect(exePipeStub.args[0]).to.deep.equal([lldbDebugger])

    chai.expect(lldbPipeStub.callCount).to.equal(1)
    chai.expect(lldbPipeStub.args[0][0]).to.equal(exeStub)
  })

  it('should correctly spawn LLDB when arguments are used', () => {
    new lldb.LLDB('./test', ['--arg1', '--arg2=test', '-a', '--', 'testing'])

    chai.expect(spawnStub.callCount).to.equal(1)
    chai.expect(spawnStub.args[0]).to.deep.equal(['lldb', ['--', './test', '--arg1', '--arg2=test', '-a', '--', 'testing']])
  })

  it('should be able to write to and start LLDB', () => {
    const lldbDebugger = new lldb.LLDB('./test')
    const lldbEmitStub = sandbox.stub(lldbDebugger, 'emit')

    let strings = [`(lldb) target create "./test"`,
        `Current executable set to './test' (x86_64).`,
        `(lldb) `, `(lldb) `]
    for (let s of strings) {
      lldbDebugger.write(s + '\n')
    }

    chai.expect(lldbEmitStub.callCount).to.equal(1)
    chai.expect(lldbEmitStub.args[0]).to.deep.equal(['started'])
  })

  it('should be able to process multiline output with unicode from LLDB', () => {
    const lldbDebugger = new lldb.LLDB('./test')
    const lldbEmitStub = sandbox.stub(lldbDebugger, 'emit')

    lldbDebugger.write('Current executable set to \'./test\' (x86_64).\r\n(lldb) ' +
        Buffer.from([0x1b, 0x5b, 0x31, 0x47, 0x1b, 0x5b, 0x32, 0x6d]).toString() +
        '(lldb) ' +
        Buffer.from([0x1b, 0x5b, 0x32, 0x32, 0x6d, 0x1b, 0x5b, 0x38, 0x47]).toString())

    chai.expect(lldbEmitStub.callCount).to.equal(1)
    chai.expect(lldbEmitStub.args[0]).to.deep.equal(['started'])
  })

  it('should be able to launch a process and report it', () => {
    exeStub.write = sandbox.stub()

    const lldbDebugger = new lldb.LLDB('./test')
    const lldbEmitStub = sandbox.stub(lldbDebugger, 'emit')

    lldbDebugger.run()

    chai.expect(lldbDebugger.exe.write.callCount).to.equal(1)
    chai.expect(lldbDebugger.exe.write.args[0]).to.deep.equal(['run\n'])

    lldbDebugger.write(`Process 12345 launched: '/test' (x86_64)\n`)

    chai.expect(lldbDebugger._properties.arch).to.equal('x86_64')
    chai.expect(lldbDebugger._properties.pid).to.equal('12345')
    chai.expect(lldbEmitStub.callCount).to.equal(1)
    chai.expect(lldbEmitStub.args[0]).to.deep.equal(['process_spawn', '12345'])
  })

  it('should report a process finishing', () => {
    exeStub.write = sandbox.stub()

    const lldbDebugger = new lldb.LLDB('./test')
    const lldbEmitStub = sandbox.stub(lldbDebugger, 'emit')

    lldbDebugger._properties.pid = '12345'

    lldbDebugger.write(`Process 12345 exited with status = 0 (0x00000000) \r\n(lldb) `)

    chai.expect(lldbEmitStub.callCount).to.equal(1)
    chai.expect(lldbEmitStub.args[0]).to.deep.equal(['process_exit', '0'])
  })
})
