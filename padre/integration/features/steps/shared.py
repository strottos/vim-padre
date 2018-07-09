"""
Test basic PADRE functions with behave
"""
import asyncio
import json
import os
import re
import socket
import subprocess
from tempfile import TemporaryDirectory
from shutil import copyfile

from behave import given, when, then, fixture, use_fixture

TEST_FILES_DIR = os.path.join(
    os.path.dirname(os.path.realpath(__file__)),
    "../../test_files",
)

TIMEOUT = 5


class Padre():
    """
    Details for program
    """
    def __init__(self, executable, program_type):
        self._executable = executable
        self._program_type = program_type
        self._port = None
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


async def do_read_from_padre(future, reader, loop):
    """
    Read from PADRE
    """
    def cancel():
        future.cancel()

    loop.call_at(loop.time() + TIMEOUT, cancel)

    line = await reader.read(4096)
    line = line.decode()

    results = []

    idx = json.decoder.WHITESPACE.match(line, 0).end()
    end = len(line)

    try:
        while idx != end:
            (_, to) = json._default_decoder.raw_decode(line, idx=idx)
            results.append(line[idx:to])
            idx = json.decoder.WHITESPACE.match(line, to).end()
    except ValueError as exc:
        raise ValueError('%s (%r at position %d).' % (exc, line[idx:], idx))

    future.set_result(results)


async def do_send_to_padre(future, writer, message, loop):
    """
    Send a message to PADRE
    """
    def cancel():
        future.cancel()

    loop.call_at(loop.time() + TIMEOUT, cancel)

    writer.write(message.encode())
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

        context.padre.process = await asyncio.create_subprocess_exec(
            os.path.join(
                os.path.dirname(os.path.realpath(__file__)), "../../../padre"
            ), "--debugger={}".format(context.padre.program_type),
            "--port={}".format(context.padre.port), context.padre.executable,
            stdout=asyncio.subprocess.PIPE,
            loop=loop
        )

        line = await context.padre.process.stdout.readline()

        future.set_result(line)

    loop = asyncio.get_event_loop()
    future = loop.create_future()
    ensure = asyncio.ensure_future(do_run_padre(context, future, loop),
                                   loop=loop)
    loop.run_until_complete(ensure)
    line = future.result()

    assert line == "Listening on localhost:{}\n".format(context.padre.port).encode()
    yield True  # Pause teardown till later

    context.padre.process.terminate()


@fixture
def connect_to_padre(context):
    """
    Open a socket to the PADRE process and attach that socket to the
    `padre` object
    """
    use_fixture(run_padre, context)

    async def do_connect_to_padre(loop):
        con = asyncio.open_connection(
            "127.0.0.1",
            context.padre.port)

        context.reader, context.writer = await asyncio.wait_for(con,
                                                                int(TIMEOUT),
                                                                loop=loop)

    loop = asyncio.get_event_loop()

    loop.run_until_complete(do_connect_to_padre(loop))


@given("that we have a file '{source}'")
def copy_file(context, source):
    """
    Copy the contents of the file to a temporary directory and change dir
    to that directory
    """
    context.tmpdir = TemporaryDirectory()
    copyfile(os.path.join(TEST_FILES_DIR, source),
             os.path.join(context.tmpdir.name, source)
             )


@given(
    "I have compiled the test program '{source}' with compiler "
    "{compiler} to program '{output}'"
    )
def compile_program(context, source, compiler, output):
    """
    Compile the program and store that in the context for the program
    """
    execute = compiler.split(' ')
    execute.extend(["-o",
                    os.path.join(context.tmpdir.name, output),
                    os.path.join(context.tmpdir.name, source)
                    ])
    subprocess.run(execute, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                   check=True, cwd=os.getcwd())


@given(
    "that we have a test program '{executable}' that runs with '{progtype}'"
    )
def padre(context, executable, progtype):
    """
    Copy the contents of the test program to a temporary empty directory
    and change dir to that directory and store the program in the context
    """
    if "/" not in executable:
        executable = os.path.join(context.tmpdir.name, executable)
    if not os.path.exists(executable):
        copyfile(os.path.join(TEST_FILES_DIR, executable), executable)
    context.padre = Padre(executable, progtype)
    return padre


