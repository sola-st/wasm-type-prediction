from typing import List, Callable, Tuple, Iterator
import os
import logging
import re
from argparse import ArgumentTypeError

# see https://stackoverflow.com/questions/6512280/accept-a-range-of-numbers-in-the-form-of-0-5-using-pythons-argparse
def parse_range_argument(arg: str) -> Tuple[int, int]:
    match = re.match(r'(\d+)\.\.(\d+)$', arg)
    if not match:
        raise ArgumentTypeError(f"'{arg}' is not a range in the format N..N")
    start = int(match.group(1))
    end = int(match.group(2))
    if not start < end:
        raise ArgumentTypeError(f"end of range {arg} is not greater than start")
    return start, end

def readlines_else_create(filename: str, create_contents: Callable[[], List[str]], log) -> List[str]:
    try:
        with open(filename, 'r') as f:
            if log:
                log.info(f"loading from '{filename}'")
            lines = f.read().splitlines()
    except IOError:
        if log:
            log.info(f"'{filename}' does not exist, generating...")
        lines = create_contents()
        with open(filename, 'w') as f:
            for line in lines:
                f.write(f"{line}\n")
    return lines

# see https://stackoverflow.com/questions/2183233/how-to-add-a-custom-loglevel-to-pythons-logging-facility
def add_logging_level(levelName, levelNum, methodName=None):
    """
    Comprehensively adds a new logging level to the `logging` module and the
    currently configured logging class.

    `levelName` becomes an attribute of the `logging` module with the value
    `levelNum`. `methodName` becomes a convenience method for both `logging`
    itself and the class returned by `logging.getLoggerClass()` (usually just
    `logging.Logger`). If `methodName` is not specified, `levelName.lower()` is
    used.

    To avoid accidental clobberings of existing attributes, this method will
    raise an `AttributeError` if the level name is already an attribute of the
    `logging` module or if the method name is already present 

    Example
    -------
    >>> addLoggingLevel('TRACE', logging.DEBUG - 5)
    >>> logging.getLogger(__name__).setLevel("TRACE")
    >>> logging.getLogger(__name__).trace('that worked')
    >>> logging.trace('so did this')
    >>> logging.TRACE
    5

    """
    if not methodName:
        methodName = levelName.lower()

    if hasattr(logging, levelName):
        raise AttributeError('{} already defined in logging module'.format(levelName))
    if hasattr(logging, methodName):
        raise AttributeError('{} already defined in logging module'.format(methodName))
    if hasattr(logging.getLoggerClass(), methodName):
        raise AttributeError('{} already defined in logger class'.format(methodName))

    # This method was inspired by the answers to Stack Overflow post
    # http://stackoverflow.com/q/2183233/2988730, especially
    # http://stackoverflow.com/a/13638084/2988730
    def logForLevel(self, message, *args, **kwargs):
        if self.isEnabledFor(levelNum):
            self._log(levelNum, message, args, **kwargs)
    
    def logToRoot(message, *args, **kwargs):
        logging.log(levelNum, message, *args, **kwargs)

    logging.addLevelName(levelNum, levelName)
    setattr(logging, levelName, levelNum)
    setattr(logging.getLoggerClass(), methodName, logForLevel)
    setattr(logging, methodName, logToRoot)

# find a file by filename in a directory given by path, 
# return results in BFS order (i.e., topmost file first)
def find_by_filename_bfs(path, filename: str) -> Iterator[str]:
    for root, dirs, files in os.walk(path):
        for f in files:
            if f == filename:
                yield os.path.join(root, f)
