*** Settings ***
Documentation   Test that we can run and debug simple programs
Library         OperatingSystem
Library         RunPadre.py
Test Setup      Setup Padre


*** Variables ***
${simple_program}               simple_program
${simple_program_ext}           c
${simple_program_source_name}   ${simple_program}.${simple_program_ext}
${simple_program_body}          SEPARATOR=\n
...                             \#include <stdio.h>
...
...                             void func1();
...                             void func2();
...                             void func3();
...
...                             void func1() {
...                                 func2();
...                             }
...
...                             void func2() {
...                                 func3();
...                             }
...
...                             void func3() {
...                                 int a = 1;
...                                 printf("Test %d\\n", a);
...                             }
...
...                             int main() {
...                                 func1();
...                                 return 0;
...                             }


*** Test Cases ***
Debug simple C program
    [Documentation]     Check that we can effectively debug a simple program
    [Tags]              Smoke
    ${received} =       expect from padre       \\["call","padre#debugger#SignalPADREStarted",\\[\\]\\]
    Should Be True      ${received}[0] == True
    Should Be True      len(${received}) == 1
    send to padre       [1,"breakpoint line=16 file=${simple_program}.${simple_program_ext}"]\n
    ${received} =       expect from padre       \\[1,"OK line=16 file=${simple_program}.${simple_program_ext}"\\]
    Should Be True      ${received}[0] == True
    send to padre       [2,"run"]\n
    ${received} =       expect from padre       \\[2,"OK pid=(\\d+)"\\]
    Should Be True      ${received}[0] == True
    Should Be True      len(${received}) == 2
    ${received} =       expect from padre       \\["call","padre#debugger#JumpToPosition",\\[16,".*\\/${simple_program}.${simple_program_ext}"\\]\\]
    Should Be True      ${received}[0] == True
    send to padre       [3,"stepIn"]\n
    ${received} =       expect from padre       \\["call","padre#debugger#JumpToPosition",\\[6,".*\\/${simple_program}.${simple_program_ext}"\\]\\]
    Should Be True      ${received}[0] == True
    ${received} =       expect from padre       \\[3,"OK"\\]
    Should Be True      ${received}[0] == True
    send to padre       [4,"stepIn"]\n
    ${received} =       expect from padre       \\["call","padre#debugger#JumpToPosition",\\[9,".*\\/${simple_program}.${simple_program_ext}"\\]\\]
    Should Be True      ${received}[0] == True
    ${received} =       expect from padre       \\[4,"OK"\\]
    Should Be True      ${received}[0] == True
    send to padre       [5,"stepIn"]\n
    ${received} =       expect from padre       \\["call","padre#debugger#JumpToPosition",\\[12,".*\\/${simple_program}.${simple_program_ext}"\\]\\]
    Should Be True      ${received}[0] == True
    ${received} =       expect from padre       \\[5,"OK"\\]
    Should Be True      ${received}[0] == True
    send to padre       [6,"stepOver"]\n
    ${received} =       expect from padre       \\["call","padre#debugger#JumpToPosition",\\[13,".*\\/${simple_program}.${simple_program_ext}"\\]\\]
    Should Be True      ${received}[0] == True
    ${received} =       expect from padre       \\[6,"OK"\\]
    Should Be True      ${received}[0] == True
    send to padre       [7,"print variable=a"]\n
    ${received} =       expect from padre       \\[7,"OK variable=a value=1 type=int"\\]
    Should Be True      ${received}[0] == True
    send to padre       [8,"stepOver"]\n
    ${received} =       expect from padre       \\["call","padre#debugger#JumpToPosition",\\[14,".*\\/${simple_program}.${simple_program_ext}"\\]\\]
    Should Be True      ${received}[0] == True
    ${received} =       expect from padre       \\[8,"OK"\\]
    Should Be True      ${received}[0] == True
    send to padre       [9,"continue"]\n
    ${received} =       expect from padre       \\[9,"OK"\\]
    Should Be True      ${received}[0] == True
    ${received} =       expect from padre       \\["call","padre#debugger#ProcessExited",\\[(\\d+),\\d+\\]\\]
    Should Be True      ${received}[0] == True
    Should Be True      len(${received}) == 2
    Should Be True      ${received}[1] == '0'


*** Keywords ***
Write program
    [Arguments]     ${program_source_file}                 ${program_body}
    Create File     ${TEMPDIR}${/}${program_source_file}   ${program_body}

Compile program
    [Arguments]     ${program_source_file}      ${program_output_file}
    Run     gcc -g -o ${TEMPDIR}${/}${program_output_file} ${TEMPDIR}/${program_source_file}

Setup Padre
    Write program       ${simple_program_source_name}       ${simple_program_body}
    Compile program     ${simple_program_source_name}       ${simple_program}
    run padre       ${TEMPDIR}/${simple_program}
