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

    exeStub.write = sandbox.stub()
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
    chai.expect(lldbPipeStub.args[0]).to.deep.equal([exeStub])

    chai.expect(exeStub.write.callCount).to.equal(2)
    chai.expect(exeStub.write.args[0]).to.deep.equal(['settings set stop-line-count-after 0\n'])
    chai.expect(exeStub.write.args[1]).to.deep.equal(['settings set stop-line-count-before 0\n'])
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

  it('should be able to process multiline output with bash colour codes from LLDB', () => {
    const lldbDebugger = new lldb.LLDB('./test')
    const lldbEmitStub = sandbox.stub(lldbDebugger, 'emit')

    lldbDebugger.write('Current executable set to \'./test\' (x86_64).\r\n(lldb) ' +
        Buffer.from([0x1b, 0x5b, 0x31, 0x47, 0x1b, 0x5b, 0x32, 0x6d]).toString() +
        '(lldb) ' +
        Buffer.from([0x1b, 0x5b, 0x32, 0x32, 0x6d, 0x1b, 0x5b, 0x38, 0x47]).toString())

    chai.expect(lldbEmitStub.callCount).to.equal(1)
    chai.expect(lldbEmitStub.args[0]).to.deep.equal(['started'])
  })

  it('should be able to launch a process and report it', async () => {
    const lldbDebugger = new lldb.LLDB('./test')
    lldbDebugger.exe.write.resetHistory()

    const runPromise = lldbDebugger.run()

    chai.expect(lldbDebugger.exe.write.callCount).to.equal(1)
    chai.expect(lldbDebugger.exe.write.args[0]).to.deep.equal(['process launch\n'])

    lldbDebugger.write(`Process 12345 launched: '/test' (x86_64)\n`)

    const ret = await runPromise

    chai.expect(lldbDebugger._properties.arch).to.equal('x86_64')
    chai.expect(lldbDebugger._properties.pid).to.equal('12345')
    chai.expect(ret).to.deep.equal({'pid': 12345})
  })

  it('should report a process finishing', () => {
    const lldbDebugger = new lldb.LLDB('./test')
    const lldbEmitStub = sandbox.stub(lldbDebugger, 'emit')

    lldbDebugger._properties.pid = '12345'

    lldbDebugger.write(`Process 12345 exited with status = 0 (0x00000000) \r\n(lldb) `)

    chai.expect(lldbEmitStub.callCount).to.equal(1)
    chai.expect(lldbEmitStub.args[0]).to.deep.equal(['process_exit', '0', '12345'])
  })

  it('should allow the debugger to set a breakpoint in LLDB', async () => {
    const lldbDebugger = new lldb.LLDB('./test')
    lldbDebugger.exe.write.resetHistory()

    const breakpointPromise = lldbDebugger.breakpointFileAndLine('main.c', 20)

    chai.expect(lldbDebugger.exe.write.callCount).to.equal(1)
    chai.expect(lldbDebugger.exe.write.args[0]).to.deep.equal(['break set --file main.c --line 20\n'])

    lldbDebugger.write(`Breakpoint 1: where = test\`main + 29 at main.c:20, address = 0x0000000100000f4d`)

    const ret = await breakpointPromise

    chai.expect(ret).to.deep.equal({
      'breakpointId': 1,
      'line': 20,
      'file': 'main.c',
    })
  })

  it('should allow the debugger to step in in LLDB', async () => {
    const lldbDebugger = new lldb.LLDB('./test')
    lldbDebugger.exe.write.resetHistory()

    const stepInPromise = lldbDebugger.stepIn()

    chai.expect(lldbDebugger.exe.write.callCount).to.equal(1)
    chai.expect(lldbDebugger.exe.write.args[0]).to.deep.equal(['thread step-in\n'])

    lldbDebugger.write(`* thread #1, queue = 'com.apple.main-thread', stop reason = step in`)
    lldbDebugger.write(`    frame #0: 0x0000000100000f4d test\`main at test.c:20`)

    const ret = await stepInPromise

    chai.expect(ret).to.deep.equal({})
  })

  it('should allow the debugger to step over in LLDB', async () => {
    const lldbDebugger = new lldb.LLDB('./test')
    lldbDebugger.exe.write.resetHistory()

    const stepOverPromise = lldbDebugger.stepOver()

    chai.expect(lldbDebugger.exe.write.callCount).to.equal(1)
    chai.expect(lldbDebugger.exe.write.args[0]).to.deep.equal(['thread step-over\n'])

    lldbDebugger.write(`* thread #1, queue = 'com.apple.main-thread', stop reason = step over`)
    lldbDebugger.write(`    frame #0: 0x0000000100000f4d test\`main at test.c:20`)

    const ret = await stepOverPromise

    chai.expect(ret).to.deep.equal({})
  })

  it('should allow the debugger to continue in LLDB', async () => {
    const lldbDebugger = new lldb.LLDB('./test')
    lldbDebugger.exe.write.resetHistory()
    lldbDebugger._properties.pid = '12345'

    const continuePromise = lldbDebugger.continue()

    chai.expect(lldbDebugger.exe.write.callCount).to.equal(1)
    chai.expect(lldbDebugger.exe.write.args[0]).to.deep.equal(['thread continue\n'])

    lldbDebugger.write(`Process 12345 resuming`)

    const ret = await continuePromise

    chai.expect(ret).to.deep.equal({})
  })

  it('should allow the debugger to print integers in LLDB', async () => {
    const lldbDebugger = new lldb.LLDB('./test')
    lldbDebugger.exe.write.resetHistory()

    const printVariablePromise = lldbDebugger.printVariable('abc')

    chai.expect(lldbDebugger.exe.write.callCount).to.equal(1)
    chai.expect(lldbDebugger.exe.write.args[0]).to.deep.equal(['frame variable abc\n'])

    lldbDebugger.write(`(int) abc = 123`)

    const ret = await printVariablePromise

    chai.expect(ret).to.deep.equal({
      'variable': 'abc',
      'value': 123,
      'type': 'int',
    })
  })

  it('should report the current position when reported by LLDB', async () => {
    const lldbDebugger = new lldb.LLDB('./test')
    lldbDebugger.exe.write.resetHistory()
    const lldbEmitStub = sandbox.stub(lldbDebugger, 'emit')

    lldbDebugger.write(`    frame #0: 0x0000000100000f86 test_prog\`main at test_prog.c:10`)

    chai.expect(lldbEmitStub.callCount).to.equal(1)
    chai.expect(lldbEmitStub.args[0]).to.deep.equal(['process_position', 10, 'test_prog.c'])
  })
})
