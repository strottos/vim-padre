Feature: Basics
    Basic functionality of PADRE

    Scenario: Check we can communicate over the socket correctly
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb' debugger
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
        When I send a request to PADRE '{\n "cmd":"ping"\n }' on connection 1
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
        When I terminate padre
        Then padre is not running

    Scenario: Check we can handle badly sent data and it will log errors appropriately.
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb' debugger
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a raw request to PADRE 'nonsense'
        Then I expect to be called with
            | function           | args                                |
            | padre#debugger#Log | [2,"Must be valid JSON"]            |
            | padre#debugger#Log | [5,"Can't read 'nonsense': [^ ].*"] |
        When I send a raw request to PADRE '[1,{"cmd":"no end"'
        And I send a raw request to PADRE ']'
        Then I expect to be called with
            | function           | args                                                  |
            | padre#debugger#Log | [2,"Must be valid JSON"]                              |
            | padre#debugger#Log | [5,"Can't read '\\[1,{\"cmd\":\"no end\"]': [^ ].*$"] |
        When I send a raw request to PADRE '[1,{}]'
        Then I expect to be called with
            | function           | args                                           |
            | padre#debugger#Log | [2,"Can't find command"]                       |
            | padre#debugger#Log | [5,"Can't find command '\\[1,{}\\]': [^ ].*$"] |
        When I send a raw request to PADRE '{}'
        Then I expect to be called with
            | function           | args                                    |
            | padre#debugger#Log | [2,"Can't read JSON"]                   |
            | padre#debugger#Log | [5,"Can't read '{}': Must be an array"] |
        When I send a raw request to PADRE '[]'
        Then I expect to be called with
            | function           | args                                                    |
            | padre#debugger#Log | [2,"Can't read JSON"]                                   |
            | padre#debugger#Log | [5,"Can't read '\\[\\]': Array should have 2 elements"] |
        When I send a raw request to PADRE '["a","b"]'
        Then I expect to be called with
            | function           | args                              |
            | padre#debugger#Log | [2,"Can't read id"]               |
            | padre#debugger#Log | [5,"Can't read '\"a\"': [^ ].*$"] |
        When I send a raw request to PADRE '[1]'
        Then I expect to be called with
            | function           | args                                |
            | padre#debugger#Log | [2,"Can't read JSON"]               |
            | padre#debugger#Log | [5,"Can't read '\\[1\\]': [^ ].*$"] |
        When I send a raw request to PADRE '[1,2]'
        Then I expect to be called with
            | function           | args                                  |
            | padre#debugger#Log | [2,"Can't read JSON"]                 |
            | padre#debugger#Log | [5,"Can't read '\\[1,2\\]': [^ ].*$"] |
        When I send a raw request to PADRE '[1,{"cmd":"ping"},3]'
        Then I expect to be called with
            | function           | args                                                     |
            | padre#debugger#Log | [2,"Can't read JSON"]                                    |
            | padre#debugger#Log | [5,"Can't read '\\[1,{\"cmd\":\"ping\"},3\\]': [^ ].*$"] |
        When I send a request to PADRE '{"bad":"request"}'
        Then I expect to be called with
            | function           | args                                                              |
            | padre#debugger#Log | [2,"Can't find command"]                                          |
            | padre#debugger#Log | [5,"Can't find command '\\[1,{\"bad\":\"request\"}\\]': [^ ].*$"] |
        When I send a raw request to PADRE '[1,{"cmd":{}}]'
        Then I expect to be called with
            | function           | args                                                     |
            | padre#debugger#Log | [2,"Can't find command"]                                 |
            | padre#debugger#Log | [5,"Can't find command '\\[1,{\"cmd\":{}}\\]': [^ ].*$"] |
        When I send a request to PADRE '{"cmd":"not_exists"}'
        Then I expect to be called with
            | function           | args                                 |
            | padre#debugger#Log | [2,"Command unknown"]                |
            | padre#debugger#Log | [5,"Command unknown: 'not_exists'$"] |
        #When I send a raw request to PADRE '[1,{"cmd":"ping"'
        #And I send a raw request to PADRE '[1,{"cmd":"ping"}]'
        #Then I receive a raw response '[1,{"ping":"pong","status":"OK"}]'
        #When I send a raw request to PADRE '}]'
        #Then I receive a raw response '[1,{"ping":"pong","status":"OK"}]'
        #When I send a raw request to PADRE '[1,{"cmd":"ping"}][2,{"cmd":"ping"}]'
        #Then I receive a raw response '[1,{"status":"OK","ping":"pong"}][2,{"status":"OK","ping":"pong"}]'
        #When I send a raw request to PADRE '[3,{"cmd":"ping"}]'
        #And I send a raw request to PADRE '[4,{"cmd":"ping"}]'
        #Then I receive a raw response '[3,{"status":"OK","ping":"pong"}]'
        #And I receive a raw response '[4,{"status":"OK","ping":"pong"}]'
        When I terminate padre
        Then padre is not running

    Scenario: Check we can handle errors setting breakpoints
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb' debugger
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE '{"cmd":"breakpoint"}'
        Then I expect to be called with
            | function           | args                              |
            | padre#debugger#Log | [2,"Can't understand request"]    |
            | padre#debugger#Log | [5,"Need to specify a file name"] |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test.c"}'
        Then I expect to be called with
            | function           | args                                |
            | padre#debugger#Log | [2,"Can't understand request"]      |
            | padre#debugger#Log | [5,"Need to specify a line number"] |
        When I send a request to PADRE '{"cmd":"breakpoint","file":12,"line":1}'
        Then I expect to be called with
            | function           | args                                  |
            | padre#debugger#Log | [2,"Can't read 'file' argument"]      |
            | padre#debugger#Log | [5,"Can't understand 'file': [^ ].*"] |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test.c","line":"a"}'
        Then I expect to be called with
            | function           | args                                  |
            | padre#debugger#Log | [2,"Can't read 'line' argument"]      |
            | padre#debugger#Log | [5,"Can't understand 'line': [^ ].*"] |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test.c","line":12.42}'
        Then I expect to be called with
            | function           | args                                 |
            | padre#debugger#Log | [2,"Badly specified 'line'"]         |
            | padre#debugger#Log | [5,"Badly specified 'line': [^ ].*"] |
        When I send a request to PADRE '{"cmd":"breakpoint","line":1,"file":"test.c","bad_arg":1,"bad_arg2":2}'
        Then I expect to be called with
            | function           | args                                                 |
            | padre#debugger#Log | [2,"Bad arguments"]                                  |
            | padre#debugger#Log | [5,"Bad arguments: \\[\"bad_arg\", \"bad_arg2\"\\]"] |
        When I terminate padre
        Then padre is not running

    Scenario: Check we can handle errors getting variables
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb' debugger
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE '{"cmd":"print"}'
        Then I expect to be called with
            | function           | args                                  |
            | padre#debugger#Log | [2,"Can't understand request"]        |
            | padre#debugger#Log | [5,"Need to specify a variable name"] |
        When I send a request to PADRE '{"cmd":"print","variable":1}'
        Then I expect to be called with
            | function           | args                                     |
            | padre#debugger#Log | [2,"Badly specified 'variable'"]         |
            | padre#debugger#Log | [5,"Badly specified 'variable': [^ ].*"] |
        When I send a request to PADRE '{"cmd":"print","variable":"a","bad_arg":1,"bad_arg2":2}'
        Then I expect to be called with
            | function           | args                                                 |
            | padre#debugger#Log | [2,"Bad arguments"]                                  |
            | padre#debugger#Log | [5,"Bad arguments: \\[\"bad_arg\", \"bad_arg2\"\\]"] |
        When I terminate padre
        Then padre is not running

    Scenario: Check we can get and set config items per connection
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb' debugger
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE '{"cmd":"getConfig"}'
        Then I expect to be called with
            | function           | args                           |
            | padre#debugger#Log | [2,"Can't understand request"] |
            | padre#debugger#Log | [5,"Need to specify a 'key'"]  |
        When I send a request to PADRE '{"cmd":"getConfig","key":123}'
        Then I expect to be called with
            | function           | args                                    |
            | padre#debugger#Log | [2,"Badly specified string 'key'"]      |
            | padre#debugger#Log | [5,"Badly specified string 'key': 123"] |
        When I send a request to PADRE '{"cmd":"setConfig"}'
        Then I expect to be called with
            | function           | args                           |
            | padre#debugger#Log | [2,"Can't understand request"] |
            | padre#debugger#Log | [5,"Need to specify a 'key'"]  |
        When I send a request to PADRE '{"cmd":"setConfig","key":"test"}'
        Then I expect to be called with
            | function           | args                             |
            | padre#debugger#Log | [2,"Can't understand request"]   |
            | padre#debugger#Log | [5,"Need to specify a 'value'"]  |
        When I send a request to PADRE '{"cmd":"setConfig","key":123,"value":123}'
        Then I expect to be called with
            | function           | args                                    |
            | padre#debugger#Log | [2,"Badly specified string 'key'"]      |
            | padre#debugger#Log | [5,"Badly specified string 'key': 123"] |
        When I send a request to PADRE '{"cmd":"setConfig","key":"test","value":"123"}'
        Then I expect to be called with
            | function           | args                                                  |
            | padre#debugger#Log | [2,"Badly specified 64-bit integer 'value'"]          |
            | padre#debugger#Log | [5,"Badly specified 64-bit integer 'value': \"123\""] |
        When I send a request to PADRE '{"cmd":"setConfig","key":"test","value":123123123123123123123123123123123123123}'
        Then I expect to be called with
            | function           | args                                                               |
            | padre#debugger#Log | [2,"Badly specified 64-bit integer 'value'"]          |
            | padre#debugger#Log | [5,"Badly specified 64-bit integer 'value': 1.2312312312312312e38"] |
        When I open another connection to PADRE
        Then I expect to be called on connection 1 with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE '{"cmd":"getConfig","key":"BackPressure"}' on connection 0
        Then I receive a response '{"status":"OK","value":20}' on connection 0
        When I send a request to PADRE '{"cmd":"setConfig","key":"BackPressure","value":25}' on connection 0
        Then I receive a response '{"status":"OK"}' on connection 0
        When I send a request to PADRE '{"cmd":"getConfig","key":"BackPressure"}' on connection 0
        Then I receive a response '{"status":"OK","value":25}' on connection 0
        When I send a request to PADRE '{"cmd":"getConfig","key":"BackPressure"}' on connection 1
        Then I receive a response '{"status":"OK","value":20}' on connection 1
