Feature: NodeJS
    Debug with PADRE for a nodeJS program

    Scenario: Debug a basic program with nodeJS
        Given that we have a test program './test_files/test_prog.js' that runs with 'node' debugger
        When I debug the program with PADRE
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test_files/test_prog.js","line":16}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function           | args                                       |
            | padre#debugger#Log | [4,"Breakpoint pending.*test_prog.js.*16"] |
        When I send a request to PADRE '{"cmd":"run"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                                   |
            | padre#debugger#Log            | [4,"Launching process"]                |
            | padre#debugger#Log            | [4,"^.* launched .*$"]   |
            | padre#debugger#JumpToPosition | [".*test_prog.js",22]                  |
            | padre#debugger#Log            | [4,"Breakpoint set.*test_prog.js.*16"] |
        When I send a request to PADRE '{"cmd":"breakpoint","file":"test_files/test_prog.js","line":19}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                                   |
            | padre#debugger#Log            | [4,"Breakpoint set.*test_prog.js.*19"] |
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.js",16] |
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.js",17] |
        When I send a request to PADRE '{"cmd":"stepOver"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.js",18] |
        When I send a request to PADRE '{"cmd":"stepIn"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.js",12] |
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.js",19] |
        When I send a request to PADRE '{"cmd":"print","variable":"b"}'
        Then I receive a response '{"status":"OK","variable":"b","value":123,"type":"number"}'
        When I send a request to PADRE '{"cmd":"continue"}'
        Then I receive both a response '{"status":"OK"}' and I expect to be called with
            | function                     | args       |
            | padre#debugger#ProcessExited | [0,"\\d+"] |
        When I terminate padre
        Then padre is not running
