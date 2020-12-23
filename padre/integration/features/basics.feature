Feature: Basics
    Basic functionality of PADRE

    Scenario: Check we can communicate over the socket correctly
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb' debugger
        When I debug the program with PADRE
        And I give PADRE chance to start
        When I open another connection to PADRE
        When I open another connection to PADRE
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
        And I give PADRE chance to start
        When I send a raw request to PADRE 'nonsense'
        Then I receive a raw response containing the following entries
            | entry                                                              |
            | ^\[0,\{                                                            |
            | \}]$                                                               |
            | "debug":"Can't read 'nonsense': expected ident at line 1 column 2" |
            | "error":"Must be valid JSON"                                       |
            | "status":"ERROR"                                                   |
        When I send a raw request to PADRE '[1,{"cmd":"no end"'
        And I send a raw request to PADRE ']'
        # TODO: Fix the following so it can read the ID in some cases
        Then I receive a raw response containing the following entries
            | entry                                                |
            | ^\[0,\{                                              |
            | \}]$                                                 |
            | "debug":"Can't read '\[1,\{\\"cmd\\":\\"no end\\"\]' |
            | "error":"Must be valid JSON"                         |
            | "status":"ERROR"                                     |
        When I send a raw request to PADRE '[1,{}]'
        Then I receive a raw response containing the following entries
            | entry                                    |
            | ^\[1,\{                                  |
            | \}]$                                     |
            | "debug":"Can't find command '\[1,\{\}\]' |
            | "error":"Can't find command"             |
            | "status":"ERROR"                         |
        When I send a raw request to PADRE '{}'
        Then I receive a raw response containing the following entries
            | entry                                |
            | ^\[0,\{                              |
            | \}]$                                 |
            | "debug":"Can't read '\{\}'           |
            | "error":"Not an array, invalid JSON" |
            | "status":"ERROR"                     |
        When I send a raw request to PADRE '[]'
        Then I receive a raw response containing the following entries
            | entry                                                     |
            | ^\[0,\{                                                   |
            | \}]$                                                      |
            | "debug":"Can't read '\[\]': Array should have 2 elements" |
            | "error":"Array must have 2 elements, invalid JSON"        |
            | "status":"ERROR"                                          |
        When I send a raw request to PADRE '["a","b"]'
        Then I receive a raw response containing the following entries
            | entry                                                                      |
            | ^\[0,\{                                                                    |
            | \}]$                                                                       |
            | "debug":"Can't read '\\"a\\"': invalid type: string \\"a\\", expected u64" |
            | "error":"Can't read id"                                                    |
            | "status":"ERROR"                                                           |
        When I send a raw request to PADRE '[1]'
        Then I receive a raw response containing the following entries
            | entry                                                      |
            | ^\[1,\{                                                    |
            | \}]$                                                       |
            | "debug":"Can't read '\[1\]': Array should have 2 elements" |
            | "error":"Array must have 2 elements, invalid JSON"         |
            | "status":"ERROR"                                           |
        When I send a raw request to PADRE '[1,2]'
        Then I receive a raw response containing the following entries
            | entry                                                    |
            | ^\[1,\{                                                  |
            | \}]$                                                     |
            | "debug":"Can't read '\[1,2\]': invalid type: integer `2` |
            | "error":"Can't read 2nd argument as dictionary"          |
            | "status":"ERROR"                                         |
        When I send a raw request to PADRE '[1,{"cmd":"ping"},3]'
        Then I receive a raw response containing the following entries
            | entry                                                                                |
            | ^\[1,\{                                                                              |
            | \}]$                                                                                 |
            | "debug":"Can't read '\[1,\{\\"cmd\\":\\"ping\\"\},3\]': Array should have 2 elements |
            | "error":"Array must have 2 elements, invalid JSON"                                   |
            | "status":"ERROR"                                                                     |
        When I send a request to PADRE '{"bad":"request"}'
        Then I receive a raw response containing the following entries
            | entry                                                                                      |
            | ^\[1,\{                                                                                    |
            | \}]$                                                                                       |
            | "debug":"Can't find command '\[1,\{\\"bad\\":\\"request\\"\}\]': Need a cmd in 2nd object" |
            | "error":"Can't find command"                                                               |
            | "status":"ERROR"                                                                           |
        When I send a raw request to PADRE '[1,{"cmd":{}}]'
        Then I receive a raw response containing the following entries
            | entry                                                                                         |
            | ^\[1,\{                                                                                       |
            | \}]$                                                                                          |
            | "debug":"Can't find command '\[1,\{\\"cmd\\":\{\}\}\]': invalid type: map, expected a string" |
            | "error":"Can't find command"                                                                  |
            | "status":"ERROR"                                                                              |
        When I send a request to PADRE '{"cmd":"not_exists"}'
        Then I receive a raw response containing the following entries
            | entry                                   |
            | ^\[2,\{                                 |
            | \}]$                                    |
            | "debug":"Command unknown: 'not_exists'" |
            | "error":"Command unknown"               |
            | "status":"ERROR"                        |
        When I terminate padre
        Then padre is not running

    Scenario: Check we can handle errors setting breakpoints
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb' debugger
        When I debug the program with PADRE
        And I give PADRE chance to start
        When I send a request to PADRE '{"cmd":"breakpoint"}'
        Then I receive a raw response containing the following entries
            | entry                                 |
            | ^\[1,\{                               |
            | \}]$                                  |
            | "debug":"Need to specify a file name" |
            | "error":"Can't understand request"    |
            | "status":"ERROR"                      |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test.c"}'
        Then I receive a raw response containing the following entries
            | entry                                   |
            | ^\[2,\{                                 |
            | \}]$                                    |
            | "debug":"Need to specify a line number" |
            | "error":"Can't understand request"      |
            | "status":"ERROR"                        |
        When I send a request to PADRE '{"cmd":"breakpoint","file":12,"line":1}'
        Then I receive a raw response containing the following entries
            | entry                                  |
            | ^\[3,\{                                |
            | \}]$                                   |
            | "debug":"Can\'t understand 'file': 12" |
            | "error":"Can't read 'file' argument"   |
            | "status":"ERROR"                       |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test.c","line":"a"}'
        Then I receive a raw response containing the following entries
            | entry                                      |
            | ^\[4,\{                                    |
            | \}]$                                       |
            | "debug":"Can't understand 'line': \\"a\\"" |
            | "error":"Can\'t read 'line' argument"      |
            | "status":"ERROR"                           |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test.c","line":12.42}'
        Then I receive a raw response containing the following entries
            | entry                                   |
            | ^\[5,\{                                 |
            | \}]$                                    |
            | "debug":"Badly specified 'line': 12.42" |
            | "error":"Badly specified 'line'"        |
            | "status":"ERROR"                        |
        When I send a request to PADRE '{"cmd":"breakpoint","line":1,"file":"test.c","bad_arg":1,"bad_arg2":2}'
        Then I receive a raw response containing the following entries
            | entry                                                      |
            | ^\[6,\{                                                    |
            | \}]$                                                       |
            | "debug":"Bad arguments: \[\\"bad_arg\\", \\"bad_arg2\\"\]" |
            | "error":"Bad arguments"                                    |
            | "status":"ERROR"                                           |
        When I terminate padre
        Then padre is not running

    Scenario: Check we can handle errors getting variables
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb' debugger
        When I debug the program with PADRE
        And I give PADRE chance to start
        When I send a request to PADRE '{"cmd":"print"}'
        Then I receive a raw response containing the following entries
            | entry                                     |
            | ^\[1,\{                                   |
            | \}]$                                      |
            | "debug":"Need to specify a variable name" |
            | "error":"Can't understand request"        |
            | "status":"ERROR"                          |
        When I send a request to PADRE '{"cmd":"print","variable":1}'
        Then I receive a raw response containing the following entries
            | entry                                   |
            | ^\[2,\{                                 |
            | \}]$                                    |
            | "debug":"Badly specified 'variable': 1" |
            | "error":"Badly specified 'variable'"    |
            | "status":"ERROR"                        |
        When I send a request to PADRE '{"cmd":"print","variable":"a","bad_arg":1,"bad_arg2":2}'
        Then I receive a raw response containing the following entries
            | entry                                                      |
            | ^\[3,\{                                                    |
            | \}]$                                                       |
            | "debug":"Bad arguments: \[\\"bad_arg\\", \\"bad_arg2\\"\]" |
            | "error":"Bad arguments"                                    |
            | "status":"ERROR"                                           |
        When I terminate padre
        Then padre is not running

    Scenario: Check we can get and set config items per connection
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb' debugger
        When I debug the program with PADRE
        And I give PADRE chance to start
        When I send a request to PADRE '{"cmd":"getConfig"}'
        Then I receive a raw response containing the following entries
            | entry                              |
            | ^\[1,\{                            |
            | \}]$                               |
            | "debug":"Need to specify a 'key'"  |
            | "error":"Can't understand request" |
            | "status":"ERROR"                   |
        When I send a request to PADRE '{"cmd":"getConfig","key":123}'
        Then I receive a raw response containing the following entries
            | entry                                       |
            | ^\[2,\{                                     |
            | \}]$                                        |
            | "debug":"Badly specified string 'key': 123" |
            | "error":"Badly specified string 'key'"      |
            | "status":"ERROR"                            |
        When I send a request to PADRE '{"cmd":"setConfig"}'
        Then I receive a raw response containing the following entries
            | entry                              |
            | ^\[3,\{                            |
            | \}]$                               |
            | "debug":"Need to specify a 'key'"  |
            | "error":"Can't understand request" |
            | "status":"ERROR"                   |
        When I send a request to PADRE '{"cmd":"setConfig","key":"test"}'
        Then I receive a raw response containing the following entries
            | entry                               |
            | ^\[4,\{                             |
            | \}]$                                |
            | "debug":"Need to specify a 'value'" |
            | "error":"Can't understand request"  |
            | "status":"ERROR"                    |
        When I send a request to PADRE '{"cmd":"setConfig","key":123,"value":123}'
        Then I receive a raw response containing the following entries
            | entry                                       |
            | ^\[5,\{                                     |
            | \}]$                                        |
            | "debug":"Badly specified string 'key': 123" |
            | "error":"Badly specified string 'key'"      |
            | "status":"ERROR"                            |
        When I send a request to PADRE '{"cmd":"setConfig","key":"test","value":"123"}'
        Then I receive a raw response containing the following entries
            | entry                                                       |
            | ^\[6,\{                                                     |
            | \}]$                                                        |
            | "debug":"Badly specified 64-bit integer 'value': \\"123\\"" |
            | "error":"Badly specified 64-bit integer 'value'"            |
            | "status":"ERROR"                                            |
        When I send a request to PADRE '{"cmd":"setConfig","key":"test","value":123123123123123123123123123123123123123}'
        Then I receive a raw response containing the following entries
            | entry                                                                   |
            | ^\[7,\{                                                                 |
            | \}]$                                                                    |
            | "debug":"Badly specified 64-bit integer 'value': 1.2312312312312312e38" |
            | "error":"Badly specified 64-bit integer 'value'"                        |
            | "status":"ERROR"                                                        |
        When I open another connection to PADRE
        When I send a request to PADRE '{"cmd":"getConfig","key":"BackPressure"}' on connection 0
        Then I receive a response '{"status":"OK","value":20}' on connection 0
        When I send a request to PADRE '{"cmd":"setConfig","key":"BackPressure","value":25}' on connection 0
        Then I receive a response '{"status":"OK"}' on connection 0
        When I send a request to PADRE '{"cmd":"getConfig","key":"BackPressure"}' on connection 0
        Then I receive a response '{"status":"OK","value":25}' on connection 0
        When I send a request to PADRE '{"cmd":"getConfig","key":"BackPressure"}' on connection 1
        Then I receive a response '{"status":"OK","value":20}' on connection 1
        When I terminate padre
        Then padre is not running
