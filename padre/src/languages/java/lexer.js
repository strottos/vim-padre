'use strict'

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

const JAVA_SEPARATORS = [
  `(`, `)`, `{`, `}`, `[`, `]`, `;`, `,`, `.`, `...`, `@`, `::`,
]

const JAVA_OPERATORS = [
  `=`, `>`, `<`, `!`, `~`, `?`, `:`, `->`,
  `==`, `>=`, `<=`, `!=`, `&&`, `||`, `++`, `--`,
  `+`, `-`, `*`, `/`, `&`, `|`, `^`, `%`, `<<`, `>>`, `>>>`,
  `+=`, `-=`, `*=`, `/=`, `&=`, `|=`, `^=`, `%=`, `<<=`, `>>=`, `>>>=`,
]

// TODO: Unicode
// const transformUnicodeCharacters = (data) => {
//   let numBackslashesFound = 0
//   let pos = 0
//
//   while (pos < data.length) {
//     if ((numBackslashesFound % 2) === 1 &&
//         _readByte(data, pos) === _readByte(Buffer.from('u'))) {
//       numBackslashesFound = 0
//       data = Buffer.concat([
//         Buffer.from(data.slice(0, pos - 1)),
//         Buffer.from(data.slice(pos + 5))
//       ])
//     }
//
//     if (_readByte(data, pos) === 0x3d) {
//       numBackslashesFound += 1
//     }
//
//     pos += 1
//   }
//
//   return data
// }
//
// TODO: Needed
// const transformToLines = (data) => {
//   let pos = 0
//   let ret = []
//   let currentPos = 0
//
//   while (currentPos < data.length) {
//     const lineTerminator = _lineTerminator(data.slice(currentPos))
//     if (lineTerminator) {
//       ret.push(data.slice(pos, currentPos))
//       currentPos += lineTerminator
//       pos = currentPos
//       continue
//     }
//     currentPos += 1
//   }
//
//   return ret
// }

const tokenize = (data) => {
  let tokens = []

  let pos = _getSizeOfSkip(data)

  while (pos < data.length) {
    let ret = _getNextToken(data.slice(pos))

    let size = ret[0]
    let token = ret[1]

    if (size === 0) {
      throw new Error(`Can't understand Java at position ${pos}`)
    }

    tokens.push(token.toString('utf-8'))
    pos += size

    // Skip any whitespace or comments to get to next token
    pos += _getSizeOfSkip(data.slice(pos))
  }

  return tokens
}

const isIdentifier = (data) => {
  return _getSizeOfIdentifierOrKeyword(Buffer.from(data)) === data.length && !isKeyword(data)
}

const isKeyword = (data) => {
  data = Buffer.from(data)
  const token = data.slice(0, _getSizeOfIdentifierOrKeyword(data)).toString('utf-8')
  if (JAVA_KEYWORDS.indexOf(token) !== -1) {
    return true
  }
  return false
}

const isIntegerLiteral = (data) => {
  return _getSizeOfNumericLiteral(Buffer.from(data)) > 0
}

const _getSizeOfSkip = (data) => {
  let pos = 0
  let size = -1

  while (size !== 0) {
    size = 0

    // Check whitespace first
    if (_isWhitespace(data.slice(pos))) {
      size = _getWhitespaceSize(data.slice(pos))
    // Check for comments
    } else if (_isEndOfLineComment(data.slice(pos))) {
      size = _getEndOfLineCommentSize(data.slice(pos))
    } else if (_isTraditionalComment(data.slice(pos))) {
      size = _getSizeOfTraditionalComment(data.slice(pos))
    }

    pos += size
  }

  return pos
}

const _getNextToken = (data) => {
  let token = null
  let size = 0

  let methods = [
    _getSizeOfIdentifierOrKeyword,
    _getSizeOfNumericLiteral,
    _getSizeOfCharacterLiteral,
    _getSizeOfStringLiteral,
    _getSizeOfSeparator,
    _getSizeOfOperator,
  ]

  for (let method of methods) {
    size = method(data)
    if (size) {
      break
    }
  }

  token = data.slice(0, size)

  return [size, token]
}

const _readByte = (data, pos) => {
  if (!pos) {
    pos = 0
  }
  if (pos >= data.length) {
    return -1
  }
  return data.readInt8(pos)
}

const _isWhitespace = (data) => {
  return [0x09, 0x0c, 0x20].indexOf(_readByte(data)) !== -1 || _lineTerminator(data) > 0
}

