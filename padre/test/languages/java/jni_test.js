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

  it(`should convert a classes JNI signature to it's class name`, async () => {
    chai.expect(javaJNI.convertJNISignatureToClass(`Lcom/padre/test/SimpleJavaClass;`))
        .to.equal(`com.padre.test.SimpleJavaClass`)
  })

  it(`should throw an error when it can't convert a classes JNI signature to it's class name`, async () => {
    chai.expect(() => javaJNI.convertJNISignatureToClass(`com/padre/test/SimpleJavaClass;`))
        .to.throw(`Can't convert 'com/padre/test/SimpleJavaClass;' to a class`)
  })

  it(`should convert a classes JNI signature to it's directory and filename`, async () => {
    chai.expect(javaJNI.convertJNISignatureToDirectoryAndFilename(`Lcom/padre/test/SimpleJavaClass;`))
        .to.equal(`com/padre/test/SimpleJavaClass`)
  })
})
