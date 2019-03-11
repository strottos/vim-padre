Feature: Basics
    Basic functionality of PADRE

    Scenario: Check we can communicate over the socket correctly
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb'
        When I debug the program with PADRE
        And I open another connection to PADRE
        And I open another connection to PADRE
        And I send a request to PADRE 'ping' on connection 0
        Then I receive a response 'OK pong' on connection 0
        When I send a request to PADRE 'ping' on connection 1
        Then I receive a response 'OK pong' on connection 1
        When I send a request to PADRE 'ping' on connection 2
        Then I receive a response 'OK pong' on connection 2
        When I send a request to PADRE 'pings' on connection 0
        Then I receive both a response 'OK' and I expect to be called on connection 0 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        And I expect to be called on connection 1 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        And I expect to be called on connection 2 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        When I send a request to PADRE 'pings' on connection 1
        Then I expect to be called on connection 0 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        Then I receive both a response 'OK' and I expect to be called on connection 1 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        And I expect to be called on connection 2 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        When I send a request to PADRE 'pings' on connection 2
        Then I expect to be called on connection 0 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        And I expect to be called on connection 1 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        Then I receive both a response 'OK' and I expect to be called on connection 2 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |

    Scenario: Check we can handle badly sent data and it will log errors appropriately.
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb'
        When I debug the program with PADRE
        And I send a raw request to PADRE 'nonsense'
        Then I expect to be called with
            | function           | args                                                              |
            | padre#debugger#Log | [2,"Must be valid JSON"]                                          |
            | padre#debugger#Log | [5,"Can't read JSON character o in line 1 at column 2: nonsense"] |
        When I send a raw request to PADRE '[1,"no end"'
        Then I expect to be called with
            | function           | args                                   |
            | padre#debugger#Log | [2,"Must be valid JSON"]               |
            | padre#debugger#Log | [5,"Can't read JSON: \\[1,\"no end\""] |
        When I send a raw request to PADRE '["a","b"]'
        Then I expect to be called with
            | function           | args                                        |
            | padre#debugger#Log | [2,"Can't read id"]                         |
            | padre#debugger#Log | [5,"Can't read id from: \\[\"a\",\"b\"\\]"] |
        When I send a raw request to PADRE '[1,2]'
        Then I expect to be called with
            | function           | args                                     |
            | padre#debugger#Log | [2,"Can't read command"]                 |
            | padre#debugger#Log | [5,"Can't read command from: \\[1,2\\]"] |
        When I send a request to PADRE 'bad request'
        Then I receive both a response 'ERROR' and I expect to be called with
            | function           | args                                                   |
            | padre#debugger#Log | [2,"Can't understand request"]                         |
            | padre#debugger#Log | [5,"Can't understand request: [\\d+,\"bad request\"]"] |
