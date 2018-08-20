'use strict'

const chai = require('chai')

const javaLexer = require.main.require('src/languages/java/lexer')

const JAVA_KEYWORDS = [
  'abstract', 'continue', 'for', 'new', 'switch',
  'assert', 'default', 'if', 'package', 'synchronized',
  'boolean', 'do', 'goto', 'private', 'this',
  'break', 'double', 'implements', 'protected', 'throw',
  'byte', 'else', 'import', 'public', 'throws',
  'case', 'enum', 'instanceof', 'return', 'transient',
  'catch', 'extends', 'int', 'short', 'try',
  'char', 'final', 'interface', 'static', 'void',
  'class', 'finally', 'long', 'strictfp', 'volatile',
  'const', 'float', 'native', 'super', 'while',
  '_'
]

describe('Test Java Lexer Grammar', () => {
  //  TODO:
//  it('should transform any unicode characters appropriately', () => {
//    const data = fs.readFileSync('test/data/src/com/padre/test/UTF16Test.java')
//    const ret = javaLexer.transformUnicodeCharacters(data)
//    console.log(ret.toString('utf-8'))
//    chai.expect(ret.slice(140, 154)).to.equal(Buffer.from([
//      0x5c, 0x5c, 0x75, 0x32, 0x31, 0x32, 0x32, 0x3d,
//      0x21, 0x22, 0x22, 0x32, 0x29, 0x3b, 0x0a
//    ]))
//  })

  it('should throw an error if the Java code isn\'t understood', async () => {
    let err = null
    const data = Buffer.from([0x01])

    try {
      javaLexer.tokenize(data)
    } catch (error) {
      err = error
    }

    chai.expect(err.message).to.equal(`Can't understand Java at position 0`)
  })

  it('should transform lines into id with correct whitespace and different lines separating', () => {
    const data = Buffer.from(`test1 test2\ntest3\ttest4\ntest5\ftest6`)
    const ret = javaLexer.tokenize(data)
    chai.expect(ret.length).to.equal(6)
    chai.expect(ret).to.deep.equal([
      `test1`, `test2`, `test3`, `test4`, `test5`, `test6`,
    ])
  })

  it('should tokenize a string with comments in correctly', () => {
    const data = Buffer.from(`test1 // test comment /* testing\ntest2` +
        `/* testing //\rcomment */test3\r\n\r\ntest4\r\r\n`)
    const ret = javaLexer.tokenize(data)
    chai.expect(ret).to.deep.equal([
      `test1`, `test2`, `test3`, `test4`,
    ])
  })

  it('should tokenize a string correctly with numeric literals', () => {
    const data = Buffer.from(`test 0 2 0372 0xDada_Cafe 1996 0x00_FF__00_FF\n` +
        `0l 0777L 0x100000000L 2_147_483_648L 0xC0B0L 1e1f 2.f .3f 0f 3.14f\n` +
        `6.022137e+23f 1e1 2. .3 0.0 3.14 1e-9d 1e137`)

    const ret = javaLexer.tokenize(data)
    chai.expect(ret).to.deep.equal([
      `test`, `0`, `2`, `0372`, `0xDada_Cafe`, `1996`, `0x00_FF__00_FF`, `0l`,
      `0777L`, `0x100000000L`, `2_147_483_648L`, `0xC0B0L`, `1e1f`, `2.f`, `.3f`, `0f`,
      `3.14f`, `6.022137e+23f`, `1e1`, `2.`, `.3`, `0.0`, `3.14`, `1e-9d`, `1e137`,
    ])
  })

  it('should tokenize a string correctly with character literals', () => {
    const data = Buffer.from(`test 'c' '\\t' '\\n'`)

    const ret = javaLexer.tokenize(data)
    chai.expect(ret).to.deep.equal([
      `test`, `'c'`, `'\\t'`, `'\\n'`,
    ])
  })

  it('should error while tokenizing a string with bad character literals', () => {
    let err = null
    const data = Buffer.from(`'test'`)

    try {
      javaLexer.tokenize(data)
    } catch (error) {
      err = error
    }

    chai.expect(err.message).to.equal(`Can't understand Java at position 0`)
  })

  it('should tokenize a string correctly with string literals', () => {
    const data = Buffer.from(`test "testing\\n" "testing \\" test"`)

    const ret = javaLexer.tokenize(data)
    chai.expect(ret).to.deep.equal([
      `test`, `"testing\\n"`, `"testing \\" test"`,
    ])
  })

  it('should given an error when tokenizing a string literal with a newline', () => {
    let err = null
    const data = Buffer.from(`"test\ntest"`)

    try {
      javaLexer.tokenize(data)
    } catch (error) {
      err = error
    }

    chai.expect(err.message).to.equal(`Can't understand Java at position 0`)
  })

  it('should tokenize a string correctly with separators', () => {
    const data = Buffer.from(`.,;::...`)

    const ret = javaLexer.tokenize(data)
    chai.expect(ret).to.deep.equal([
      `.`, `,`, `;`, `::`, `...`,
    ])
  })

  // TODO: Side case of List<List<String>> doesn't have final token >>
  it('should tokenize a string correctly with operators', () => {
    const data = Buffer.from(`== != << >> > ~ ?&=`)

    const ret = javaLexer.tokenize(data)
    chai.expect(ret).to.deep.equal([
      `==`, `!=`, `<<`, `>>`, `>`, `~`, `?`, `&=`,
    ])
  })

//  it('should fail to tokenize a string correctly with incorrect integer literals', () => {
//    let data = Buffer.from(`087`)
//    let ret = javaLexer.tokenize(data)
//
//    chai.expect(ret).to.deep.equal([
//      Buffer.from(`0`),
//      Buffer.from(`87`),
//    ])
//
//    data = Buffer.from(`0xb4892`)
//    ret = javaLexer.tokenize(data)
//
//    chai.expect(ret).to.deep.equal([
//      Buffer.from(`0`),
//      Buffer.from(`xb`),
//      Buffer.from(`4892`),
//    ])
//  })

  //  TODO:
//  it('should ignore a final Ctrl-Z', () => {
//    const data = Buffer.from([0x41, 0x42, 0x1a])
//    const ret = javaLexer.tokenize(data)
//    chai.expect(ret).to.deep.equal([
//      Buffer.from(`AB`)
//    ])
//  })
})

