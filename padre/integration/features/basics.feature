Feature: Basics
    Basic functionality of PADRE

    Scenario: Check we can communicate over the socket correctly
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I open another connection to PADRE
        Then I expect to be called on connection 1 with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I open another connection to PADRE
        Then I expect to be called on connection 2 with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE '{"cmd":"ping"}' on connection 0
        Then I receive a response '{"status":"OK","ping":"pong"}' on connection 0
        When I send a request to PADRE '{
        "cmd":"ping"
        }' on connection 1
        Then I receive a response '{"status":"OK","ping":"pong"}' on connection 1
        When I send a request to PADRE '{"cmd":"ping"}' on connection 2
        Then I receive a response '{"status":"OK","ping":"pong"}' on connection 2
        When I send a request to PADRE '{"cmd":"pings"}' on connection 0
        Then I receive both a response '{"status":"OK"}' and I expect to be called on connection 0 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        And I expect to be called on connection 1 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        And I expect to be called on connection 2 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        When I send a request to PADRE '{"cmd":"pings"}' on connection 1
        Then I expect to be called on connection 0 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        Then I receive both a response '{"status":"OK"}' and I expect to be called on connection 1 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        And I expect to be called on connection 2 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        When I send a request to PADRE '{"cmd":"pings"}' on connection 2
        Then I expect to be called on connection 0 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        And I expect to be called on connection 1 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        Then I receive both a response '{"status":"OK"}' and I expect to be called on connection 2 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        When I terminate the program

    Scenario: Check we can handle terminating connections
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I terminate connection 0
        When I open another connection to PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I open another connection to PADRE
        Then I expect to be called on connection 1 with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE '{"cmd":"pings"}' on connection 0
        Then I receive both a response '{"status":"OK"}' and I expect to be called on connection 0 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        And I expect to be called on connection 1 with
            | function           | args       |
            | padre#debugger#Log | [4,"pong"] |
        When I terminate the program

    Scenario: Check we can handle badly sent data and it will log errors appropriately.
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a raw request to PADRE 'nonsense'
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
            | padre#debugger#Log | [5,"Can't read id: \\[\"a\",\"b\"\\]"] |
        When I send a raw request to PADRE '[1,2]'
        Then I expect to be called with
            | function           | args                                     |
            | padre#debugger#Log | [2,"Can't read command"]                 |
            | padre#debugger#Log | [5,"Can't read command: \\[1,2\\]"] |
        When I send a request to PADRE 'bad_request'
        Then I receive both a response '{"status":"ERROR"}' and I expect to be called with
            | function           | args                                                   |
            | padre#debugger#Log | [2,"Can't understand request"]                         |
            | padre#debugger#Log | [5,"Can't understand request: [\\d+,\"bad_request\"]"] |
        When I send a raw request to PADRE '[1,""]'
        Then I receive both a response '{"status":"ERROR"}' and I expect to be called with
            | function           | args                           |
            | padre#debugger#Log | [2,"Can't find command"]       |
            | padre#debugger#Log | [5,"Can't find command: \"\""] |
        When I send a raw request to PADRE '[1,{"cmd":"ping"}][2,{"cmd":"ping"}]'
        Then I receive a raw response '[1,{"status":"OK","ping":"pong"}][2,{"status":"OK","ping":"pong"}]'
        When I send a raw request to PADRE '[3,{"cmd":"ping"}]'
        And I send a raw request to PADRE '[4,{"cmd":"ping"}]'
        Then I receive a raw response '[3,{"status":"OK","ping":"pong"}]'
        And I receive a raw response '[4,{"status":"OK","ping":"pong"}]'
        When I terminate the program

    Scenario: Check we can handle errors setting breakpoints
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE '{"cmd":"breakpoint"}'
        Then I receive both a response '{"status":"ERROR"}' and I expect to be called with
            | function           | args                                 |
            | padre#debugger#Log | [2,"Can't read file for breakpoint"] |
            | padre#debugger#Log | [5,"Can't read file for breakpoint"] |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test.c"}'
        Then I receive both a response '{"status":"ERROR"}' and I expect to be called with
            | function           | args                                 |
            | padre#debugger#Log | [2,"Can't read line for breakpoint"] |
            | padre#debugger#Log | [5,"Can't read line for breakpoint"] |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test.c","line":"a"}'
        Then I receive both a response '{"status":"ERROR"}' and I expect to be called with
            | function           | args                                 |
            | padre#debugger#Log | [2,"Can't parse line number"] |
            | padre#debugger#Log | [5,"Can't parse line number: invalid digit found in string"] |
        When I send a request to PADRE '{"cmd":"breakpoint","line":1,"file":"test.c","bad_arg":1,"bad_arg2":2}'
        Then I receive both a response '{"status":"ERROR"}' and I expect to be called with
            | function           | args                                                               |
            | padre#debugger#Log | [2,"Bad arguments for breakpoint"]                                 |
            | padre#debugger#Log | [5,"Bad arguments for breakpoint: \\[\"bad_arg\",\"bad_arg2\"\\]"] |
        When I terminate the program

    Scenario: Check we can handle errors getting variables
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE '{"cmd":"print"}'
        Then I receive both a response '{"status":"ERROR"}' and I expect to be called with
            | function           | args                                |
            | padre#debugger#Log | [2,"Can't read variable for print"] |
            | padre#debugger#Log | [5,"Can't read variable for print"] |
        When I send a request to PADRE '{"cmd":"print","variable":"a","bad_arg":1,"bad_arg2":2}'
        Then I receive both a response '{"status":"ERROR"}' and I expect to be called with
            | function           | args                                                          |
            | padre#debugger#Log | [2,"Bad arguments for print"]                                 |
            | padre#debugger#Log | [5,"Bad arguments for print: \\[\"bad_arg\",\"bad_arg2\"\\]"] |
        When I terminate the program
