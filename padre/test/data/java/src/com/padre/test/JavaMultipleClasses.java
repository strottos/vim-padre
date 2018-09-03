package com.padre.test;

public class JavaMultipleClasses {
    public static void main(String[] args) throws Exception {
        JavaTestClass1 jtc1 = new JavaTestClass1();
        jtc1.method1(1, 2, 3);
    }
}

class JavaTestClass1 {
    public void method1(int a, int b, int c) {
        System.out.printf("%d\n", method2(a, b, c));
    }

    public int method2(int a, int b, int c) {
        JavaTestClass2 jtc2 = new JavaTestClass2();
        return jtc2.method3(a, b, c);
    }
}

class JavaTestClass2 {
    public int method3(int a, int b, int c) {
        System.out.printf("%d %d %d\n", a, b, c);
        return 4;
    }
}
