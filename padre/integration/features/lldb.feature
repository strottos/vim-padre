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
            | function                     | args                   |
            | padre#debugger#BreakpointSet | [".*test_prog.c$", 22] |
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
        When I terminate the program
    
    Scenario Outline: Debug a basic program with LLDB using the PADRE interface
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler '<compiler>' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE 'breakpoint file=test_prog.c line=17'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                     | args                     |
            | padre#debugger#Log           | [4, ".*test_prog.c.*17"] |
            | padre#debugger#BreakpointSet | [".*test_prog.c$", 17]   |
        When I send a request to PADRE 'breakpoint file=not_exists.c line=17'
        Then I receive both a response '{"status":"PENDING"}' and I expect to be called with
            | function           | args                     |
            | padre#debugger#Log | [4,".*not_exists.c.*17"] |
        When I send a request to PADRE 'run'
        Then I receive both a response '{"status":"OK","pid":"\\d+"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#BreakpointSet  | [".*test_prog.c$",22] |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",22] |
        When I send a request to PADRE 'stepIn'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                 |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",8] |
        When I send a request to PADRE 'stepOver'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                 |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",9] |
        When I send a request to PADRE 'stepIn'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",13] |
        When I send a request to PADRE 'continue'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",17] |
        When I send a request to PADRE 'stepOver'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",18] |
        When I send a request to PADRE 'print variable=a'
        Then I receive a response '{"status":"OK","variable":"a","value":"1","type":"int"}'
        When I send a request to PADRE 'continue'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                     | args       |
            | padre#debugger#ProcessExited | [0,"\\d+"] |
        When I terminate the program
    
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
        When I send a request to PADRE 'run'
        Then I receive both a response '{"status":"OK","pid":"\\d+"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#BreakpointSet  | [".*test_prog.c$",22] |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",22] |
        When I send a command 's' using the terminal
        Then I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.c$", 8] |
        When I send a request to PADRE 'stepOver'
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
        When I send a request to PADRE 'print variable=a'
        Then I receive a response '{"status":"OK","variable":"a","value":"1","type":"int"}'
        When I send a command 'c' using the terminal
        Then I expect to be called with
            | function                      | args                   |
            | padre#debugger#JumpToPosition | [".*test_prog.c$", 10] |
        When I send a request to PADRE 'continue'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                     | args       |
            | padre#debugger#ProcessExited | [0,"\\d+"] |
        When I terminate the program
    
    Scenario: Error over PADRE when program not running
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE 'stepIn'
        Then I receive both a response '{"status":"ERROR"}' and I expect to be called with
            | function           | args                      |
            | padre#debugger#Log | [3,"program not running"] |
        When I send a request to PADRE 'stepOver'
        Then I receive both a response '{"status":"ERROR"}' and I expect to be called with
            | function           | args                      |
            | padre#debugger#Log | [3,"program not running"] |
        When I send a request to PADRE 'continue'
        Then I receive both a response '{"status":"ERROR"}' and I expect to be called with
            | function           | args                      |
            | padre#debugger#Log | [3,"program not running"] |
        When I send a request to PADRE 'print variable=a'
        Then I receive both a response '{"status":"ERROR"}' and I expect to be called with
            | function           | args                      |
            | padre#debugger#Log | [3,"program not running"] |
        When I terminate the program
    
    Scenario: General error handling over PADRE when program is running
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE 'run'
        Then I receive both a response '{"status":"OK","pid":"\\d+"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#BreakpointSet  | [".*test_prog.c$",22] |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",22] |
        When I send a request to PADRE 'print variable=a'
        Then I receive both a response '{"status":"ERROR"}' and I expect to be called with
            | function           | args                                  |
            | padre#debugger#Log | [3,"variable 'a' doesn't exist here"] |
        When I terminate the program

    Scenario: Printing variables in rust
        Given that we have a file 'test_print_variables.rs'
        And I have compiled the test program 'test_print_variables.rs' with compiler 'rustc -g' to program 'test_print_variables'
        And that we have a test program 'test_print_variables' that runs with 'rust-lldb'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE 'run'
        Then I receive both a response '{"status":"OK","pid":"\\d+"}' and I expect to be called with
            | function           | args                              |
            | padre#debugger#Log | [3,"Stopped at unknown position"] |
        When I send a request to PADRE 'continue'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                             |
            | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",4] |
        When I send a request to PADRE 'stepOver'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                             |
            | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",5] |
        When I send a request to PADRE 'print variable=a'
        Then I receive a response '{"status":"OK","variable":"a","value":"42","type":"int"}'
        When I send a request to PADRE 'stepOver'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                             |
            | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",6] |
        When I send a request to PADRE 'print variable=b'
        Then I receive a response '{"status":"OK","variable":"b","deref":{"variable":"\\*b","type":"int","value":"42"},"type":"int \\*"}'
        When I send a request to PADRE 'stepOver'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                             |
            | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",7] |
        When I send a request to PADRE 'print variable=a'
        Then I receive a response '{"status":"OK","variable":"a","value":"^42.[0-9][0-9]*$","type":"float"}'
        When I send a request to PADRE 'stepOver'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                             |
            | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",8] |
        When I send a request to PADRE 'print variable=a'
        Then I receive a response '{"status":"OK","variable":"a","value":"true","type":"bool"}'
        When I send a request to PADRE 'stepOver'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                             |
            | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",9] |
        When I send a request to PADRE 'print variable=a'
        Then I receive a response '{"status":"OK","variable":"a","value":"TEST","type":"&str"}'
        When I send a request to PADRE 'stepOver'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                              |
            | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",10] |
        When I send a request to PADRE 'print variable=b'
        Then I receive a response '{"status":"OK","variable":"b","deref":{"variable":"\\*b","type":"&str","value":"TEST"},"type":"&str *"}'
