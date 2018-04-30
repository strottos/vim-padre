#!/usr/bin/env python
"""
Simple test program
"""
import sys
import time
import six


def main():
    """
    Main program
    """
    six.print_('Test stdout vimscript jobs')
    six.print_('Test stderr vimscript jobs', file=sys.stderr)
    time.sleep(0.5)
    six.print_('Testing done')


if __name__ == '__main__':
    main()
