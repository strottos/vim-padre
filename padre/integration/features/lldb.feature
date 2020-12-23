Feature: LLDB
    Debug with PADRE a program needing LLDB

    Scenario: Debug a basic program with LLDB using the LLDB command line
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb' debugger
        When I debug the program with PADRE
        And I give PADRE chance to start
        When I send a command 'b main' using the terminal
        Then I expect to be called with
            | function           | args                                  |
            | padre#debugger#Log | [4,"Breakpoint set.*test_prog.c.*22"] |
        When I send a command 'run' using the terminal
        Then I expect to be called with
            | function                      | args                        |
            | padre#debugger#Log            | [4,"Process \\d+ launched"] |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",22]       |
        When I send a command 's' using the terminal
        Then I expect to be called with
            | function                      | args                 |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",8] |
        When I send a command 'n' using the terminal
        Then I expect to be called with
            | function                      | args                 |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",9] |
        When I send a command 'c' using the terminal
        Then I expect to be called with
            | function           | args                                       |
            | padre#debugger#Log | [4,"Process \\d+ exited with exit code 0"] |
        When I terminate padre
        Then padre is not running

    Scenario Outline: Debug a basic program with LLDB using the PADRE interface
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler '<compiler>' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb' debugger
        When I debug the program with PADRE
        And I give PADRE chance to start
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test_prog.c","line":17}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                      |
            | padre#debugger#Log | [4,"Breakpoint set.*test_prog.c.*17"]                   |
            | padre#debugger#Log | [4,"Setting breakpoint.*test_prog.c.*17"] |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"not_exists.c","line":17}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                       |
            | padre#debugger#Log | [4,"Setting breakpoint.*not_exists.c.*17"] |
            | padre#debugger#Log | [4,"Breakpoint pending"]                   |
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                                  |
            | padre#debugger#Log            | [4,"Launching process"]               |
            | padre#debugger#Log            | [4,"Process \\d+ launched"]           |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",22]                 |
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
        When I send a request to PADRE '{"cmd":"stepOver","count":3}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",10] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                       |
            | padre#debugger#Log | [4,"Process \\d+ exited with exit code 0"] |
        When I terminate padre
        Then padre is not running

        Examples:
        | compiler     |
        | gcc -g -O0   |
        | clang -g -O0 |

    Scenario: Debug a basic program by setting multiple breakpoints immediately
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'clang -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb' debugger
        When I debug the program with PADRE
        # Purposefully don't give it a chance to start
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test_prog.c","line":8}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                     |
            | padre#debugger#Log | [4,"Breakpoint set.*test_prog.c.*8"]     |
            | padre#debugger#Log | [4,"Setting breakpoint.*test_prog.c.*8"] |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test_prog.c","line":13}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                      |
            | padre#debugger#Log | [4,"Breakpoint set.*test_prog.c.*13"]     |
            | padre#debugger#Log | [4,"Setting breakpoint.*test_prog.c.*13"] |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test_prog.c","line":17}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                      |
            | padre#debugger#Log | [4,"Breakpoint set.*test_prog.c.*17"]     |
            | padre#debugger#Log | [4,"Setting breakpoint.*test_prog.c.*17"] |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test_prog.c","line":18}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                      |
            | padre#debugger#Log | [4,"Breakpoint set.*test_prog.c.*18"]     |
            | padre#debugger#Log | [4,"Setting breakpoint.*test_prog.c.*18"] |
        When I send a request to PADRE '{"cmd":"unbreakpoint","file":"test_prog.c","line":18}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                      |
            | padre#debugger#Log | [4,"Removed breakpoint.*test_prog.c.*18"] |
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                                  |
            | padre#debugger#Log            | [4,"Launching process"]               |
            | padre#debugger#Log            | [4,"Process \\d+ launched"]           |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",22]                 |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                 |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",8] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",13] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",17] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                       |
            | padre#debugger#Log | [4,"Process \\d+ exited with exit code 0"] |
        When I terminate padre
        Then padre is not running

    Scenario: Debug a basic program with LLDB using the both the LLDB command line and the PADRE connection
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb' debugger
        When I debug the program with PADRE
        And I give PADRE chance to start
        When I send a command 'b func3' using the terminal
        Then I expect to be called with
            | function           | args                                  |
            | padre#debugger#Log | [4,"Breakpoint set.*test_prog.c.*17"] |
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                        |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",22]       |
            | padre#debugger#Log            | [4,"Launching process"]     |
            | padre#debugger#Log            | [4,"Process \\d+ launched"] |
        When I send a command 's' using the terminal
        Then I expect to be called with
            | function                      | args                 |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",8] |
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                 |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",9] |
        When I send a command 'n' using the terminal
        Then I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",17] |
        When I send a command 'n' using the terminal
        Then I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",18] |
        When I send a request to PADRE '{"cmd":"print","variable":"a"}'
        Then I receive a response '{"status":"OK","variable":"a","value":"1","type":"int"}'
        When I send a command 'c' using the terminal
        Then I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",10] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                       |
            | padre#debugger#Log | [4,"Process \\d+ exited with exit code 0"] |
        When I terminate padre
        Then padre is not running

    Scenario: PADRE error reporting when program not running
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb' debugger
        When I debug the program with PADRE
        And I give PADRE chance to start
        When I send a request to PADRE '{"cmd":"stepIn"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                     |
            | padre#debugger#Log | [3,"No process running"] |
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                     |
            | padre#debugger#Log | [3,"No process running"] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                     |
            | padre#debugger#Log | [3,"No process running"] |
        When I send a request to PADRE '{"cmd":"print","variable":"a"}'
        Then I receive a response '{"status":"ERROR","error":"Variable not found","debug":"Variable 'a' not found"}'
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                        |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",22]       |
            | padre#debugger#Log            | [4,"Launching process"]     |
            | padre#debugger#Log            | [4,"Process \\d+ launched"] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                       |
            | padre#debugger#Log | [4,"Process \\d+ exited with exit code 0"] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                     |
            | padre#debugger#Log | [3,"No process running"] |
        When I terminate padre
        Then padre is not running

    Scenario: General error handling over PADRE when program is running
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'lldb' debugger
        When I debug the program with PADRE
        And I give PADRE chance to start
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                        |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",22]       |
            | padre#debugger#Log            | [4,"Launching process"]     |
            | padre#debugger#Log            | [4,"Process \\d+ launched"] |
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"ERROR","error":"Process already running","debug":"Process with pid '\\d+' already running"}' and I expect to be called with
            | function                      | args                        |
            | padre#debugger#Log            | [4,"Launching process"]     |
        When I send a request to PADRE '{"cmd":"print","variable":"a"}'
        Then I receive a response '{"status":"ERROR","error":"Variable not found","debug":"Variable 'a' not found"}'
        When I terminate padre
        Then padre is not running

    Scenario: Debugging rust
        Given that we have a file 'test_print_variables.rs'
        And I have compiled the test program 'test_print_variables.rs' with compiler 'rustc -g' to program 'test_print_variables'
        And that we have a test program 'test_print_variables' that runs with 'lldb' debugger
        When I debug the program with PADRE
        And I give PADRE chance to start
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                              |
            | padre#debugger#Log | [3,"Stopped at unknown position"] |
            | padre#debugger#Log | [4,"Launching process"]           |
            | padre#debugger#Log | [4,"Process \\d+ launched"]       |
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
        When I send a request to PADRE '{"cmd":"print","variable":"b"}'
        Then I receive a response '{"status":"OK","variable":"b","type":"int \\*","value":"^0x[0-9a-f]*$"}'
        When I send a request to PADRE '{"cmd":"print","variable":"*b"}'
        Then I receive a response '{"status":"OK","variable":"b","type":"int","value":"42"}'
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                              |
            | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",19] |
        When I send a request to PADRE '{"cmd":"print","variable":"a"}'
        Then I receive a response '{"status":"OK","variable":"a","value":"^42.[0-9][0-9]*$","type":"float"}'
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                              |
            | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",20] |
        When I send a request to PADRE '{"cmd":"print","variable":"a"}'
        Then I receive a response '{"status":"OK","variable":"a","value":"true","type":"bool"}'
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                              |
            | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",21] |
        When I send a request to PADRE '{"cmd":"print","variable":"a"}'
        Then I receive a response '{"status":"OK","variable":"a","value":"TEST","type":"&str"}'
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                               |
            | padre#debugger#JumpToPosition | [".*test_print_variables.rs$",22] |
        When I send a request to PADRE '{"cmd":"print","variable":"b"}'
        Then I receive a response '{"status":"OK","variable":"b","type":"&str *","value":"^0x[0-9a-f]*$"}'
        When I terminate padre
        Then padre is not running

    Scenario: Test spawning process timeout
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with '`pwd`/test_files/lldb_spawn_timeout.py' debugger of type 'lldb'
        When I debug the program with PADRE
        When I send a request to PADRE '{"cmd":"setConfig","key":"ProcessSpawnTimeout","value":1}'
        Then I receive a response '{"status":"OK"}'
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"ERROR","debug":"Process spawning timed out after .*","error":"Timed out spawning process"}' and I expect to be called with
            | function                     | args                             |
            | padre#debugger#Log           | [4,"Launching process"]          |

    Scenario: Test breakpoint timeout
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with '`pwd`/test_files/lldb_breakpoint_timeout.py' debugger of type 'lldb'
        When I debug the program with PADRE
        When I send a request to PADRE '{"cmd":"setConfig","key":"BreakpointTimeout","value":1}'
        Then I receive a response '{"status":"OK"}'
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test.c","line":17}'
        Then I receive both a response '{"status":"ERROR","debug":"Breakpoint setting timed out after .*","error":"Timed out setting breakpoint"}' and I expect to be called with
            | function           | args                                                      |
            | padre#debugger#Log | [4,"Setting breakpoint in file test.c at line number 17"] |

    Scenario: Test print timeout
        Given that we have a file 'test_prog.c'
        And I have compiled the test program 'test_prog.c' with compiler 'gcc -g -O0' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with '`pwd`/test_files/lldb_print_variable_timeout.py' debugger of type 'lldb'
        When I debug the program with PADRE
        When I send a request to PADRE '{"cmd":"setConfig","key":"PrintVariableTimeout","value":1}'
        Then I receive a response '{"status":"OK"}'
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                                  |
            | padre#debugger#Log            | [4,"Launching process"]               |
            | padre#debugger#Log            | [4,"Process \\d+ launched"]           |
            | padre#debugger#JumpToPosition | [".*test_prog.c$",22]                 |
        When I send a request to PADRE '{"cmd":"print","variable":"a"}'
        Then I receive a response '{"status":"ERROR","debug":"Printing variable timed out after .*","error":"Timed out printing variable"}'
