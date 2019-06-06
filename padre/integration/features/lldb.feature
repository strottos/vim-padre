Feature: LLDB
    Debug with PADRE for a program needing LLDB

    Scenario: Debug a basic program with LLDB using the LLDB command line
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a command 'b main' using the terminal
        Then I expect to be called with
            | function                     | args                     |
            | padre#debugger#BreakpointSet | [".*test_prog.c$", 22]   |
        When I send a command 'run' using the terminal
        Then I expect to be called with
            | function                      | args                   |
            | padre#debugger#JumpToPosition | [".*test_prog.c$", 22] |
        When I send a command 's' using the terminal
        Then I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.c$", 8] |
        When I send a command 'n' using the terminal
        Then I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.c$", 9] |
        When I send a command 'c' using the terminal
        Then I expect to be called with
            | function                     | args       |
            | padre#debugger#ProcessExited | [0,"\\d+"] |
        When I terminate padre
        #Then padre is not running

    Scenario Outline: Debug a basic program with LLDB using the PADRE interface
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler '<compiler>' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test_prog.c","line":17}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                     | args                     |
            | padre#debugger#Log           | [4, ".*test_prog.c.*17"] |
            | padre#debugger#BreakpointSet | [".*test_prog.c$", 17]   |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"not_exists.c","line":17}'
        Then I receive both a response '{"status":"PENDING"}' and I expect to be called with
            | function           | args                     |
            | padre#debugger#Log | [4,".*not_exists.c.*17"] |
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK","pid":"\\d+"}' and I expect to be called with
            | function                      | args                    |
            | padre#debugger#BreakpointSet  | [".*test_prog.c$",22]   |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",22]   |
            | padre#debugger#Log            | [4,"Launching process"] |
        When I send a request to PADRE '{"cmd":"stepIn"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                 |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",8] |
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                 |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",9] |
        When I send a request to PADRE '{"cmd":"stepIn"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",13] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",17] |
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",18] |
        When I send a request to PADRE '{"cmd":"print","variable":"a"}'
        Then I receive a response '{"status":"OK","variable":"a","value":"1","type":"int"}'
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                     | args       |
            | padre#debugger#ProcessExited | [0,"\\d+"] |
        When I terminate padre
        #Then padre is not running

        Examples:
        | compiler     |
        | gcc -g -O0   |
        | clang -g -O0 |

    Scenario: Debug a basic program with LLDB using the both the LLDB command line and the PADRE connection
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a command 'b func3' using the terminal
        Then I expect to be called with
            | function                     | args                   |
            | padre#debugger#BreakpointSet | [".*test_prog.c$", 17] |
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK","pid":"\\d+"}' and I expect to be called with
            | function                      | args                    |
            | padre#debugger#BreakpointSet  | [".*test_prog.c$",22]   |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",22]   |
            | padre#debugger#Log            | [4,"Launching process"] |
        When I send a command 's' using the terminal
        Then I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.c$", 8] |
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                 |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",9] |
        When I send a command 'n' using the terminal
        Then I expect to be called with
            | function                      | args                   |
            | padre#debugger#JumpToPosition | [".*test_prog.c$", 17] |
        When I send a command 'n' using the terminal
        Then I expect to be called with
            | function                      | args                   |
            | padre#debugger#JumpToPosition | [".*test_prog.c$", 18] |
        When I send a request to PADRE '{"cmd":"print","variable":"a"}'
        Then I receive a response '{"status":"OK","variable":"a","value":"1","type":"int"}'
        When I send a command 'c' using the terminal
        Then I expect to be called with
            | function                      | args                   |
            | padre#debugger#JumpToPosition | [".*test_prog.c$", 10] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                     | args       |
            | padre#debugger#ProcessExited | [0,"\\d+"] |
        When I terminate padre
        #Then padre is not running

    Scenario: PADRE error reporting when program not running
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE '{"cmd":"stepIn"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                      |
            | padre#debugger#Log | [3,"program not running"] |
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                      |
            | padre#debugger#Log | [3,"program not running"] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                      |
            | padre#debugger#Log | [3,"program not running"] |
        When I send a request to PADRE '{"cmd":"print","variable":"a"}'
        Then I receive both a response '{"status":"ERROR"}' and I expect to be called with
            | function           | args                      |
            | padre#debugger#Log | [3,"program not running"] |
        When I terminate padre
        #Then padre is not running

    Scenario: General error handling over PADRE when program is running
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK","pid":"\\d+"}' and I expect to be called with
            | function                      | args                    |
            | padre#debugger#BreakpointSet  | [".*test_prog.c$",22]   |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",22]   |
            | padre#debugger#Log            | [4,"Launching process"] |
        When I send a request to PADRE '{"cmd":"print","variable":"a"}'
        Then I receive both a response '{"status":"ERROR"}' and I expect to be called with
            | function           | args                                  |
            | padre#debugger#Log | [3,"variable 'a' doesn't exist here"] |
        When I terminate padre
        #Then padre is not running

    Scenario: Printing variables in rust
        Given that we have a file 'test_print_variables.rs'
        And I have compiled the test program 'test_print_variables.rs' with compiler 'rustc -g' to program 'test_print_variables'
        And that we have a test program 'test_print_variables' that runs with 'lldb'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK","pid":"\\d+"}' and I expect to be called with
            | function           | args                              |
            | padre#debugger#Log | [3,"Stopped at unknown position"] |
            | padre#debugger#Log | [4,"Launching process"]           |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                              |
            | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",16] |
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                              |
            | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",17] |
        When I send a request to PADRE '{"cmd":"print","variable":"a"}'
        Then I receive a response '{"status":"OK","variable":"a","value":"42","type":"int"}'
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                              |
            | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",18] |
            #When I send a request to PADRE '{"cmd":"print","variable":"b"}'
            #Then I receive a response '{"status":"OK","variable":"b","deref":{"variable":"\\*b","type":"int","value":"42"},"type":"int \\*","value":"^&0x[0-9a-f]*$"}'
            #When I send a request to PADRE '{"cmd":"stepOver"}'
            #Then I receive both a response '{"status":"OK"}' and I expect to be called with
            #    | function                      | args                              |
            #    | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",19] |
            #When I send a request to PADRE '{"cmd":"print","variable":"a"}'
            #Then I receive a response '{"status":"OK","variable":"a","value":"^42.[0-9][0-9]*$","type":"float"}'
            #When I send a request to PADRE '{"cmd":"stepOver"}'
            #Then I receive both a response '{"status":"OK"}' and I expect to be called with
            #    | function                      | args                              |
            #    | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",20] |
            #When I send a request to PADRE '{"cmd":"print","variable":"a"}'
            #Then I receive a response '{"status":"OK","variable":"a","value":"true","type":"bool"}'
            #When I send a request to PADRE '{"cmd":"stepOver"}'
            #Then I receive both a response '{"status":"OK"}' and I expect to be called with
            #    | function                      | args                              |
            #    | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",21] |
            #When I send a request to PADRE '{"cmd":"print","variable":"a"}'
            #Then I receive a response '{"status":"OK","variable":"a","value":"TEST","type":"&str"}'
            #When I send a request to PADRE '{"cmd":"stepOver"}'
            #Then I receive both a response '{"status":"OK"}' and I expect to be called with
            #    | function                      | args                               |
            #    | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",22] |
            #When I send a request to PADRE '{"cmd":"print","variable":"b"}'
            #Then I receive a response '{"status":"OK","variable":"b","deref":{"variable":"\\*b","type":"&str","value":"TEST"},"type":"&str *","value":"^&0x[0-9a-f]*$"}'
