Feature: Go Delve
    Debug with PADRE a program needing Go Delve

    Scenario: Debug a basic program with Go Delve using the Delve interface
        Given that we have a file 'test_prog.go'
        And I have compiled the test program 'test_prog.go' with compiler 'go build -gcflags=-N' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'dlv' debugger of type 'godlv'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                      | args                                    |
            | padre#debugger#JumpToPosition | [".*test_prog.go$",26]                  |
            | padre#debugger#Log            | [4,"Process launched with pid: \\d+"]   |
        When I send a command 'next' using the terminal
        Then I expect to be called with
            | function                      | args                   |
            | padre#debugger#JumpToPosition | [".*test_prog.go$",27] |
        When I terminate padre
        Then padre is not running

    Scenario: Debug a basic program with Go Delve using the PADRE interface
        Given that we have a file 'test_prog.go'
        And I have compiled the test program 'test_prog.go' with compiler 'go build -gcflags=-N' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'dlv' debugger of type 'godlv'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                      | args                                    |
            | padre#debugger#JumpToPosition | [".*test_prog.go$",26]                  |
            | padre#debugger#Log            | [4,"Process launched with pid: \\d+"]   |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"`test_dir`/test_prog.go","line":22}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                    |
            | padre#debugger#Log | [4, "Setting.*test_prog.go.*22"]        |
            | padre#debugger#Log | [4, "Breakpoint set.*test_prog.go.*22"] |
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                   |
            | padre#debugger#JumpToPosition | [".*test_prog.go$",27] |
        When I send a request to PADRE '{"cmd":"stepIn"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                   |
            | padre#debugger#JumpToPosition | [".*test_prog.go$",27] |
        When I send a request to PADRE '{"cmd":"stepIn"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                   |
            | padre#debugger#JumpToPosition | [".*test_prog.go$",20] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                   |
            | padre#debugger#JumpToPosition | [".*test_prog.go$",22] |
        When I send a request to PADRE '{"cmd":"print","variable":"b"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args        |
            | padre#debugger#Log | [4,"b=123"] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args       |
            | padre#debugger#ProcessExited  | [0,"\\d+"] |
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                                  |
            | padre#debugger#JumpToPosition | [".*test_prog.go$",26]                |
            | padre#debugger#Log            | [4,"Process launched with pid: \\d+"] |
            | padre#debugger#Log            | [4,"Launching process"]               |
        When I terminate padre
        Then padre is not running

    # TODO: Test breakpoint when file doesn't exist
