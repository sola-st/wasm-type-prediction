#!/usr/bin/env python3.7

import subprocess
import random
import re
import os
import sys
import logging
import shutil
import time
import argparse
import humanize
import signal

from pathlib import Path

from typing import List, Tuple

from utils import *

parser = argparse.ArgumentParser(description='Download and try to compile Ubuntu packages to Wasm.')
parser.add_argument('--package-list', metavar='<file>', help='list of packages to load from disk or generate (default: packages.list)', default='packages.list')
parser.add_argument('--output-dir', '-o', metavar='<dir>', help='directory for downloaded packages, logs, and compiled binaries (default: output/)', default='output')
parser.add_argument('--range', type=parse_range_argument, metavar='N..N', help='process only entries in that range from the package list (default: process all)', default=None)
parser.add_argument('--shuffle', default=False, action='store_true', help='shuffle the packages, avoids many related packages together [overall outcome is the same, but in the beginning should converge faster] (default: alphabetically)')
parser.add_argument('--random-sample', type=int, metavar='N', help='process only N random samples of the package list (default: no random sampling)', default=None)
parser.add_argument('--keep-src', default=False, action='store_true', help='do not delete src/ directory for each package (default: delete to save space)')
parser.add_argument('--verbose', '-v', default=False, action='store_true', help='produce debug output')
parser.add_argument('--logfile', metavar='<file>', help='write console output also to logfile (default: log only to console)', default=None)
args = parser.parse_args()

log_handlers = [logging.StreamHandler(sys.stdout)]
if args.logfile:
    log_handlers.append(logging.FileHandler(args.logfile))
logging.basicConfig(
    format='%(asctime)s %(levelname)-8s %(message)s', 
    level=logging.DEBUG if args.verbose else logging.INFO,
    datefmt='%Y-%m-%d %H:%M:%S',
    handlers=log_handlers
)
add_logging_level('SUCCESS', 25)
log = logging.getLogger(__name__)

log.debug(args)

# get a list of all packages in the Ubuntu repositories (either freshly or from disk)
def get_all_packages() -> List[str]:
    return sorted(
        subprocess.run(
            ['apt-cache', 'pkgnames'],
            capture_output=True,
            text=True).stdout.splitlines())

packages = readlines_else_create(args.package_list, get_all_packages, log)
log.info(f"{len(packages)} packages available")

# remove some packages that are not containing any code in the first place
def remove_packages(packages: List[str], pattern: str):
    pattern = f"^{pattern}$"
    regex = re.compile(pattern)
    packages[:] = [p for p in packages if not regex.match(p)]
    log.info(f"{len(packages)} packages after filtering out '{pattern}'")

remove_packages(packages, 'linux.*((headers)|(modules)|(image)|(tools)|(buildinfo)).*')
# Firefox and Thunderbird ship localized versions as seperate (and quite large) packages
remove_packages(packages, '((firefox)|(thunderbird)).*locale.*')
remove_packages(packages, '.*fonts.*')

# optional: select only packages in given range (e.g., for parallelization)
if args.range:
    start, end = args.range
    packages = packages[start:end]
    log.info(f"selected {len(packages)} packages from range {start} to {end} (exclusive)")
    log.debug(packages)

# optional: subsample packages for speedup
if args.random_sample:
    if args.random_sample < len(packages):
        # use a fixed see to make shuffling deterministic (for the same list at least)
        random.Random(0).shuffle(packages)
        packages = packages[:args.random_sample]
        # sort again, such that the sample is random, but the order then is alphabetical
        packages.sort()
        log.info(f"selected random subset of {len(packages)} packages")
        log.debug(packages)
    else:
        log.info('--random-sample ignored, because package list contains already less than N packages')

# optional: shuffle packages list for more unrelated packages in the beginning
if args.shuffle:
    if args.random_sample:
        log.info('--shuffle ignored because --random-sample already selects a random subset')
    else:
        # use a fixed see to make shuffling deterministic (for the same list at least)
        random.Random(0).shuffle(packages)
        log.info('shuffled package list to get faster convergence in the beginning')

cwd = os.getcwd()

