function c() {
  return 'test string'
}

function d() {
  return {
    a: [1, 2, 3]
  }
}

function e() {
  return d()
}

function a(b) {
  console.log(c())
  console.log(b)
  console.log(e())
  return 456
}

console.log(a(123))
