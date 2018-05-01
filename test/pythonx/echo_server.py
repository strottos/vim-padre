#!/usr/bin/env python
"""
Socket test program
"""
import argparse
import socket
import sys
import time


def main():
    """
    Main program
    """
    parser = argparse.ArgumentParser()
    parser.add_argument('--port', type=int, required=True,
                        help='port to run on')
    parser.add_argument('--init_sleep', type=int, default=0,
                        help='time to sleep for at the start')
    parser.add_argument('--sleep', type=int, default=0,
                        help='time to sleep for at the start')
    args = parser.parse_args()

    time.sleep(args.init_sleep)
    test_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    test_socket.bind(('0.0.0.0', args.port))
    test_socket.listen(1)
    while True:
        connection, address = test_socket.accept()
        while True:
            data = connection.recv(2048)
            print("Received: {}".format(data.rstrip()))
            time.sleep(args.sleep)

            if data == 'quit\r\n':
                connection.shutdown(1)
                connection.close()
                sys.exit(0)

            elif data:
                connection.send(data)


if __name__ == '__main__':
    main()
