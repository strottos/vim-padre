"""
Setup Padre ready for load testing

Ideally this should be done as part of locust but it didn't work
"""

import re
import time
from socket import socket

TIMEOUT = 5


def main():
    """
    Send a run command to Padre and check it comes up OK
    """
    host = "localhost:12345"
    socket_data = host.split(":")
    host = socket_data[0]
    port = int(socket_data[1]) if len(socket_data) > 1 else 12345
    print("{}".format((host, port)))
    sock = socket()
    sock.connect((host, port))
    padre_started = b'["call","padre#debugger#SignalPADREStarted",[]]'
    assert sock.recv(256) == padre_started
    sock.send(b"""[1,{"cmd":"run"}]""")
    time.sleep(3)
    run_response = sock.recv(256).decode()
    assert re.match(r""".*\[1,{"pid":"\d+","status":"OK"}\]""", run_response)
    assert re.match(r""".*\["call",".*JumpToPosition",\[.*,2\]\]""",
                    run_response)
    print("Setup Successfully")


main()
