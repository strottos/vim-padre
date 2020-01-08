"""
Test basic PADRE functions with behave
"""
import asyncio
import json
import os
import re
import socket
import subprocess
import threading
import time
from shutil import copyfile
from tempfile import TemporaryDirectory

import psutil
from behave import fixture, given, then, use_fixture, when
from hamcrest import (assert_that, equal_to, has_item, is_in, is_not,
                      matches_regexp)

TEST_FILES_DIR = os.path.join(
    os.path.dirname(os.path.realpath(__file__)), "../../test_files"
)

TIMEOUT = 15


class Padre:
    """
    Details for program
    """

    def __init__(self, executable, debugger, program_type):
        self._executable = executable
        self._debugger = debugger
        self._program_type = program_type
        self._port = None
        self._pid = None
        self._children = set()
        self._proc = None
        self._request_counter = 1
        self._last_request_number = None

    @property
    def executable(self):
        """
        The executable for PADRE
        """
        return self._executable

    @property
    def debugger(self):
        """
        The debugger executable path
        """
        return self._debugger

    @property
    def program_type(self):
        """
        The program type for PADRE, e.g. lldb, node, java, etc
        """
        return self._program_type

    @property
    def port(self):
        """
        The port that PADRE is running on
        """
        if not self._port:
            self._port = self.get_unused_localhost_port()
        return self._port

    @property
    def pid(self):
        """
        Return the PADRE process
        """
        return self._pid

    @property
    def process(self):
        """
        Return the PADRE process
        """
        return self._proc

    @process.setter
    def process(self, proc):
        """
        The setter for the process
        """
        self._pid = proc.pid
        self._proc = proc

    @property
    def request_counter(self):
        """
        Return the current request counter
        """
        self._last_request_number = self._request_counter
        self._request_counter += 1
        return self._last_request_number

    @property
    def last_request_number(self):
        """
        Return the previous request number
        """
        return self._last_request_number

    @property
    def children(self):
        """
        Return the children of PADRE
        """
        return self._children

    @staticmethod
    def get_unused_localhost_port():
        """
        Find an unused port. Based on a similar function in YouCompleteMe.
        """
        sock = socket.socket()
        # This tells the OS to give us any free port in the range 1024-65535
        sock.bind(("", 0))
        port = sock.getsockname()[1]
        sock.close()
        return port

    def get_children(self):
        """
        Find Children of Padre PID and store them
        """
        self._children = self._children.union(
            set(psutil.Process(self.pid).children(recursive=True))
        )


async def do_read_from_padre(future, reader, loop):
    """
    Read from PADRE
    """

    def cancel():
        future.cancel()

    loop.call_at(loop.time() + TIMEOUT, cancel)

    async def do_read(reader):
        line = await reader.read(4096)
        line = line.decode()
        return line

    line = await asyncio.wait_for(do_read(reader), timeout=TIMEOUT)

    results = []

    idx = json.decoder.WHITESPACE.match(line, 0).end()
    end = len(line)

    try:
        while idx != end:
            (_, to) = json._default_decoder.raw_decode(line, idx=idx)
            results.append(line[idx:to])
            idx = json.decoder.WHITESPACE.match(line, to).end()
    except ValueError as exc:
        raise ValueError("%s (%r at position %d)." % (exc, line[idx:], idx))

    if len(results):
        print("Responses: {}".format(results))

    future.set_result(results)


async def do_send_to_padre(future, writer, message, loop):
    """
    Send a message to PADRE
    """

    def cancel():
        future.cancel()

    loop.call_at(loop.time() + TIMEOUT, cancel)

    async def do_write(writer, message):
        writer.write(message.encode())

    await asyncio.wait_for(do_write(writer, message), timeout=TIMEOUT)

    future.set_result(True)


