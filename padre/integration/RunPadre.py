"""
Library for running padre and communicating with it's stdout via expect
"""

import os
import re
import socket
import subprocess
import time

from robot.api import logger

class RunPadre:

    def __init__(self):
        self._buffer = ''
        self.socket = None
        self.child = None

    def run_padre(self, args):
        logger.info("Spawning Padre with args {}".format(args))
        program = os.path.realpath(os.getcwd() + "/../padre")
        run_list = [program, '--']
        if isinstance(args, list):
            run_list.extend(args)
        else:
            run_list.extend(args.split(' '))
        logger.info("Running command {}".format(run_list))
        self.child = subprocess.Popen(run_list)
        port = 12345

        time.sleep(1)
        self.socket = socket.socket()
        self.socket.connect(("localhost", port))

    def send_to_padre(self, s):
        logger.info("Sending: {}".format(s.encode("utf-8")))
        time.sleep(2)
        self.socket.send(s.encode("utf-8"))

    def expect_from_padre(self, pattern):
        logger.info("Retrieving from Padre while expecting: {}"
                    .format(pattern.encode("utf-8")))
        if not self._buffer:
            self._buffer += self.socket.recv(4096).decode("utf-8")
        logger.info("Received: {}".format(self._buffer.encode("utf-8")))
        logger.info("re.compile({}).match({})"
                    .format(pattern.encode('utf-8'),
                            self._buffer.encode('utf-8')))
        res = re.compile(pattern).match(self._buffer)
        self._buffer = self._buffer[len(res[0]):]
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