@when("I debug the program with PADRE")
def padre_debugger(context):
    """
    I start the PADRE debugger
    """
    use_fixture(connect_to_padre, context)


def check_calls_in(results, function, args):
    """
    Check the we have been called with the right arguments in `results`.
    E.g. we check that the following in the `results` list somewhere:

    ["call","<<function>>",<<args>>]

    e.g. ["call","padre#debugger#SignalPADREStarted",[]]
    """
    results_json = [json.loads(x) for x in results]

    result_found = False

    assert "call" in [x[0] for x in results_json]
    results_json = [x for x in results_json if x[0] == "call"]

    assert function in [x[1] for x in results_json]
    results_json = [x for x in results_json if x[1] == function]

    for result_json in results_json:
        found = True
        for (i, expected_arg) in enumerate(result_json[2]):
            print("{} {}".format(args[i], expected_arg))
            if not re.compile(str(args[i])).match(str(expected_arg)):
                found = False
                break

        if found:
            result_found = True
            break

    assert result_found


def check_call(result, function, args):
    """
    Check the we have been called with the right arguments.
    E.g. we check that

    ["call","<<function>>",<<args>>]

    e.g. ["call","padre#debugger#SignalPADREStarted",[]]
    """
    result_json = json.loads(result)
    assert result_json[0] == "call"
    assert result_json[1] == function
    for (i, arg) in enumerate(args):
        assert re.compile(arg).match(str(result_json[2][i]))


def check_response_in(results, request_number, expected_response):
    """
    I expect a response to a request of the following form in results:

    [<request_number>,"<response>"]

    e.g. [1,"OK file=test_prog.c line=16"]
    """
    json_results = [json.loads(x) for x in results]
    responses = [x for x in json_results if x[0] == request_number]
    print(responses)
    assert len(responses) == 1
    response = responses[0]

    assert response[0] == request_number
    assert response[1].split(' ')[0] == expected_response.split(' ')[0]
    assert re.compile(expected_response).match(response[1])


@then("I expect to be called with")
def padre_called_with(context):
    """
    I have recieved from PADRE the right call
    """
    loop = asyncio.get_event_loop()
    results = []
    while len(results) < len(context.table.rows):
        future = loop.create_future()
        loop.run_until_complete(do_read_from_padre(future, context.reader, loop))
        results.extend(future.result())

    assert len(results) == len(context.table.rows)
    for row in context.table:
        check_calls_in(results, row[0], json.loads(row[1]))


@when("I send a request to PADRE '{request}'")
def padre_request(context, request):
    """
    I send to PADRE a request of the form

    [<request_counter>,"<request>"]

    e.g. [1,"breakpoint file=test_prog.c line=16"]
    """
    loop = asyncio.get_event_loop()
    future = loop.create_future()

    request = json.dumps([context.padre.request_counter, request],
                         separators=(',', ':'))

    print("Requesting: {}".format(request))
    loop.run_until_complete(do_send_to_padre(future, context.writer,
                                             request, loop))
    assert future.result() is True


@then("I receive a response '{response}'")
def padre_response(context, response):
    """
    I expect the correct response to a request
    """
    loop = asyncio.get_event_loop()
    future = loop.create_future()
    loop.run_until_complete(do_read_from_padre(future, context.reader, loop))
    assert len(future.result()) == 1
    check_response_in(future.result(),
                      context.padre.last_request_number,
                      response
                      )


@then("I receive both a response '{response}' and I expect to be called with")
def padre_response_and_code_jump(context, response):
    """
    I expect a response and to jump to a point in the code in two separate
    messages
    """
    loop = asyncio.get_event_loop()
    results = []
    while len(results) < len(context.table.rows) + 1:
        future = loop.create_future()
        loop.run_until_complete(do_read_from_padre(future, context.reader, loop))
        results.extend(future.result())

    assert len(results) == len(context.table.rows) + 1
    for row in context.table:
        check_calls_in(results, row[0], json.loads(row[1]))
    check_response_in(results, context.padre.last_request_number, response)
