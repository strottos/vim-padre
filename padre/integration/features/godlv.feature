Feature: Go Delve
    Debug with PADRE a program needing Go Delve

    Scenario: Debug a basic program with Go Delve using the Delve interface
        Given that we have a file 'test_prog.go'
        And I have compiled the test program 'test_prog.go' with compiler 'go build -o test_prog' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'dlv' debugger of type 'godlv'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                      | args                                    |
            | padre#debugger#Log            | [4, "Breakpoint set.*test_prog.go.*26"] |
            | padre#debugger#JumpToPosition | [".*test_prog.go$",26]                  |
        When I terminate padre
        Then padre is not running

    Scenario: Debug a basic program with Go Delve using the PADRE interface
        Given that we have a file 'test_prog.go'
        And I have compiled the test program 'test_prog.go' with compiler 'go build -o test_prog' to program 'test_prog'
        And that we have a test program 'test_prog' that runs with 'dlv' debugger of type 'godlv'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                      | args                                    |
            | padre#debugger#Log            | [4, "Breakpoint set.*test_prog.go.*26"] |
            | padre#debugger#JumpToPosition | [".*test_prog.go$",26]                  |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"`pwd`/test_prog.go","line":16}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                    |
            | padre#debugger#Log | [4, ".*test_prog.go.*16"]               |
        When I terminate padre
        Then padre is not running

    # TODO: Test breakpoint when file doesn't exist