@fixture
def run_padre(context, timeout=20):
    """
    Run padre debugger for program given
    """

    async def do_run_padre(context, future, loop):
        def cancel():
            future.cancel()

        loop.call_at(loop.time() + TIMEOUT, cancel)

        program = os.path.join(
            os.path.dirname(os.path.realpath(__file__)), "../../../target/debug/padre"
        )

        args = [
            "--host={}".format("127.0.0.1"),
            "--port={}".format(context.padre.port),
            context.padre.executable,
        ]

        if context.padre.program_type is not None:
            args.append("--type={}".format(context.padre.program_type))

        if context.padre.debugger is not None:
            args.append("--debugger={}".format(context.padre.debugger))

        context.padre.process = await asyncio.create_subprocess_exec(
            program,
            *args,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            loop=loop,
            cwd=os.path.realpath(context.tmpdir.name)
        )

        line = await context.padre.process.stdout.readline()

        future.set_result(line)

    loop = asyncio.get_event_loop()
    future = loop.create_future()
    ensure = asyncio.ensure_future(do_run_padre(context, future, loop), loop=loop)
    loop.run_until_complete(ensure)
    line = future.result()

    expected = "Listening on 127.0.0.1:{}\n".format(context.padre.port).encode()
    assert_that(line, equal_to(expected), "Started server")
    context.connections = []

    def print_stuff(loop, context):
        async def a_print_stuff(context):
            i = 0
            while True:
                try:
                    line = await context.padre.process.stdout.readline()
                    if line != b"":
                        i = 0
                        print(line)
                    else:
                        i += 1
                    if i == 3:
                        break
                except AttributeError:
                    break

        asyncio.ensure_future(a_print_stuff(context), loop=loop)

    t = threading.Thread(target=print_stuff, args=(loop, context))
    t.start()

    yield True  # Pause teardown till later

    if context.padre.process.returncode is None:
        context.padre.process.terminate()


@fixture
def connect_to_padre(context):
    """
    Open a socket to the PADRE process and attach that socket to the
    `padre` object
    """

    async def do_connect_to_padre(loop):
        con = asyncio.open_connection("127.0.0.1", context.padre.port)

        context.connections.append(await asyncio.wait_for(con, int(TIMEOUT), loop=loop))

    loop = asyncio.get_event_loop()

    loop.run_until_complete(do_connect_to_padre(loop))


@given("that we have a file '{source}'")
def copy_file(context, source):
    """
    Copy the contents of the file to a temporary directory and change dir
    to that directory
    """
    context.tmpdir = TemporaryDirectory()
    copyfile(
        os.path.join(TEST_FILES_DIR, source), os.path.join(context.tmpdir.name, source)
    )


@given(
    "I have compiled the test program '{source}' with compiler "
    "'{compiler}' to program '{output}'"
)
def compile_program(context, source, compiler, output):
    """
    Compile the program and store that in the context for the program
    """
    context.program = output
    execute = compiler.split(" ")
    execute.extend(
        [
            "-o",
            os.path.join(context.tmpdir.name, output),
            os.path.join(context.tmpdir.name, source),
        ]
    )
    subprocess.run(
        execute,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
        cwd=os.getcwd(),
    )
    time.sleep(0.1)


@given("that we have only a test program '{executable}'")
def padre(context, executable):
    """
    Copy the contents of the test program to a temporary empty directory
    and change dir to that directory and store the program in the context
    """
    if "/" not in executable:
        executable = os.path.join(context.tmpdir.name, executable)
    if not os.path.exists(executable):
        copyfile(os.path.join(TEST_FILES_DIR, executable), executable)
    context.padre = Padre(executable, None, None)
    return padre


@given(
    "that we have a test program '{executable}' that runs with '{debugger}' debugger"
)
def padre_with_debugger(context, executable, debugger):
    """
    Copy the contents of the test program to a temporary empty directory
    and change dir to that directory and store the program in the context
    """
    if "/" not in executable:
        executable = os.path.join(context.tmpdir.name, executable)
    if not os.path.exists(executable):
        copyfile(os.path.join(TEST_FILES_DIR, executable), executable)
    context.padre = Padre(executable, debugger, None)
    return padre


@given(
    "that we have a test program '{executable}' that runs with '{debugger}' debugger of type '{progtype}'"
)
def padre_with_debugger_and_type(context, executable, debugger, progtype):
    """
    Copy the contents of the test program to a temporary empty directory
    and change dir to that directory and store the program in the context
    """
    if "/" not in executable:
        executable = os.path.join(context.tmpdir.name, executable)
    if not os.path.exists(executable):
        copyfile(os.path.join(TEST_FILES_DIR, executable), executable)
    context.padre = Padre(executable, debugger, progtype)
    return padre


@when("I debug the program with PADRE")
def padre_debugger(context):
    """
    I start and connect to the PADRE debugger
    """
    use_fixture(run_padre, context)
    use_fixture(connect_to_padre, context)
    context.padre.get_children()


@when("I sleep for a moment")
def sleep_moment(context):
    """
    Just sleep for a very short period of time
    """
    time.sleep(0.1)


@when("I give PADRE chance to start")
def sleep_at_startup(context):
    """
    This is a bit rubbish but it's for tests that interfere with writing to
    stdin. This gives PADRE a chance to fully startup before we start confusing
    it.
    """
    time.sleep(1)


@when("I open another connection to PADRE")
def connect_padre(context):
    """
    I connect to the PADRE debugger
    """
    use_fixture(connect_to_padre, context)


