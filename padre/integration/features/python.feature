Feature: Python
    Debug with PADRE a Python program

    Scenario: Debug a basic program with Python using the Python debugger command line
        Given that we have a test program './test_files/test_prog.py' that runs with 'python3' debugger of type 'python'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK","pid":"\\d+"}' and I expect to be called with
            | function                      | args                    |
            | padre#debugger#Log            | [4,"Launching process"] |
            | padre#debugger#JumpToPosition | [".*test_prog.py",3]    |
        When I send a command 'b a' using the terminal
        Then I expect to be called with
            | function                     | args                    |
            | padre#debugger#BreakpointSet | [".*test_prog.py$", 20] |
        When I send a command 's' using the terminal
        Then I expect to be called with
            | function                      | args                   |
            | padre#debugger#JumpToPosition | [".*test_prog.py$", 6] |
        When I send a command 'n' using the terminal
        Then I expect to be called with
            | function                      | args                    |
            | padre#debugger#JumpToPosition | [".*test_prog.py$", 10] |
        When I send a command 'c' using the terminal
        Then I expect to be called with
            | function                      | args                    |
            | padre#debugger#JumpToPosition | [".*test_prog.py$", 21] |
        When I send a command 'c' using the terminal
        Then I expect to be called with
            | function                      | args                   |
            | padre#debugger#ProcessExited  | [0,"\\d+"]             |
            | padre#debugger#JumpToPosition | [".*test_prog.py$", 3] |
        When I terminate padre
        Then padre is not running

    Scenario: Debug a basic program with Python using the PADRE interface
        Given that we have a test program './test_files/test_prog.py' that runs with 'python3' debugger of type 'python'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"`pwd`/test_files/test_prog.py","line":21}'
        Then I receive both a response '{"status":"PENDING"}' and I expect to be called with
            | function           | args                      |
            | padre#debugger#Log | [4, ".*test_prog.py.*21"] |
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK","pid":"\\d+"}' and I expect to be called with
            | function                      | args                    |
            | padre#debugger#Log            | [4,"Launching process"] |
            | padre#debugger#JumpToPosition | [".*test_prog.py",3]    |
            | padre#debugger#BreakpointSet  | [".*test_prog.py",21]   |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"`pwd`/test_files/test_prog.py","line":22}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                      |
            | padre#debugger#Log            | [4, ".*test_prog.py.*22"] |
            | padre#debugger#BreakpointSet  | [".*test_prog.py",22]     |
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                 |
            | padre#debugger#JumpToPosition | [".*test_prog.py",6] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.py",21] |
        When I send a request to PADRE '{"cmd":"stepIn"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                 |
            | padre#debugger#JumpToPosition | [".*test_prog.py",6] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.py",22] |
        When I send a request to PADRE '{"cmd":"print","variable":"b"}'
        Then I receive a response '{"status":"OK","variable":"b","value":"123"}'
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                   |
            | padre#debugger#ProcessExited  | [0,"\\d+"]             |
            | padre#debugger#JumpToPosition | [".*test_prog.py$", 3] |
        When I terminate padre
        Then padre is not running
