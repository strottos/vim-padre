Feature: Python
    Debug with PADRE a Python program

    Scenario: Debug a basic program with Python using the Python debugger command line
        Given that we have a file 'test_prog.py'
        And that we have a test program 'test_prog.py' that runs with 'python3' debugger of type 'python'
        When I debug the program with PADRE
        And I give PADRE chance to start
        Then I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.py", 1] |
        When I send a command 'b a' using the terminal
        Then I expect to be called with
            | function           | args                                   |
            | padre#debugger#Log | [4,"Breakpoint set.*test_prog.py.*15"] |
        When I send a command 's' using the terminal
        Then I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.py$",5] |
        When I send a command 'n' using the terminal
        Then I expect to be called with
            | function                      | args                   |
            | padre#debugger#JumpToPosition | [".*test_prog.py$",11] |
        When I send a command 'c' using the terminal
        Then I expect to be called with
            | function                      | args                   |
            | padre#debugger#JumpToPosition | [".*test_prog.py$",16] |
        When I send a command 'c' using the terminal
        Then I expect to be called with
            | function                      | args                                       |
            | padre#debugger#Log            | [4,"Process \\d+ exited with exit code 0"] |
            | padre#debugger#JumpToPosition | [".*test_prog.py$",1]                      |
        When I terminate padre
        Then padre is not running

    Scenario: Debug a basic program with Python using the PADRE interface
        Given that we have a file 'test_prog.py'
        And that we have only a test program 'test_prog.py'
        When I debug the program with PADRE
        And I give PADRE chance to start
        Then I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.py", 1] |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"`test_dir`/test_prog.py","line":16}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                       |
            | padre#debugger#Log | [4,"Setting breakpoint.*test_prog.py.*16"] |
            | padre#debugger#Log | [4,"Breakpoint set.*test_prog.py.*16"]     |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"`test_dir`/test_prog.py","line":18}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                       |
            | padre#debugger#Log | [4,"Setting breakpoint.*test_prog.py.*18"] |
            | padre#debugger#Log | [4,"Breakpoint set.*test_prog.py.*18"]     |
        When I send a request to PADRE '{"cmd":"unbreakpoint","file":"`test_dir`/test_prog.py","line":18}'
        Then I receive a response '{"status":"OK"}'
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                                   |
            | padre#debugger#Log            | [4,"Restarting process"]               |
            | padre#debugger#JumpToPosition | [".*test_prog.py",1]                   |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"`test_dir`/test_prog.py","line":17}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                       |
            | padre#debugger#Log | [4,"Setting breakpoint.*test_prog.py.*17"] |
            | padre#debugger#Log | [4,"Breakpoint set.*test_prog.py.*17"]     |
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                 |
            | padre#debugger#JumpToPosition | [".*test_prog.py",5] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.py",16] |
        When I send a request to PADRE '{"cmd":"stepIn"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                 |
            | padre#debugger#JumpToPosition | [".*test_prog.py",1] |
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                 |
            | padre#debugger#JumpToPosition | [".*test_prog.py",2] |
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                           |
            | padre#debugger#JumpToPosition | [".*test_prog.py",2]           |
            | padre#debugger#Log            | [4,"Returning.*'test string'"] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.py",17] |
        When I send a request to PADRE '{"cmd":"print","variable":"b"}'
        Then I receive a response '{"status":"OK","variable":"b","value":"123","type":"<class 'int'>"}'
        When I send a request to PADRE '{"cmd":"stepOver","count":3}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.py",19] |
            | padre#debugger#Log            | [4,"Returning.*"]     |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                                       |
            | padre#debugger#JumpToPosition | [".*test_prog.py",1]                       |
            | padre#debugger#Log            | [4,"Process \\d+ exited with exit code 0"] |
        When I terminate padre
        Then padre is not running
