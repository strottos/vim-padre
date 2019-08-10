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
    sys.stdout.write("(lldb) ")
    sys.stdout.flush()

    for line in sys.stdin:
        line = line.rstrip()
        if re.match('settings.*', line):
            sys.stdout.write("(lldb) ")
            sys.stdout.flush()
            continue

        match = re.match('frame variable (.*)', line)
        if match:
            time.sleep(TIMEOUT + 1)
            sys.stdout.write("(int) i = 0\n")
        sys.stdout.write("(lldb) ")
        sys.stdout.flush()


if __name__ == '__main__':
    main()
