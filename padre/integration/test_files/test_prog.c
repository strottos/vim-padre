#include <stdio.h>

void func1();
void func2();
void func3();

void func1() {
    printf("Test 1\n");
    func2();
}

void func2() {
    func3();
}

void func3() {
    int a = 1;
    printf("Test %d\n", a);
}

int main() {
    func1();
    return 0;
}
