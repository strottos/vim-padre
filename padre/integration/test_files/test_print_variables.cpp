#include <string>
#include <vector>

class TestClass {
public:
    std::string a;
    const char* b;
};

struct TestStruct {
    bool a;
    int b;
    std::vector<std::string> c;
    TestClass d;
};

int main() {
    int a = 42;
    int &b = a;
    int *c = &a;
    bool d = true;
    const char* e = "TEST";
    const char* &f = e;
    std::string g = "TEST";
    std::string &h = g;
    std::string *i = &g;
    std::vector<std::string> j = {"TEST1", "TEST2", "TEST3"};
    TestStruct k;
    k.a = true;
    k.b = 42;
    k.c = {"TEST1"};
    TestClass l;
    l.a = "TEST_INNER";
    l.b = "TESTING";
    k.d = l;

    return 0;
}