const _getWhitespaceSize = (data) => {
  let pos = 0
  while (_isWhitespace(data.slice(pos))) {
    pos += 1
  }
  return pos
}

const _lineTerminator = (data) => {
  return [10, 13].indexOf(_readByte(data)) !== -1
}

const _isEndOfLineComment = (data) => {
  return _readByte(data) === 47 && _readByte(data, 1) === 47
}

const _getEndOfLineCommentSize = (data) => {
  let pos = 0
  while (!_lineTerminator(data.slice(pos))) {
    pos += 1
  }
  return pos
}

const _isTraditionalComment = (data) => {
  return _readByte(data) === 47 && _readByte(data, 1) === 42
}

const _getSizeOfTraditionalComment = (data) => {
  let pos = 0
  while (_readByte(data, pos) !== 42 || _readByte(data, pos + 1) !== 47) {
    pos += 1
  }
  pos += 2
  return pos
}

const _getSizeOfIdentifierOrKeyword = (data) => {
  if (!_isJavaLetter(data)) {
    return 0
  }

  let pos = 1
  while (_isJavaLetter(data.slice(pos)) || _isDecimalDigit(data.slice(pos))) {
    pos += 1
  }
  return pos
}

// NB: Will currently recognise as numbers `0x`, `087`, `0b123ab`, `123_`.
// Shouldn't be a problem right now as to get this far it should be compilable.
const _getSizeOfNumericLiteral = (data) => {
  if ((!_isDecimalDigit(data) && data.slice(0, 1).toString('utf-8') !== '.') ||
      (data.slice(0, 1).toString('utf-8') === '.' && !_isHexDigit(data.slice(1, 2)))) {
    return 0
  }
  let pos = 1
  while (_isHexDigit(data.slice(pos)) ||
      ['x', 'b', '_', '.', 'e', 'f', 'l', 'L', '-', '+'].indexOf(
          data.slice(pos, pos + 1).toString('utf-8')) !== -1) {
    pos += 1
  }
  return pos
}

const _getSizeOfCharacterLiteral = (data) => {
  if (data.slice(0, 1).toString('utf-8') !== '\'') {
    return 0
  }

  let pos = 1
  if (data.slice(1, 2).toString('utf-8') !== '\\') {
    pos += 1
  } else {
    pos += 2
  }

  if (data.slice(pos, pos + 1).toString('utf-8') !== '\'') {
    return 0
  }

  pos += 1

  return pos
}

const _getSizeOfStringLiteral = (data) => {
  if (data.slice(0, 1).toString('utf-8') !== '"') {
    return 0
  }

  let pos = 1
  while (data.slice(pos, pos + 1).toString('utf-8') !== '"' && pos < data.length) {
    if (_readByte(data, pos) === 0x0a || _readByte(data, pos) === 0x0d) {
      return 0
    }
    if (_readByte(data, pos) === 0x5c) {
      pos += 1
    }
    pos += 1
  }

  pos += 1

  return pos
}

const _getSizeOfSeparator = (data) => {
  for (let size = 3; size >= 1; size--) {
    if (JAVA_SEPARATORS.filter(x => x.length === size).indexOf(data.slice(0, size).toString('utf-8')) !== -1) {
      return size
    }
  }

  return 0
}

const _getSizeOfOperator = (data) => {
  for (let size = 4; size >= 1; size--) {
    if (JAVA_OPERATORS.filter(x => x.length === size).indexOf(data.slice(0, size).toString('utf-8')) !== -1) {
      return size
    }
  }

  return 0
}

const _isAlpha = (data) => {
  const chr = _readByte(data)
  return (chr >= 0x41 && chr <= 0x5a) || (chr >= 0x61 && chr <= 0x7a)
}

const _isDecimalDigit = (data) => {
  const chr = _readByte(data)
  return (chr >= 0x30 && chr <= 0x39)
}

const _isHexDigit = (data) => {
  const chr = _readByte(data)
  return _isDecimalDigit(data) || (chr >= 0x41 && chr <= 0x46) || (chr >= 0x61 && chr <= 0x66)
}

const _isJavaLetter = (data) => {
  return _isAlpha(data) || [0x24, 0x5f].indexOf(_readByte(data)) !== -1
}

module.exports = {
  tokenize,
  isIdentifier,
  isKeyword,
  isIntegerLiteral,
}
