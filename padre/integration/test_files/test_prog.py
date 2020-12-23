def c():
    return 'test string'


def d():
    return {
        "a": [1, 2, 3]
    }


def e():
    return d()


def a(b):
    print(c())
    print(b)
    print(e())
    return 456


print(a(123))