@then("I expect to be called on connection {connection} with")
def padre_called_with(context, connection):
    """
    I have recieved from PADRE the right call
    """
    num_expected_results = len(context.table.rows)
    results = read_results(
        num_expected_results, context.connections[int(connection)][0]
    )
    assert_that(
        len(results),
        equal_to(num_expected_results),
        "Padre called with expected number of results",
    )
    for row in context.table:
        check_calls_in(results, row[0], json.loads(row[1]))


@then("I expect to be called with")
def padre_called_with_connection_zero(context):
    padre_called_with(context, 0)


@when("I send a raw request to PADRE '{request}' on connection {connection}")
def padre_request_raw(context, request, connection):
    """
    I send a request {request} as raw data on connection {connection}

    Largely used for error checking
    """
    loop = asyncio.get_event_loop()
    future = loop.create_future()

    print("Request: {}".format(request))

    request = request.replace("`pwd`", os.getcwd())
    request = request.replace("`test_dir`", os.path.realpath(context.tmpdir.name))

    loop.run_until_complete(
        do_send_to_padre(future, context.connections[int(connection)][1], request, loop)
    )
    assert_that(future.result(), "Padre request sent")
    context.padre.get_children()


@when("I send a raw request to PADRE '{request}'")
def padre_request_raw_connection_zero(context, request):
    """
    I send a request {request} as raw data on connection 0

    Largely used for error checking
    """
    padre_request_raw(context, request, 0)


@when("I send a request to PADRE '{request}' on connection {connection}")
def padre_request(context, request, connection):
    """
    I send to PADRE on the connection <connection> a request of the form

    [<request_counter>,<request>]

    e.g. [1,{"cmd":"breakpoint","file":"test_prog.c","line":16} ]
    """
    request = json.dumps(
        [context.padre.request_counter, json.loads(request.replace("\\n", "\n"))],
        separators=(",", ":"),
    )
    padre_request_raw(context, request, connection)


@when("I send a request to PADRE '{request}'")
def padre_request_connection_zero(context, request):
    """
    I send to PADRE a request on connection 0 of the form

    [<request_counter>,<request>]

    e.g. [1,{"cmd":"breakpoint","file":"test_prog.c","line":16} ]
    """
    padre_request(context, request, 0)


def get_response(connection):
    """
    Perform getting a response
    """
    loop = asyncio.get_event_loop()
    future = loop.create_future()

    def cancel():
        future.cancel()
        assert False

    loop.call_at(loop.time() + TIMEOUT, cancel)

    loop.run_until_complete(do_read_from_padre(future, connection, loop))

    return future


@then("I receive a response '{response}' on connection {connection}")
def padre_response(context, response, connection):
    """
    I expect the correct response to a request on connection <connection>
    """
    future = get_response(context.connections[int(connection)][0])
    assert_that(len(future.result()), equal_to(1), "Got one response")
    check_response_in(future.result(), context.padre.last_request_number, response)


@then("I receive a response '{response}'")
def padre_response_connection_zero(context, response):
    """
    I expect the correct response to a request
    """
    padre_response(context, response, 0)


@then("I receive a raw response '{response}'")
def padre_raw_response(context, response):
    """
    I expect the correct response to a request
    """
    future = get_response(context.connections[0][0])
    assert_that(len(future.result()), equal_to(1), "Got one response")
    assert_that(future.result()[0], equal_to(response))


@then(
    "I receive both a response '{response}' and I expect to be called on connection {connection} with"
)
def padre_response_and_code_jump(context, response, connection):
    """
    I expect a response and to jump to a point in the code in two separate
    messages
    """
    num_expected_results = len(context.table.rows) + 1
    results = read_results(
        num_expected_results, context.connections[int(connection)][0]
    )
    assert_that(
        len(results),
        equal_to(num_expected_results),
        "Got {} responses".format(num_expected_results),
    )
    for row in context.table:
        check_calls_in(results, row[0], json.loads(row[1]))
    check_response_in(results, context.padre.last_request_number, response)


@then("I receive both a response '{response}' and I expect to be called with")
def padre_response_and_code_jump_connection_zero(context, response):
    padre_response_and_code_jump(context, response, 0)


def check_calls_in(results, function, args):
    """
    Check the we have been called with the right arguments in `results`.
    E.g. we check that the following in the `results` list somewhere:

    ["call","<<function>>",<<args>>]

    e.g. ["call","padre#debugger#Log",[4,"TESTING"]]
    """
    results_json = [json.loads(x) for x in results]

    result_found = False

    assert_that([x[0] for x in results_json], has_item("call"), "Found call")
    results_json = [x for x in results_json if x[0] == "call"]

    assert_that(
        [x[1] for x in results_json],
        has_item(function),
        "Found function {}".format(function),
    )
    results_json = [x for x in results_json if x[1] == function]

    for result_json in results_json:
        found = True
        for (i, expected_arg) in enumerate(result_json[2]):
            # TODO: Check dictionaries are in, assume they match roughly for now
            if isinstance(args[i], dict):
                continue

            if not re.compile(str(args[i])).match(str(expected_arg)):
                found = False
                break

        if found:
            result_found = True
            break

    assert_that(result_found, "Result found")


