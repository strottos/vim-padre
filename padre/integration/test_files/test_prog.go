package main

func c() string {
    return "test string"
}

func d() map[string][]int {
    ret := make(map[string][]int);
    a := []int{1, 2, 3}
    ret["a"] = a

    return ret
}

func e() map[string][]int {
    return d()
}

func a(b int) int {
    print(c())
    print(b)
    print(e())
    return 456
}

func main() {
    print(a(123))
}
