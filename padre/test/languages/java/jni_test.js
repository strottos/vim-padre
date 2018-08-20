'use strict'

const chai = require('chai')

const javaJNI = require.main.require('src/languages/java/jni')

describe('Test Java JNI Helpers', () => {
  it(`should convert a fully qualified class name to it's JNI signature`, async () => {
    chai.expect(javaJNI.covertClassToJNISignature(`com.padre.test.SimpleJavaClass`))
        .to.equal(`Lcom/padre/test/SimpleJavaClass;`)
  })
})
