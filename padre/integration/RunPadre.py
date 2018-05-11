"""
Library for running padre and communicating with it's stdout via expect
"""

import os
import re
import socket
import time

import pexpect

from robot.api import logger

class RunPadre:

    def __init__(self):
        pass

    def run_padre(self, args):
        logger.info("Spawning Padre with args {}".format(args))
        program = os.path.realpath(os.getcwd() + '/../padre')
        logger.info("Running program {} with args {}".format(program, args))
        if isinstance(args, list):
            self.child = pexpect.spawn(program + ' ' + args.join(' '))
        else:
            self.child = pexpect.spawn(program + ' ' + args)
        self.child.expect("Listening on .*$")
        port = int(self.child.after.decode('utf-8').strip().split(' ')[2].split(':')[1])
        logger.info("Padre running on port " + str(port))

        self.socket = socket.socket()
        self.socket.connect(('localhost', port))

    def send_to_padre(self, s):
        logger.info("Sending: " + s)
        self.socket.send(s.encode('utf-8'))

    def expect_from_padre(self, pattern):
        logger.info("Retrieving from Padre while expecting: " + pattern)
        recv = self.socket.recv(4096).decode('utf-8')
        logger.info("Received: " + recv)
        res = re.compile(pattern).match(recv)
        if not res:
            return [False]
        ret = [True]
        i = 1
        while True:
            try:
                ret.append(res[i])
            except IndexError:
                break
            i += 1
        return ret