describe('Test Java Lexer Token Checkers', () => {
  it('should be possible to identify a keyword', () => {
    for (let keyword of JAVA_KEYWORDS) {
      chai.expect(javaLexer.isKeyword(keyword)).to.be.true
    }

    chai.expect(javaLexer.isKeyword('homer')).to.be.false
    chai.expect(javaLexer.isKeyword('marge')).to.be.false
    chai.expect(javaLexer.isKeyword('bart')).to.be.false
    chai.expect(javaLexer.isKeyword('')).to.be.false
  })

  it('should be possible to check for an identifier', () => {
    for (let identifier of ['homer', 'marge', 'bart']) {
      chai.expect(javaLexer.isIdentifier(identifier)).to.be.true
    }

    for (let keyword of JAVA_KEYWORDS) {
      chai.expect(javaLexer.isIdentifier(keyword)).to.be.false
    }

    chai.expect(javaLexer.isIdentifier('1test')).to.be.false
    chai.expect(javaLexer.isIdentifier('+')).to.be.false
    chai.expect(javaLexer.isIdentifier('(')).to.be.false
    chai.expect(javaLexer.isIdentifier('test+')).to.be.false
  })

  // TODO
//  it('should be possible to check for an integer literal', () => {
//    chai.expect(javaLexer.isIntegerLiteral('bart')).to.be.false
//
//    chai.expect(javaLexer.isIntegerLiteral('0')).to.be.true
//
//    chai.expect(javaLexer.isIntegerLiteral('123')).to.be.true
//    chai.expect(javaLexer.isIntegerLiteral('123l')).to.be.true
//    chai.expect(javaLexer.isIntegerLiteral('123L')).to.be.true
//    chai.expect(javaLexer.isIntegerLiteral('123__123_123___123')).to.be.true
//    chai.expect(javaLexer.isIntegerLiteral('123__123_123___')).to.be.false
//
//    chai.expect(javaLexer.isIntegerLiteral('0x123abc')).to.be.true
//    chai.expect(javaLexer.isIntegerLiteral('0x123__ABC')).to.be.true
//    chai.expect(javaLexer.isIntegerLiteral('0x123__abc__')).to.be.false
//
//    chai.expect(javaLexer.isIntegerLiteral('0x123abc')).to.be.true
//    chai.expect(javaLexer.isIntegerLiteral('0x123__ABC')).to.be.true
//    chai.expect(javaLexer.isIntegerLiteral('0x123__abc__')).to.be.false
//
//    chai.expect(javaLexer.isIntegerLiteral('0123abc')).to.be.false
//    chai.expect(javaLexer.isIntegerLiteral('0123')).to.be.true
//    chai.expect(javaLexer.isIntegerLiteral('0x123__456')).to.be.true
//    chai.expect(javaLexer.isIntegerLiteral('0x123__456__')).to.be.false
//
//    chai.expect(javaLexer.isIntegerLiteral('0b123')).to.be.false
//    chai.expect(javaLexer.isIntegerLiteral('0b1101')).to.be.true
//    chai.expect(javaLexer.isIntegerLiteral('0x1101_1011')).to.be.true
//    chai.expect(javaLexer.isIntegerLiteral('0x1101_1011__')).to.be.false
//  })
})
