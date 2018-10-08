'use strict'

const fs = require('fs')

const _ = require('lodash')

const javaLexer = require('./lexer')

const getPositionDataAtLine = async (filename, lineNum) => {
  return new Promise((resolve, reject) => {
    fs.readFile(filename, (err, data) => {
      if (err) {
        return reject(err)
      }
      resolve(_evaluate(data, lineNum))
    })
  })
}

const _evaluate = (data, lineNum) => {
  const tokens = javaLexer.getTokensWithLines(data)

  let pkg = ''
  let cls = null
  let cmd = null
  let clsStart = 1
  let clsEnd = Math.max.apply([], tokens.map(x => x.lineNum))
  let blockNum = null

  for (let i = 0; i < tokens.length; i++) {
    let token = tokens[i]

    if (cmd === 'package' && token.token !== ';') {
      pkg += token.token
    }

    if (cmd === 'class' && !cls) {
      if (token.lineNum > lineNum) {
        break
      }
      cls = token.token
      cmd = null
      clsStart = token.lineNum
      blockNum = 0
    }

    if (token.token === ';') {
      cmd = null
    } else if (token.token === 'package' || token.token === 'class') {
      cmd = token.token
    } else if (token.token === '{' && _.isNumber(blockNum)) {
      blockNum += 1
    } else if (token.token === '}' && _.isNumber(blockNum)) {
      blockNum -= 1
      if (blockNum === 0) {
        clsEnd = token.lineNum
        blockNum = null
      }
    }
  }

  const method = _evaluateMethod(tokens.filter(x => x.lineNum >= clsStart && x.lineNum <= clsEnd), lineNum)

  if (pkg) {
    cls = pkg + '.' + cls
  }

  return [cls, method]
}

const _evaluateMethod = (tokens, lineNum) => {
  let methods = []
  let analyseMethodName = false

  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i]

    if (['public', 'private', 'protected', 'package'].indexOf(token.token) !== -1) {
      analyseMethodName = true
    }

    if (!analyseMethodName) {
      continue
    }

    if (javaLexer.isIdentifier(token.token) && tokens[i + 1].token === '(') {
      let j
      for (j = i + 2; j < tokens.length; j++) {
        if (tokens[j].token === ')') {
          break
        }
      }
      j++
      for (; j < tokens.length; j++) {
        if (tokens[j].token === '{') {
          methods.push(token)
          analyseMethodName = false
          break
        } else if (tokens[j].token === ';') {
          break
        }
      }
    }
  }

  return _.last(methods.filter(x => x.lineNum <= lineNum).map(x => x.token))
}

module.exports = {
  getPositionDataAtLine
}
