'use strict'

const fs = require('fs')

const _ = require('lodash')

const javaLexer = require('./lexer')

const getClassAtLine = async (filename, lineNum) => {
  return new Promise((resolve, reject) => {
    fs.readFile(filename, (err, data) => {
      if (err) {
        return reject(err)
      }
      resolve(_evaluateClassAtLineForData(data, lineNum))
    })
  })
}

const _evaluateClassAtLineForData = (data, lineNum) => {
  const tokens = javaLexer.tokenize(data)

  const ret = _evaluatePackageClass(tokens)
  return ret
}

const _evaluatePackageClass = (tokens) => {
  let pkg = ''
  let cls = ''

  let tknIndex = 0
  let token = tokens[0]

  let cmd = null
  let isPublic = false

  while (tknIndex < tokens.length) {
    if (cmd === 'package' && token !== ';') {
      pkg += token
    }

    if (cmd === 'class' && isPublic) {
      cls += token
      cmd = null
      isPublic = false
      break
    }

    if (token === ';') {
      isPublic = false
      cmd = null
    } else if (token === 'public') {
      isPublic = true
    } else if (token === 'package' || token === 'class') {
      cmd = token
    }

    tknIndex += 1
    token = tokens[tknIndex]
  }

  let ret = ''

  if (pkg) {
    ret = pkg + '.'
  }

  ret += cls

  return ret
}

module.exports = {
  getClassAtLine
}
