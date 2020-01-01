#!/usr/bin/env python

import pty
import sys

pty.spawn(sys.argv[1:])
