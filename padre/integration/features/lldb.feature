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
        When I send a command 'r run' using the terminal
        Then I expect to be called with
            | function                      | args                   |
            | padre#debugger#JumpToPosition | [".*test_prog.c$", 22] |

#    Scenario Outline: Debug a basic program with LLDB using the PADRE interface
#        Given that we have a file 'test_prog.c'
#        And I have compiled the test program 'test_prog.c' with compiler '<compiler>' to program 'test_prog'
#        And that we have a test program 'test_prog' that runs with 'lldb'
#        When I debug the program with PADRE
#        Then I expect to be called with
#            | function                          | args |
#            | padre#debugger#SignalPADREStarted | []   |
#        When I send a request to PADRE 'breakpoint file=test_prog.c line=17'
#        Then I receive a response 'OK'
#        When I send a request to PADRE 'run'
#        Then I receive both a response 'OK pid=\d+' and I expect to be called with
#            | function                      | args                 |
#            | padre#debugger#JumpToPosition | [".*test_prog.c",22] |
#        When I send a request to PADRE 'stepIn'
#        Then I receive both a response 'OK' and I expect to be called with
#            | function                      | args                |
#            | padre#debugger#JumpToPosition | [".*test_prog.c",8] |
#        When I send a request to PADRE 'stepOver'
#        Then I receive both a response 'OK' and I expect to be called with
#            | function                      | args                |
#            | padre#debugger#JumpToPosition | [".*test_prog.c",9] |
#        When I send a request to PADRE 'stepIn'
#        Then I receive both a response 'OK' and I expect to be called with
#            | function                      | args                 |
#            | padre#debugger#JumpToPosition | [".*test_prog.c",13] |
#        When I send a request to PADRE 'continue'
#        Then I receive both a response 'OK' and I expect to be called with
#            | function                      | args                 |
#            | padre#debugger#JumpToPosition | [".*test_prog.c",17] |
#        When I send a request to PADRE 'stepOver'
#        Then I receive both a response 'OK' and I expect to be called with
#            | function                      | args                 |
#            | padre#debugger#JumpToPosition | [".*test_prog.c",18] |
#        When I send a request to PADRE 'print variable=a'
#        Then I receive a response 'OK variable=a value=1 type=number'
#        When I send a request to PADRE 'continue'
#        Then I receive both a response 'OK' and I expect to be called with
#            | function                     | args       |
#            | padre#debugger#ProcessExited | [0,"\\d+"] |
#
#        Examples:
#        | compiler     |
#        | gcc -g -O0   |
#        | clang -g -O0 |
