*** Settings ***
Documentation   Test that we can run and debug simple programs
Library         OperatingSystem
Library         RunPadre.py
Test Setup      Setup Padre


*** Variables ***
${simple_program}               simple_program.js
${simple_program_body}          SEPARATOR=\n
...                             function c() {
...                               return 'test string'
...                             }
...
...                             function d() {
...                               return {
...                                 a: [1, 2, 3]
...                               }
...                             }
...
...                             function e() {
...                               return d()
...                             }
...
...                             function a(b) {
...                               console.log(c())
...                               console.log(b)
...                               console.log(e())
...                               return 456
...                             }
...
...                             console.log(a(123))


*** Test Cases ***
Debug simple NodeJS program
    [Documentation]     Check that we can effectively debug a simple NodeJS program
    [Tags]              Smoke
    run padre node      ${TEMPDIR}/${simple_program}
    ${received} =       expect from padre       \\["call","padre#debugger#SignalPADREStarted",\\[\\]\\]
    Should Be True      ${received}[0] == True
    Should Be True      len(${received}) == 1
    send to padre       [1,"breakpoint file=${TEMPDIR}/${simple_program} line=5"]\n
    ${received} =       expect from padre       \\[1,"OK"\\]
    Should Be True      ${received}[0] == True
    send to padre       [2,"run"]\n
    ${received} =       expect from padre       \\[2,"OK pid=(\\d+)"\\]
    Should Be True      ${received}[0] == True
    Should Be True      len(${received}) == 2
    ${received} =       expect from padre       \\["call","padre#debugger#JumpToPosition",\\[".*${simple_program}",1\\]\\]
    Should Be True      ${received}[0] == True
    Should Be True      len(${received}) == 1
    send to padre       [3,"stepOver"]\n
    ${received} =       expect from padre       \\[3,"OK"\\]
    Should Be True      ${received}[0] == True
    ${received} =       expect from padre       \\["call","padre#debugger#JumpToPosition",\\[".*/${simple_program}",18\\]\\]
    Should Be True      ${received}[0] == True
    send to padre       [4,"stepIn"]\n
    ${received} =       expect from padre       \\[4,"OK"\\]
    Should Be True      ${received}[0] == True
    ${received} =       expect from padre       \\["call","padre#debugger#JumpToPosition",\\[".*${simple_program}",13\\]\\]
    Should Be True      ${received}[0] == True
    send to padre       [5,"stepOver"]\n
    ${received} =       expect from padre       \\[5,"OK"\\]
    Should Be True      ${received}[0] == True
    ${received} =       expect from padre       \\["call","padre#debugger#JumpToPosition",\\[".*${simple_program}",14\\]\\]
    Should Be True      ${received}[0] == True
    send to padre       [6,"print variable=b"]\n
    ${received} =       expect from padre       \\[6,"OK variable=b value=123 type=number"\\]
    Should Be True      ${received}[0] == True
    send to padre       [7,"continue"]\n
    ${received} =       expect from padre       \\[7,"OK"\\]
    Should Be True      ${received}[0] == True
    ${received} =       expect from padre       \\["call","padre#debugger#JumpToPosition",\\[".*${simple_program}",7\\]\\]
    Should Be True      ${received}[0] == True
    send to padre       [8,"continue"]\n
    ${received} =       expect from padre       \\[8,"OK"\\]
    Should Be True      ${received}[0] == True


*** Keywords ***
Write program
    [Arguments]     ${program_source_file}                 ${program_body}
    Create File     ${TEMPDIR}${/}${program_source_file}   ${program_body}

Setup Padre
    Write program       ${simple_program}       ${simple_program_body}
