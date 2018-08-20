'use strict'

const chai = require('chai')
const sinon = require('sinon')

const javaSyntax = require.main.require('src/languages/java/syntax')

describe('Test Java Packages', () => {
  let sandbox = null

  beforeEach(() => {
    sandbox = sinon.createSandbox()
  })

  afterEach(() => {
    sandbox.restore()
  })

  it('should find the correct class and method name given a Java file with a package', async () => {
    const ret = await javaSyntax.getPositionDataAtLine(
        `test/data/java/src/com/padre/test/SimpleJavaClass.java`, 12)
    chai.expect(ret).to.deep.equal([`com.padre.test.SimpleJavaClass`, `main`])
  })

  it('should find the correct class and method name given a Java file without a package', async () => {
    const ret = await javaSyntax.getPositionDataAtLine(
        `test/data/java/src/com/padre/test/SimpleJavaClassNoPkg.java`, 3)
    chai.expect(ret).to.deep.equal([`SimpleJavaClassNoPkg`, `main`])
  })

  it('should find the correct class and method name given a Java file with several classes', async () => {
    const ret = await javaSyntax.getPositionDataAtLine(
        `test/data/java/src/com/padre/test/JavaMultipleClasses.java`, 12)
    chai.expect(ret).to.deep.equal([`com.padre.test.JavaTestClass1`, `method1`])
  })

  it('should throw an error if a Java file doesn\'t exist', async () => {
    let err = null

    try {
      await javaSyntax.getPositionDataAtLine('file does not exist', 7)
    } catch (error) {
      err = error
    }

    chai.expect(err.message).to.equal(
        `ENOENT: no such file or directory, open 'file does not exist'`)
  })
})
