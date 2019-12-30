Feature: Go Delve
    Debug with PADRE a program needing Go Delve

    Scenario: Debug a basic program with Go Delve using the PADRE interface
        Given that we have a file 'test_prog.go'
        And I have compiled the test program 'test_prog.go' with compiler 'go build' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'godlv' debugger
        When I debug the program with PADRE
        When I send a request to PADRE '{"cmd":"breakpoint","file":"`pwd`/test_files/test_prog.go","line":16}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                    |
            | padre#debugger#Log | [4, ".*test_prog.go.*16"]               |
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK","pid":"\\d+"}' and I expect to be called with
            | function                      | args                                    |
            | padre#debugger#BreakpointSet  | [".*test_prog.go$",27]                  |
            | padre#debugger#Log            | [4, "Breakpoint set.*test_prog.go.*16"] |
            | padre#debugger#JumpToPosition | [".*test_prog.go$",27]                  |
            | padre#debugger#Log            | [4,"Launching process"]                 |
        When I terminate padre
        Then padre is not running

    # TODO: Test breakpoint when file doesn't exist
