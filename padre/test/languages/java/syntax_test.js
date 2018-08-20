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

  it('should find the correct class name given a Java file with a packge', async () => {
    const ret = await javaSyntax.getClassAtLine(
        'test/data/src/com/padre/test/SimpleJavaClass.java', 7)
    chai.expect(ret).to.equal('com.padre.test.SimpleJavaClass')
  })

  it('should find the correct class name given a Java file without a packge', async () => {
    const ret = await javaSyntax.getClassAtLine(
        'test/data/src/com/padre/test/SimpleJavaClassNoPkg.java', 7)
    chai.expect(ret).to.equal('SimpleJavaClassNoPkg')
  })

  it('should throw an error if a Java file doesn\'t exist', async () => {
    let err = null

    try {
      await javaSyntax.getClassAtLine('file does not exist', 7)
    } catch (error) {
      err = error
    }

    chai.expect(err.message).to.equal(
        `ENOENT: no such file or directory, open 'file does not exist'`)
  })
})