def make_output_dir(dirname: str, description: str) -> Path:
    path = Path(args.output_dir, dirname).resolve()
    path.mkdir(parents=True, exist_ok=True)
    log.info(f"output directory for {description}: {path.relative_to(cwd)}/")
    return path

output_dir_all = make_output_dir('all', 'all packages')
output_dir_success = make_output_dir('success', 'successful builds')
output_dir_wasm = make_output_dir('wasm', 'Wasm binaries only')
output_dir_wasm_dwarf = make_output_dir('wasm-dwarf', 'Wasm binaries with DWARF info')

log.info('starting...\n')

class PackageError(Exception):
    def __init__(self, message):
        super(PackageError, self).__init__(message)

for i, package in enumerate(packages):
    log.info(f"package {package} ({i}/{len(packages)}, {i/len(packages):.1%})")

    # keep one directory per package, skip if already processed
    package_dir = output_dir_all / package
    if package_dir.exists():
        log.info(f"directory for {package} exists, skipping\n")
        continue

    package_dir.mkdir()
    os.chdir(package_dir)

    # logs of all external commands
    logs_dir = Path('.').resolve()
    logs_dir.mkdir(exist_ok=True)

    def run_with_logs(command: List[str], log_filename: str, timeout_minutes=None, env=None):
        returncode = 0
        with open(logs_dir / f"{log_filename}.stdout", 'w') as stdout:
            with open(logs_dir / f"{log_filename}.stderr", 'w') as stderr:
                # We want the timeout to kill not only the process, but also all children.
                # Unfortunately, subprocess.run() with timeout will only send SIGKILL to the process itself, not all its children.
                # So we have to do two things: 1. spawn the process in a new process group (preexec_fn argument)
                # and 2. send kill to all processeses in that process group and for the latter we
                # need the pid, which run() cannot give to us.
                # Hence, call Popen directly, see implementation of subprocess.run() and
                # https://stackoverflow.com/questions/32222681/how-to-kill-a-process-group-using-python-subprocess/32222971
                # https://stackoverflow.com/questions/23811650/is-there-a-way-to-make-os-killpg-not-kill-the-script-that-calls-it
                # https://github.com/python/cpython/blob/master/Lib/subprocess.py#L510
                with subprocess.Popen(command, stdout=stdout, stderr=stderr, preexec_fn=os.setpgrp, env=env) as proc:
                    try:
                        timeout = timeout_minutes * 60 if timeout_minutes else None
                        proc.communicate(timeout=timeout)
                    except subprocess.TimeoutExpired:
                        os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
                        proc.wait()
                        raise
                    except:  # Including KeyboardInterrupt, communicate handled that.
                        os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
                        # We don't call process.wait() as .__exit__ does that for us.
                        raise
                    returncode = proc.poll()
        stderr_text = None
        if returncode != 0:
            log.warning(f"non-zero return code {returncode} of {' '.join(command)}, see logs in {logs_dir.relative_to(cwd)}")
            with open(logs_dir / f"{log_filename}.stderr", 'r') as stderr:
                stderr_text = stderr.read()
        return returncode, stderr_text

    # TODO skip downloading if apt-cache showsrc gives Files: sizes > 100MB?

    os.mkdir('src')
    os.chdir('src')
    log.info('running apt-get source...')
    run_with_logs(['apt-get', 'source', package], 'apt-get-source')

    try:
        # apt-get source should unzip (and patch etc.) sources into a single directory
        package_src_dir = [d.path for d in os.scandir() if d.is_dir()]
        log.debug(f"extracted directories: {package_src_dir}")
        if not package_src_dir:
            raise PackageError('unzipped directory for sources not found')
        if len(package_src_dir) > 1:
            raise PackageError(f"more than one unzipped directory for sources: {package_src_dir}")
        package_src_dir = Path(package_src_dir[0]).resolve()
        log.info(f"unpacked and patched sources in {package_src_dir.relative_to(cwd)}/")

        # keep only the extracted&patched source files, remove intermediate archives
        for f in os.scandir():
            if f.is_file():
                os.remove(f)

        os.chdir(package_src_dir)

        # try to find any C, C++, header or other source file that we can compile with emscripten
        EMCC_SOURCE_FILES = ['c', 'cpp', 'c++', 'cc', 'h', 'h++', 'hxx', 'hpp']
        source_files = []
        for ext in EMCC_SOURCE_FILES:
            source_files.extend(str(f) for f in package_src_dir.glob(f"**/*.{ext}"))
        log.debug(f"source files: {source_files}")
        if not source_files:
            raise PackageError('no C/C++ source files found')

        # try to find a configure script, if not hope that it works without
        configures = list(find_by_filename_bfs(package_src_dir, 'configure'))
        log.debug(f"configure scripts: {configures}")
        configure_dir = None
        if not configures:
            log.info('no configure script found, trying to build without')
        else:
            configure = Path(configures[0]).resolve()
            if len(configures) > 1:
                log.warning(f"more than one configure script found, taking topmost: {configure.relative_to(cwd)}")

            configure_dir = configure.parent
            os.chdir(configure_dir)

            log.info(f"running emconfigure in {configure_dir.relative_to(cwd)}/...")
            # run ./configure with timeout, because some checks like for sleep and nanosleep loop forever :/
            configure_stderr = None
            try:
                _, configure_stderr = run_with_logs(['emconfigure', './configure'], 'emconfigure', timeout_minutes=20)
            # FIXME move this error message into the function, such that there are also timeout errors reported for cmake, make
            # FIXME truncate stdout/stderr logfiles (in run_with_logs) if they are beyond 100MB (?)
            except subprocess.TimeoutExpired as e:
                log.error(f"configure: killed after {e.timeout:.2f} seconds, maybe hang due to sleep/nanosleep test?")

            # special case missing library errors for quicker debugging
            if configure_stderr:
                error_required_lib = re.findall('required library (.*?) not found', configure_stderr, flags=re.IGNORECASE)
                for lib in error_required_lib:
                    log.error(f"configure: missing library {lib}")
                error_package_missing = re.findall('no package (.*) found', configure_stderr, flags=re.IGNORECASE)
                for pkg in error_package_missing:
                    log.error(f"configure: missing package {pkg}")
            
            os.chdir(package_src_dir)

        # try to run CMake if there is a CMakeLists.txt
        cmakelists = list(find_by_filename_bfs(package_src_dir, 'CMakeLists.txt'))
        log.debug(f"CMakeLists.txt files: {cmakelists}")
        cmake_dir = None
        if not cmakelists:
            log.info('no CMakeLists.txt found, trying to build without CMake')
        else:
            cmakelist = Path(cmakelists[0]).resolve()
            if len(cmakelists) > 1:
                log.warning(f"more than one CMakeLists.txt found, taking topmost: {cmakelist.relative_to(cwd)}")

            cmake_dir = cmakelist.parent
            os.chdir(cmake_dir)

            # see https://emscripten.org/docs/compiling/Building-Projects.html for running emconfigure with CMake
            # actually, running emconfigure with cmake results in an error and proposes to use emcmake...
            log.info(f"running emcmake in {cmake_dir.relative_to(cwd)}/...")
            run_with_logs(['emcmake', 'cmake', '.'], 'emcmake', timeout_minutes=20)

            os.chdir(package_src_dir)

        # take time before running make to compare generated files against this timestamp
        make_before_time = time.time()

        # run (em)make, potentially multiple times:
        # if we ran one build generation system (configure or CMake), run make in the same directory
        # and additionally always run it in the toplevel of the sources
        def run_emmake(make_dir, dir_description):
            os.chdir(make_dir)
            log.info(f"running emmake in {make_dir.relative_to(cwd)}/...")

            # Add EMMAKEN_CFLAGS=-g environment variable, which is being used by emcc (see emcc --help) such that all projects are built with debug info
            env = dict(os.environ, EMMAKEN_CFLAGS='-g')
            # also run make with timeout, in some cases there seems to be an infinite loop, maybe during generation of .dep files?
            _, make_stderr = run_with_logs(['emmake', 'make'], f"emmake-{dir_description}-dir", env=env, timeout_minutes=90)

            # special case makefile not found error for quicker debugging
            if make_stderr:
                error_no_makefile = re.search('no makefile found', make_stderr)
                if error_no_makefile:
                    log.error('make: no makefile found')

            os.chdir(package_src_dir)

        if configure_dir and configure_dir != package_src_dir:
            run_emmake(configure_dir, 'configure')
        if cmake_dir and cmake_dir != package_src_dir:
            run_emmake(cmake_dir, 'cmake')
        run_emmake(package_src_dir, 'toplevel')

        # check if there are wasm files anywhere, since linking can fail and extensions do not have to be .wasm, check by magic bytes
        def is_wasm_file_by_magic_bytes(file) -> bool:
            with open(file, 'rb') as f:
                return f.read(4) == b'\0asm'

        def contains_dwarf_info(file) -> bool:
            with open(file, 'rb') as f:
                return b'.debug_info' in f.read()

        wasm_files = []
        for path in package_src_dir.glob('**/*'):
            # resolve() can fail on cyclic symlinks (which happened for package bisonc++)
            try:
                path = Path(path).resolve()
            # ignore such broken links
            except RuntimeError:
                continue
            if not path.is_file():
                continue
            # ignore files that are from before we ran make
            if os.path.getmtime(path) < make_before_time:
                continue
            if is_wasm_file_by_magic_bytes(path):
                wasm_files.append(path)

        if wasm_files:
            log.success(f"found {len(wasm_files)} wasm binaries!")

            package_dir_success = output_dir_success / package
            if package_dir_success.exists():
                log.warning(f"{package_dir_success.relative_to(cwd)}/ already exists, removing...")
                shutil.rmtree(package_dir_success)
            package_dir_success.mkdir()
            log.info(f"copying the following wasm binaries to {package_dir_success.relative_to(cwd)}/")

            for f in wasm_files:
                log.success(f.relative_to(cwd))

                size = humanize.naturalsize(f.stat().st_size)
                log.info(f"file size: {size}")

                dwarf = contains_dwarf_info(f)
                log.info(f"DWARF info: {'yes' if dwarf else 'no'}")

                # copy wasm files recursively, with directory structure to wasm/
                # TODO instead of copying files to wasm-dwarf/ and wasm/ and copying/moving logs and src/ dir -> add per package symlink to all/?
                f_relative = f.relative_to(output_dir_all)
                f_dir_prefix = f_relative.parent

                f_dst_dir = output_dir_wasm / f_dir_prefix
                f_dst_dir.mkdir(parents=True, exist_ok=True)
                shutil.copy2(f, f_dst_dir)

                # copy also to wasm-dwarf if they also contain DWARF info, i.e., are useful for our supervised training
                if dwarf:
                    f_dst_dir = output_dir_wasm_dwarf / f_dir_prefix
                    f_dst_dir.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(f, f_dst_dir)

            # move whole src/ dir to success/, copy log files to success/ as well
            package_src_dir_parent = package_dir / 'src'

            log.info(f"copy log files in {package_dir.relative_to(cwd)}/ to {package_dir_success.relative_to(cwd)}/...")
            for f in os.scandir(package_dir):
                if f.is_file():
                    shutil.copy2(f, package_dir_success)
            # moving src/ instead of copytree and later rmdir avoids some problems with complex symlinks, see old comment:
                # symlinks=True is necessary because otherwise it tries to replicate symlinked directories,
                # and e.g., libxmlrpc-c++8v5 has a "self-referential" symlinked directory that blows up then...
            log.info(f"move {package_src_dir_parent.relative_to(cwd)}/ to {package_dir_success.relative_to(cwd)}/...")
            # WORKAROUND https://bugs.python.org/issue32689 pathlib object as first argument fails
            shutil.move(str(package_src_dir_parent), package_dir_success)

    except PackageError as e:
        log.error(e)

    os.chdir(cwd)

    # keep only the logs for all packages, in particular remove src/ to save space
    # (for successful builds the src/ directory was already moved to success/)
    package_src_dir_parent = package_dir / 'src'
    if not args.keep_src and package_src_dir_parent.exists():
        log.info(f"removing {package_src_dir_parent.relative_to(cwd)}/...")
        shutil.rmtree(package_src_dir_parent, onerror=lambda e: log.error(e))

    packages_success = sum(p.is_dir() for p in os.scandir(output_dir_success))
    log.info(f"{packages_success}/{i+1} ({packages_success/(i+1):.1%}) packages could be (partially) built\n")
