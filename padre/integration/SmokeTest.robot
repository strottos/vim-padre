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
Debug simple program
    [Documentation]     Check that we can effectively debug a simple program
    [Tags]              Smoke
    send to padre       run
    ${received} =       expect from padre       pid=(\\d+)
    Should Be True      ${received}[0] == True
    Should Be True      len(${received}) == 2
    ${received} =       expect from padre       exitcode=(\\d+)
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