def check_call(result, function, args):
    """
    Check the we have been called with the right arguments.
    E.g. we check that

    ["call","<<function>>",<<args>>]

    e.g. ["call","padre#debugger#SignalPADREStarted",[]]
    """
    result_json = json.loads(result)
    assert_that(result_json[0], equal_to("call"), "Found call")
    assert_that(result_json[1], equal_to(function), "Found function")
    for (i, arg) in enumerate(args):
        assert_that(
            str(result_json[2][i]),
            matches_regexp(re.compile(arg)),
            "Argument {} matches".format(i),
        )


def check_response_in(results, request_number, expected_response):
    """
    I expect a response to a request of the following form in results:

    [<request_number>,<response>]

    e.g. [1,{"status":"OK","file":"test_prog.c","line":16}]
    """
    json_results = [json.loads(x) for x in results]
    responses = [x for x in json_results if x[0] == request_number]
    assert_that(len(responses), equal_to(1), "Got 1 response")
    response = responses[0]

    assert_that(response[0], equal_to(request_number), "Found correct request number")

    expected_response = json.loads(expected_response)
    check_json(response[1], expected_response)


def check_json(response, expected_response):
    """
    Recursively verify the JSON matches
    """
    assert_that(
        response.keys(),
        equal_to(expected_response.keys()),
        "Got correct keys in response",
    )

    for key in response.keys():
        if isinstance(response[key], int):
            assert_that(response[key], expected_response[key], "Integers match")
        elif not isinstance(response[key], dict):
            assert_that(
                response[key],
                matches_regexp(expected_response[key]),
                "Response regexp matches",
            )
        else:
            check_json(response[key], expected_response[key])


def read_results(expected_results, reader):
    """
    Read the results from PADRE's responses
    """
    loop = asyncio.get_event_loop()
    results = []
    timeout = loop.time() + TIMEOUT
    while len(results) < expected_results:
        future = loop.create_future()

        def cancel():
            future.cancel()

        loop.call_at(loop.time() + TIMEOUT, cancel)

        loop.run_until_complete(do_read_from_padre(future, reader, loop))
        results.extend(future.result())

        if loop.time() > timeout:
            raise Exception("Timed out waiting for response")

    return results


@when("I send a command '{command}' using the terminal")
def send_terminal_command(context, command):
    """
    Send a command over the terminal via stdio of PADRE
    """
    loop = asyncio.get_event_loop()
    future = loop.create_future()

    def cancel():
        future.cancel()
        assert False

    async def write_terminal(padre, command, future, loop):
        def cancel():
            future.cancel()

        loop.call_at(loop.time() + TIMEOUT, cancel)

        padre.process.stdin.write((command + "\n").encode())
        await padre.process.stdin.drain()

        future.set_result(True)

    cancel = loop.call_at(loop.time() + TIMEOUT, cancel)
    print(cancel)

    loop.run_until_complete(write_terminal(context.padre, command, future, loop))


@when("I wait {seconds} seconds")
def sleep_seconds(context, seconds):
    """
    Wait for specified number of seconds
    """
    time.sleep(int(seconds))


@when("I terminate connection {connection}")
def terminate_connection(context, connection):
    """
    Close connection number {connection}
    """
    connection = context.connections[int(connection)]
    context.connections.remove(connection)
    connection = None


@when("I terminate padre")
def terminate_program(context):
    """
    Close PADRE
    """
    subprocess.run(
        ["kill", "-SIGINT", "{}".format(context.padre.process.pid)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
        cwd=os.getcwd(),
    )

    async def wait_process(loop):
        await context.padre.process.wait()

    loop = asyncio.get_event_loop()

    loop.run_until_complete(wait_process(loop))


@then("padre is not running")
def padre_not_running(context):
    """
    Close PADRE
    """
    for i in range(50):
        if context.padre.process.returncode == 0:
            break
        time.sleep(TIMEOUT / 50)

    assert_that(context.padre.process.returncode, equal_to(0), "Expected 0 exit code")

    time.sleep(1)
    running = set(psutil.pids())

    assert_that(context.padre.pid, is_not(is_in(running)), "Padre Pid found Running")
    for child in context.padre.children:
        assert_that(
            child.pid, is_not(is_in(running)), "Padre Child found {}".format(child)
        )
