'use strict'

const chai = require('chai')

const javaJNI = require.main.require('src/languages/java/jni')

describe('Test Java JNI Helpers', () => {
  it(`should convert a fully qualified class name to it's JNI signature`, async () => {
    chai.expect(javaJNI.convertClassToJNISignature(`com.padre.test.SimpleJavaClass`))
        .to.equal(`Lcom/padre/test/SimpleJavaClass;`)
  })

  it(`should convert a method name to it's JNI signature`, async () => {
    chai.expect(javaJNI.convertMethodToJNISignature(`void`, [`int`, `char`, `short`]))
        .to.equal(`(ICS)V`)
    chai.expect(javaJNI.convertMethodToJNISignature(`long`, ['int', 'java.lang.String', 'int[]']))
        .to.equal(`(ILjava/lang/String;[I)J`)
  })
})
