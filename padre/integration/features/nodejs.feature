Feature: NodeJS
    Debug with PADRE for a nodeJS program

    Scenario: Debug a basic program with nodeJS
        Given that we have a test program './test_files/test_prog.js' that runs with 'node'
        When I debug the program with PADRE
        Then I expect to be called with
            | function                          | args |
            | padre#debugger#SignalPADREStarted | []   |
        When I send a request to PADRE 'breakpoint file=test_files/test_prog.js line=16'
        Then I receive a response 'PENDING'
        When I send a request to PADRE 'run'
        Then I receive both a response 'OK pid=\d+' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.js",22] |
            | padre#debugger#BreakpointSet  | [".*test_prog.js",16] |
        When I send a request to PADRE 'breakpoint file=test_files/test_prog.js line=19'
        Then I receive both a response 'OK' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#BreakpointSet  | [".*test_prog.js",19] |
        When I send a request to PADRE 'stepOver'
        Then I receive both a response 'OK' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.js",16] |
        When I send a request to PADRE 'stepOver'
        Then I receive both a response 'OK' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.js",17] |
        When I send a request to PADRE 'stepOver'
        Then I receive both a response 'OK' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.js",18] |
        When I send a request to PADRE 'stepIn'
        Then I receive both a response 'OK' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.js",12] |
        When I send a request to PADRE 'continue'
        Then I receive both a response 'OK' and I expect to be called with
            | function                      | args                  |
            | padre#debugger#JumpToPosition | [".*test_prog.js",19] |
        When I send a request to PADRE 'print variable=b'
        Then I receive a response 'OK variable=b value=123 type=number'
        When I send a request to PADRE 'continue'
        Then I receive both a response 'OK' and I expect to be called with
            | function                     | args       |
            | padre#debugger#ProcessExited | [0,"\\d+"] |
