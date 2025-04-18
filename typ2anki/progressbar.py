import fcntl
import os
import sys
import termios
import threading
import tty

BAR_LENGTH = 30
RENDER_INTERVAL = 0.5


# Wrapper around the progress bar to allow for multiple progress bars, and to handle user interrupts
class ProgressBarManager:
    _instance = None
    lock = threading.Lock()
    log_lines = 0
    max_bar_total = 0
    e = 0

    def __init__(self):
        self.bars = []
        self.interrupted = False

        def listen_for_quit():
            fd = sys.stdin.fileno()
            old_settings = termios.tcgetattr(fd)
            fl = fcntl.fcntl(fd, fcntl.F_GETFL)
            try:
                tty.setcbreak(fd)  # Instead of raw mode, use cbreak
                fcntl.fcntl(
                    fd, fcntl.F_SETFL, fl | os.O_NONBLOCK
                )  # Make stdin non-blocking

                while True:
                    try:
                        key = sys.stdin.read(1)
                        if key and key.lower() == "q":
                            self.handle_interrupt()
                    except IOError:
                        pass  # No input, keep looping

            except KeyboardInterrupt:
                self.handle_interrupt()
            finally:
                termios.tcsetattr(fd, termios.TCSADRAIN, old_settings)
                fcntl.fcntl(
                    fd, fcntl.F_SETFL, fl
                )  # Restore original stdin settings

        input_thread = threading.Thread(target=listen_for_quit, daemon=True)
        input_thread.start()

    @classmethod
    def get_instance(cls):
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance

    def register(self, bar):
        with self.lock:
            self.bars.append(bar)
            self.max_bar_total = max(self.max_bar_total, bar.total)

    def handle_interrupt(self, signum=None, frame=None):
        print("\nUser interrupted the process.")
        self.interrupted = True
        sys.exit(0)

    def init(self):
        with self.lock:
            # sys.stdout.write("\033[?1049h")  # Switch to alternate screen buffer
            # sys.stdout.flush()
            print("\n" * len(self.bars), end="")
        for bar in self.bars:
            bar.init()

    def log_message(self, message):
        with self.lock:
            num_bars = len(self.bars)
            msg_lines = message.count("\n")
            sys.stdout.write(f"\033[s\033[{self.log_lines}E{message}")
            sys.stdout.write(f"\033[u")
            sys.stdout.flush()
            self.log_lines += msg_lines

    def finalize_output(self):
        with self.lock:
            sys.stdout.write(f"\033[{self.log_lines}B")
            sys.stdout.flush()
            # sys.stdout.write("\033[?1049l")  # Return to normal buffer
            # sys.stdout.flush()


ProgressBarManager.get_instance()


class ProgressBar:
    def __init__(self, total, title, position=0):
        self.total = total
        self.title = title
        self.position = position
        self.current = 0
        self.mutable_text = ""
        self.enabled = True
        self.printed = ""
        ProgressBarManager.get_instance().register(self)

    def update(self, iteration, mutable_text=""):
        inst = ProgressBarManager.get_instance()
        if inst.interrupted:
            sys.exit(0)
        if not self.enabled:
            return
        with ProgressBarManager.lock:
            self.current = iteration
            self.mutable_text = mutable_text
            progress = iteration / self.total
            block = int(BAR_LENGTH * progress)
            bar = "█" * block + "-" * (BAR_LENGTH - block)
            width = len(str(inst.max_bar_total))
            padded_iteration = str(iteration).zfill(width)
            padded_total = str(self.total).zfill(width)
            self.printed = f"{self.title}: |{bar}| {padded_iteration}/{padded_total} {self.mutable_text}"
            sys.stdout.write(f"\033[s\033[{self.position}A{self.printed}\033[u")
            # sys.stdout.write(f"\033[{self.position}F{self.printed}\033[{self.position}E")
            sys.stdout.flush()

    def update_text(self, text):
        self.title = text
        self.update(self.current, self.mutable_text)

    def next_step(self):
        self.update(self.current + 1, self.mutable_text)

    def reset(self):
        self.update(0, "")


class FileProgressBar(ProgressBar):
    def __init__(self, total, text, position=0):
        super().__init__(total, text, position)
        self.max_len_mut = 0

    def init(self):
        self.update(0, "Waiting...")

    def next(self, text):
        self.update(self.current + 1, text)
        if len(text) > self.max_len_mut:
            self.max_len_mut = len(text)

    def done(self, text="Done!"):
        if len(text) > self.max_len_mut:
            self.max_len_mut = len(text)
        self.update(self.total, "".ljust(self.max_len_mut))
        self.update(self.total, text)
