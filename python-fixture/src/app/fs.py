from typing import Protocol
class Fs(Protocol):
    def read(self, path: str) -> str:
        ...
