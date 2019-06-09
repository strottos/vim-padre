"""
Test stepping performance of Padre with Locust performance tester
"""

import random
import re
import select
import time
import uuid
from datetime import datetime, timedelta
from socket import socket

from locust import Locust, TaskSet, events, task

TIMEOUT = 5


class TimeoutException(Exception):
    pass


class PadreConnection():
    """
    Padre socket connection wrapper
    """
    def __init__(self, host):
        self.host = host
        self.socket = socket()
        self.counter = 1

    def setup(self):
        """
        Create connection to Padre
        """
        socket_data = self.host.split(":")
        host = socket_data[0]
        port = int(socket_data[1]) if len(socket_data) > 1 else 12345
        self.socket.connect((host, port))
        self.socket.setblocking(0)
        print("Setup Successfully")

    def step(self):
        """
        Perform a step
        """
        start_time = time.time()
        try:
            cmd = "stepIn" if random.random() < 0.5 else "stepOver"
            counter = self._send("""{{"cmd":"{}"}}""".format(cmd))
            msg_time = datetime.now()
            data = ""
            status_ok = False
            position_ok = False
            while True:
                if datetime.now() - msg_time >= timedelta(seconds=TIMEOUT):
                    raise TimeoutException

                timeout = (msg_time + timedelta(seconds=TIMEOUT)
                           - datetime.now()) / timedelta(seconds=1)
                ready = select.select([self.socket], [], [], timeout)

                if ready[0]:
                    data += self.socket.recv(4096).decode()

                if re.match(""".*\\[{},{{"status":"OK"}}\\]"""
                            .format(counter), data):
                    status_ok = True

                if re.match('.*\\["call",".*JumpToPosition",\\[[^\\]]*\\]\\]',
                            data):
                    position_ok = True

                if status_ok and position_ok:
                    data = ""
                    break

        except Exception as ex:
            total_time = int((time.time() - start_time) * 1000)
            events.request_failure.fire(request_type="execute",
                                        name="Step",
                                        response_time=total_time,
                                        exception=ex)
        else:
            total_time = int((time.time() - start_time) * 1000)
            events.request_success.fire(request_type="execute",
                                        name="Step",
                                        response_time=total_time,
                                        response_length=0)

    def print(self):
        """
        Perform a print variable
        """
        start_time = time.time()
        try:
            variable = "i" if random.random() < 0.5 else "j"
            counter = self._send("""{{"cmd":"print","variable":"{}"}}"""
                                 .format(variable))
            msg_time = datetime.now()

            while True:
                if datetime.now() - msg_time >= timedelta(seconds=TIMEOUT):
                    raise TimeoutException

                timeout = (msg_time + timedelta(seconds=TIMEOUT)
                           - datetime.now()) / timedelta(seconds=1)
                ready = select.select([self.socket], [], [], timeout)

                if ready[0]:
                    data = self.socket.recv(4096).decode()

                if re.match('.*\\[{},{{"status":"OK","type":"int",'
                            '"value":"\\d+","variable":"{}"}}\\]'
                            .format(counter, variable), data):
                    break

                if re.match('.*\\[{},{{"status":"ERROR"}}\\]'.format(counter),
                            data):
                    break

        except Exception as ex:
            total_time = int((time.time() - start_time) * 1000)
            events.request_failure.fire(request_type="execute",
                                        name="Print",
                                        response_time=total_time,
                                        exception=ex)
        else:
            total_time = int((time.time() - start_time) * 1000)
            events.request_success.fire(request_type="execute",
                                        name="Print",
                                        response_time=total_time,
                                        response_length=0)

    def _send(self, msg):
        """
        Turn a message into a Padre message.

        e.g. '{"cmd":"run"}' gets sent to Padre as the byte sequence
        [<counter>,{"cmd":"run"}]
        """
        counter = self.counter
        self.counter += 1
        msg = """[{},{}]""".format(counter, msg).encode()
        self.socket.send(msg)
        return counter


class StepTaskSet(TaskSet):

    @task(10)
    def step(self):
        self.client.step()

    @task(1)
    def print(self):
        self.client.print()


class StepLocust(Locust):
    """
    Main class
    """
    task_set = StepTaskSet
    min_wait = 0
    max_wait = 100
    host = "localhost:12345"

    def __init__(self, *args, **kwargs):
        super(Locust, self).__init__(*args, **kwargs)
        self.client = PadreConnection(self.host)
        self.client.setup()
