#!/usr/bin/env python3

@dataclass(frozen=True)
class FileSummary:
    path: str
    size_bytes: int
    sha256: str
