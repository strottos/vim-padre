'use strict'

const covertClassToJNISignature = (data) => {
  return 'L' + data.replace(/\./g, '/') + ';'
}

module.exports = {
  covertClassToJNISignature
}
