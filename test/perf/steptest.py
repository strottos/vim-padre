"""
Test stepping performance of Padre with Locust performance tester
"""

import re
import select
import time
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
        padre_started = b'["call","padre#debugger#SignalPADREStarted",[]]'
        assert self.socket.recv(256) == padre_started
        self._send("""{"cmd":"run"}""")
        time.sleep(3)
        run_response = self.socket.recv(256).decode()
        assert re.match(r""".*\[1,{"pid":"\d+","status":"OK"}\]""",
                        run_response)
        assert re.match(r""".*\["call",".*JumpToPosition",\[.*,2\]\]""",
                        run_response)
        self.socket.setblocking(0)
        print("Setup Successfully")

    def step(self):
        """
        Perform a step
        """
        start_time = time.time()
        try:
            counter = self._send("""{"cmd":"stepOver"}""")
            time_start = datetime.now()
            data = None
            while True:
                if datetime.now() - time_start >= timedelta(TIMEOUT):
                    raise TimeoutException

                timeout = (time_start + timedelta(TIMEOUT) - datetime.now()) \
                    / timedelta(seconds=1)
                ready = select.select([self.socket], [], [], timeout)

                if ready[0]:
                    data = self.socket.recv(4096).decode()
                    print("Response: {}".format(data))

                if re.match(""".*\\[{},{{"status":"OK"}}\\]"""
                            .format(counter), data):
                    break
        except Exception as e:
            total_time = int((time.time() - start_time) * 1000)
            print("Failure: {}".format(total_time))
            events.request_failure.fire(request_type="execute",
                                        name="Step",
                                        response_time=total_time,
                                        exception=e)
        else:
            total_time = int((time.time() - start_time) * 1000)
            print("Success: {}".format(total_time))
            events.request_success.fire(request_type="execute",
                                        name="Step",
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

    @task
    def step(self):
        self.client.step()


class StepLocust(Locust):
    """
    Main class
    """
    task_set = StepTaskSet
    min_wait = 0
    max_wait = 1000
    host = "localhost:12345"

    def __init__(self, *args, **kwargs):
        super(Locust, self).__init__(*args, **kwargs)
        self.client = PadreConnection(self.host)
        self.client.setup()
