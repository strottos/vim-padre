#!/usr/bin/env python3
"""
Long timeout on LLDB mock
"""
import argparse
import re
import sys
import time

TIMEOUT = 10


def main():
    """
    Main program
    """
    parser = argparse.ArgumentParser()
    parser.add_argument('prog_args', type=str, nargs='+',
                        help='lldb arguments for program to run')
    args = parser.parse_args()
    prog = args.prog_args[0]
    sys.stdout.write('(lldb) target create "{}"\n'.format(prog))
    sys.stdout.write("Current executable set to '{}' (x86_64).\n".format(prog))
    sys.stdout.flush()

    for line in sys.stdin:
        line = line.rstrip()
        sys.stdout.write('(lldb) {}\n'.format(line))
        if re.match('settings.*', line):
            sys.stdout.flush()
            continue

        if re.match('b(reakpoint)* .*main', line):
            sys.stdout.write("Breakpoint 1: where = a.out`main + 15 at "
                             + "test_prog.c:22:10, address = "
                             + "0x0000000100000f2f\n")
        elif line == "process launch":
            sys.stdout.write("Process 12345 launched: "
                             + "'{}' (x86_64)\n".format(prog))
            sys.stdout.write("Process 12345 stopped\n")
            sys.stdout.write("* thread #1, queue = 'com.apple.main-thread', "
                             + "stop reason = breakpoint 1.1\n")
            sys.stdout.write("    frame #0 at /Users/stevent@kainos.com/code/"
                             + "personal/vim-padre/padre/test_prog.c:22\n")
            sys.stdout.write("Target 0: (tmp) stopped.\n")

        if re.match('frame variable (.*)', line):
            time.sleep(TIMEOUT + 1)
            sys.stdout.write("(int) i = 0\n")
        sys.stdout.flush()


if __name__ == '__main__':
    main()
