#!/usr/bin/env python3
"""Mechanical pre-merge review of a pull request. Run it by hand before merging.

    tools/review-pr.py 44                 review PR #44 of the repo `origin` points at
    tools/review-pr.py 44 --expect-head 8fcc25a
    tools/review-pr.py --range BASE HEAD  offline: review two commits you already have

It fetches the PR as DATA and never checks it out, builds it or runs anything from
it. It reads the diff with git, and runs this checkout's own scanners
(tools/check-sensitive.sh, tools/check-pak-licences.sh) over it.

What it looks for: hidden Unicode (Trojan Source), workflow script injection and
privilege grabs, download-and-run, reverse shells, TLS and package-signature
bypasses, new network hosts, obfuscation, new or altered dependencies and lockfile
swaps, binaries, symlinks, submodules, new executables, edits to the repo's own
safeguards, weakened tests, writes to the user's MinUI Roms/Saves, secrets and
personal data, and text addressed to an automated reviewer.

A clean report is NOT proof of safety. It cannot judge intent, logic errors, what a
binary does, or what a dependency's code does. Read the diff of every file it lists.

Exit status: 0 nothing found, 1 something needs a human look, 2 blocking finding.
"""
import argparse
import fnmatch
import os
import re
import subprocess
import sys
import tempfile
import unicodedata
from dataclasses import dataclass
from datetime import datetime, timezone

try:
    import tomllib
except ImportError:  # Python < 3.11
    tomllib = None

BLOCK, REVIEW, INFO = "BLOCK", "REVIEW", "INFO"
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MAX_TEXT_FILES = 400
MAX_TEXT_BYTES = 3_000_000


@dataclass
class Finding:
    sev: str
    rule: str
    msg: str
    path: str = ""
    line: int = 0
    text: str = ""


class ToolError(Exception):
    pass


# ----------------------------------------------------------------------------- git


def git(repo, *args, check=True):
    p = subprocess.run(
        ["git", "-c", "core.quotepath=false", *args],
        cwd=repo,
        capture_output=True,
        env=dict(os.environ, GIT_TERMINAL_PROMPT="0"),
    )
    if check and p.returncode != 0:
        raise ToolError(f"git {' '.join(args[:3])} failed: {p.stderr.decode(errors='replace').strip()}")
    return p.stdout, p.returncode


def git_text(repo, *args, check=True):
    out, _ = git(repo, *args, check=check)
    return out.decode("utf-8", errors="replace")


def blob(repo, rev, path):
    out, rc = git(repo, "show", f"{rev}:{path}", check=False)
    return out if rc == 0 else None


# ------------------------------------------------------------------------ rendering


def visible(s):
    """Make hidden characters visible, so the report cannot be deceptive itself."""
    out = []
    for ch in s:
        o = ord(ch)
        if ch == "\t":
            out.append("\\t")
        elif o < 32 or 0x7F <= o < 0xA0 or ch in HIDDEN or ch in BIDI:
            out.append(f"\\u{{{o:04X}}}")
        else:
            out.append(ch)
    return "".join(out)


def snippet(s, width=110):
    s = re.sub(r"[A-Za-z0-9+/_\-]{32,}", "[long token]", s.strip())
    s = visible(s)
    return s if len(s) <= width else s[: width - 1] + "…"


# ------------------------------------------------------------------------ file kinds

DOC_EXT = {".md", ".txt", ".rst", ".adoc"}
ASSET_EXT = {".png", ".jpg", ".jpeg", ".gif", ".ico", ".bmp", ".ttf", ".otf", ".woff", ".woff2", ".webp"}
CODE_EXT = {".rs", ".sh", ".bash", ".py", ".yml", ".yaml", ".toml", ".json", ".c", ".h", ".cpp", ".js", ".ts", ".lua"}


def kind_of(path):
    base = os.path.basename(path)
    ext = os.path.splitext(base)[1].lower()
    if path.startswith(".github/") and ext in (".yml", ".yaml"):
        return "workflow"
    if base == "build.rs":
        return "build"
    if ext == ".rs":
        return "rust"
    if ext in (".sh", ".bash") or path.startswith(".githooks/") or base in ("launch.sh", "auto.sh"):
        return "shell"
    if ext in DOC_EXT or base.upper().startswith(("LICENSE", "COPYING", "NOTICE", "README")):
        return "doc"
    if ext in ASSET_EXT:
        return "asset"
    return "other"


# Paths a human must read, with the reason.
SENSITIVE = [
    (".github/*", "CI and release automation runs with this repo's tokens"),
    (".githooks/*", "git hooks run on every contributor's machine"),
    ("tools/*", "scripts the maintainers run"),
    ("*/build.rs", "runs on the build machine at compile time"),
    ("build.rs", "runs on the build machine at compile time"),
    ("Cargo.toml", "dependencies and build settings"),
    ("*/Cargo.toml", "dependencies and build settings"),
    ("Cargo.lock", "pins the exact dependency code that is built"),
    (".cargo/*", "changes how everything is built"),
    ("rust-toolchain.toml", "changes the compiler"),
    ("*.sh", "shell script (on the device it runs as root)"),
    ("*/launch.sh", "starts as root on the handheld"),
    ("cores/*", "emulator core build recipes and patches"),
    ("taskfile.yml", "build tasks"),
    ("Dockerfile*", "build image"),
    (".gitmodules", "submodule sources"),
    (".gitattributes", "can change how diffs and files are treated"),
    ("CODEOWNERS", "who must approve changes"),
    ("*/System/*.so", "emulator core binary that ships in the download"),
    ("*/Slot.pak/slot", "frontend binary that ships in the download"),
]

# Files that are the repo's own defences. Changing them is not wrong, but it is how
# a defence gets quietly switched off.
SAFEGUARDS = [
    "tools/check-sensitive.sh", "tools/check-sensitive-test.sh", "tools/check-pak-licences.sh",
    "tools/review-pr.py", "tools/test_review_pr.py", "tools/install-hooks.sh",
    ".githooks/*", ".github/workflows/pr.yml", ".github/workflows/secrets.yml", "CODEOWNERS",
]


def matches(path, patterns):
    return any(fnmatch.fnmatch(path, p) for p in patterns)


# -------------------------------------------------------------------- text patterns

SHELLISH = {"workflow", "shell", "build", "other", "rust"}
CODEISH = {"workflow", "shell", "build", "other", "rust"}
NOT_DOC = {"workflow", "shell", "build", "other", "rust", "asset"}

PATTERNS = [
    # (rule, severity, regex, kinds, message)
    ("pipe-to-shell", BLOCK, r"\b(curl|wget|fetch)\b[^\n|;&]*\|\s*(sudo\s+)?(env\s+)?(ba|z|da|k)?sh\b", SHELLISH,
     "downloads something and pipes it straight into a shell"),
    ("shell-from-download", BLOCK, r"\b(ba|z|da)?sh\s+(-\w+\s+)*[\"']?\$\(\s*(curl|wget)\b|<\(\s*(curl|wget)\b", SHELLISH,
     "runs a shell on freshly downloaded text"),
    ("decode-and-run", BLOCK,
     r"base64\s+(-d|--decode|-D)\b[^\n]*\|\s*(sudo\s+)?((ba|z|da)?sh|python3?|perl|ruby|node)\b|\beval\b[^\n]*base64", SHELLISH,
     "decodes hidden text and runs it"),
    ("reverse-shell", BLOCK,
     r"/dev/(tcp|udp)/|\bn(c|cat)\b[^\n]*\s-(e|c)\b|\bbash\s+-i\s*>&|\bsocat\b[^\n]*\bexec\b|\bmkfifo\b[^\n]*\bn(c|cat)\b", SHELLISH,
     "looks like a reverse shell"),
    ("setuid", BLOCK, r"\bchmod\s+(-\w+\s+)*([ugoa]*\+s|[2467][0-7]{3})\b", SHELLISH, "sets a setuid/setgid bit"),
    ("tls-off", BLOCK,
     r"\bcurl\b[^\n]*\s(-k|--insecure)\b|--no-check-certificate|danger_accept_invalid_certs|GIT_SSL_NO_VERIFY"
     r"|NODE_TLS_REJECT_UNAUTHORIZED\s*=\s*['\"]?0|verify\s*=\s*False|PYTHONHTTPSVERIFY\s*=\s*0", SHELLISH,
     "turns off TLS certificate checks"),
    ("apt-unsigned", BLOCK, r"trusted=yes|--allow-unauthenticated|AllowInsecureRepositories|--force-yes", SHELLISH,
     "installs packages without checking their signatures"),
    ("private-key", BLOCK, r"-----BEGIN [A-Z ]*PRIVATE KEY-----|\bssh-(rsa|ed25519)\s+AAAA", NOT_DOC | {"doc"},
     "contains a private or authorised SSH key"),
    ("write-block-device", BLOCK, r"\bdd\b[^\n]*\bof=/dev/(mmcblk|sd[a-z]|nvme|disk)|>\s*/dev/(mmcblk|sd[a-z]|nvme)", SHELLISH,
     "writes straight to a disk or SD card device"),
    ("rm-root", BLOCK, r"\brm\s+(-\w+\s+)+(--no-preserve-root\s+)?/(\s|\*|$)|--no-preserve-root", SHELLISH,
     "recursively deletes from the filesystem root"),
    ("pull-request-target", BLOCK, r"\bpull_request_target\b", {"workflow"},
     "runs with secrets on code from a fork's pull request"),
    ("write-all", BLOCK, r"permissions:\s*write-all", {"workflow"}, "grants the workflow every permission"),

    ("disk-tools", REVIEW, r"\b(mkfs(\.\w+)?|wipefs|fdisk|sfdisk|parted|diskutil\s+(erase|partition))\b", SHELLISH,
     "formats or partitions disks (this project never formats a user's card)"),
    ("minui-data", REVIEW, r"/mnt/sdcard/(Roms|Saves|Bios)\b|/mnt/SDCARD/(Roms|Saves|Bios)\b", SHELLISH,
     "touches the user's MinUI Roms/Saves/Bios, which slot promises never to modify"),
    ("network-fetch", REVIEW, r"\b(curl|wget)\b|\bgit\s+clone\b|\bpip3?\s+install\b|\bnpm\s+(i|install)\b|\bcargo\s+install\b|\bgem\s+install\b|\bgo\s+(get|install)\b",
     SHELLISH, "fetches or installs something from the network"),
    ("upload", REVIEW, r"\bcurl\b[^\n]*\s(-d|--data\S*|-F|-T|--upload-file)\b|\bwget\b[^\n]*--post|\bn(c|cat)\b\s", SHELLISH,
     "sends data somewhere"),
    ("sudo", REVIEW, r"\bsudo\b", SHELLISH, "uses sudo"),
    ("eval", REVIEW, r"\beval\b|\b(eval|exec)\s*\(|\bos\.system\b|\bsubprocess\b.*shell\s*=\s*True", SHELLISH,
     "evaluates text as code"),
    ("libc-exec", REVIEW, r"\blibc::(system|execv?e?p?|popen|dlopen|ptrace)\b", {"rust", "build"}, "calls into the OS to run or load code"),
    ("build-script-exec", REVIEW, r"\b(Command::new|process::Command)\b", {"build"}, "build script runs a program at compile time"),
    ("dump-env", REVIEW, r"\bprintenv\b|(^|[;&|]\s*)env\s*(\||>)|\bset\s*\|", {"workflow", "shell"}, "dumps the environment (secrets land in logs)"),
    ("scanner-bypass", REVIEW, r"--no-verify|core\.hooksPath|check-sensitive:\s*allow|SENSITIVE_PATTERNS_FILE\s*=\s*/dev/null", NOT_DOC | {"doc"},
     "bypasses or loosens a repo safeguard"),
    ("secrets-use", REVIEW, r"\$\{\{\s*secrets\.|secrets:\s*inherit", {"workflow"}, "uses repository secrets"),
    ("elevated-permission", REVIEW,
     r"\b(contents|actions|packages|id-token|pull-requests|issues|checks|statuses|deployments|security-events|workflows):\s*write\b", {"workflow"},
     "asks for a write permission"),
    ("self-hosted", REVIEW, r"runs-on:\s*\[?\s*self-hosted", {"workflow"}, "runs on a self-hosted machine"),
    ("ci-disabled", REVIEW, r"continue-on-error:\s*true|\bif:\s*(false|\$\{\{\s*false\s*\}\})", {"workflow"}, "weakens or skips a CI step"),
    ("wf-injection", REVIEW,
     r"\$\{\{\s*github\.head_ref\b|\$\{\{\s*github\.event\.[\w.\[\]*'\"-]*\.(title|body|head\.ref|head\.label|label|message|email|name|page_name|default_branch)\b",
     {"workflow"}, "puts attacker-controlled text in an expression (injectable if used inside run:; pass it via env)"),
    ("apt-key", REVIEW, r"\bapt-key\s+add\b|add-apt-repository\b", SHELLISH, "adds a new package source or key"),
    ("authorized-keys", REVIEW, r"\bauthorized_keys\b|/etc/(passwd|shadow|sudoers)|~/\.ssh|\.aws/credentials", SHELLISH,
     "touches credentials or accounts"),
    ("encoded-blob", REVIEW, r"[A-Za-z0-9+/]{80,}={0,2}", SHELLISH, "long encoded-looking blob"),
    ("hex-escapes", REVIEW, r"(\\x[0-9a-fA-F]{2}){8,}", SHELLISH, "long run of hex escapes"),
    ("svg-script", REVIEW, r"<script\b|\bon(load|error|click)\s*=", {"other"}, "script inside what may be an image"),
]
COMPILED = [(r, s, re.compile(rx, re.I if r in ("pipe-to-shell",) else 0), k, m) for r, s, rx, k, m in PATTERNS]

INJECTION = re.compile(
    r"ignore\s+(all\s+|any\s+|the\s+)?(previous|prior|above|earlier)\s+(instructions?|prompts?|rules?)"
    r"|disregard\s+(the\s+|all\s+|any\s+)?(previous|prior|above|earlier|your)\b"
    r"|you\s+are\s+(now\s+)?(an?\s+)?(ai|assistant|language\s+model|llm|reviewer|bot)\b"
    r"|\b(system|developer)\s+prompt\b|as\s+an\s+ai\s+(language\s+)?model"
    r"|do\s+not\s+(tell|inform|mention|report|flag)\b[^.\n]{0,40}\b(user|maintainer|owner|human|reviewer)"
    r"|\b(approve|merge)\s+this\s+(pr|pull\s+request|change)\s+(immediately|without|now)"
    r"|<\s*/?\s*(system|assistant|instructions?)\s*>|\[/?INST\]|\bnew\s+instructions\s*:",
    re.I,
)

URL = re.compile(r"https?://([A-Za-z0-9.-]+)(?::\d+)?", re.I)
IP_URL = re.compile(r"https?://\d{1,3}(\.\d{1,3}){3}\b")
TRUSTED_HOSTS = (
    "github.com", "githubusercontent.com", "github.io", "crates.io", "docs.rs", "rust-lang.org", "rustup.rs",
    "debian.org", "slot-cfw.fyi", "libretro.com", "shields.io", "docker.com", "docker.io", "ubuntu.com",
    "kernel.org", "gnu.org", "mozilla.org", "creativecommons.org", "apache.org", "opensource.org", "w3.org",
    "example.com", "example.org", "localhost", "127.0.0.1",
)

BIDI = set("\u202a\u202b\u202c\u202d\u202e\u2066\u2067\u2068\u2069\u200e\u200f\u061c")
HIDDEN = set("\u200b\u200c\u200d\u2060\ufeff\u00ad\u180e\u2028\u2029\u2062\u2063\u2064\u034f")
CONFUSABLE_SCRIPTS = ("CYRILLIC", "GREEK", "ARMENIAN", "FULLWIDTH", "MATHEMATICAL", "CHEROKEE", "COPTIC", "LATIN SMALL LETTER DOTLESS", "LATIN LETTER")

UNPINNED = re.compile(r"uses:\s*['\"]?([^\s'\"#]+)")
FIRST_PARTY = ("actions/", "github/")


# ------------------------------------------------------------------- change model


@dataclass
class Change:
    path: str
    old_path: str
    status: str
    old_mode: str
    new_mode: str
    added: list  # (lineno, text)
    removed: list  # text
    binary: bool = False


def collect_changes(repo, base, head):
    out, _ = git(repo, "diff", "--raw", "-z", "-M", "--no-ext-diff", "--no-textconv", base, head)
    toks = out.split(b"\0")
    changes, i = [], 0
    while i < len(toks) and toks[i].startswith(b":"):
        om, nm, _o, _n, status = toks[i][1:].decode().split(" ")[:5]
        i += 1
        if status[0] in "RC":
            old, new = toks[i].decode("utf-8", "replace"), toks[i + 1].decode("utf-8", "replace")
            i += 2
        else:
            old = new = toks[i].decode("utf-8", "replace")
            i += 1
        changes.append(Change(new, old, status[0], om, nm, [], []))
    return changes


def read_hunks(repo, base, head, change):
    if change.new_mode in ("160000", "120000") or change.status == "D":
        spec = [change.old_path] if change.status == "D" else []
    else:
        spec = [change.path]
    if change.status == "D":
        text, _ = git(repo, "diff", "-U0", "--no-color", "--no-ext-diff", "--no-textconv", base, head, "--", f":(literal){change.old_path}")
    elif spec:
        paths = [f":(literal){change.path}"] + ([f":(literal){change.old_path}"] if change.old_path != change.path else [])
        text, _ = git(repo, "diff", "-U0", "-M", "--no-color", "--no-ext-diff", "--no-textconv", base, head, "--", *paths)
    else:
        return
    lineno, in_hunk = 0, False
    for raw in text.decode("utf-8", errors="replace").split("\n"):
        if raw.startswith("@@"):
            in_hunk = True
            m = re.match(r"@@ -\d+(?:,\d+)? \+(\d+)", raw)
            lineno = int(m.group(1)) if m else 0
        elif not in_hunk:
            # Header lines. A removed line that itself starts with "--" must not be
            # mistaken for one, so headers are only recognised before the first hunk.
            if raw.startswith("Binary files") or raw.startswith("GIT binary patch"):
                change.binary = True
        elif raw.startswith("+"):
            change.added.append((lineno, raw[1:]))
            lineno += 1
        elif raw.startswith("-"):
            change.removed.append(raw[1:])


# ------------------------------------------------------------------------ checks


def check_paths(changes, repo, head):
    f = []
    for c in changes:
        p = c.path
        flags = []
        for pat, why in SENSITIVE:
            if fnmatch.fnmatch(p, pat) or (c.status == "D" and fnmatch.fnmatch(c.old_path, pat)):
                f.append(Finding(REVIEW, "sensitive-path", f"read this file yourself: {why}", p))
                break
        if matches(p, SAFEGUARDS) or matches(c.old_path, SAFEGUARDS):
            verb = "deletes" if c.status == "D" else "changes"
            f.append(Finding(REVIEW, "safeguard", f"{verb} one of the repo's own safeguards", p))
        if c.status == "D" and re.search(r"(^|/)(tests?|spec)/|_test\.|^test_|-test\.", c.old_path):
            f.append(Finding(REVIEW, "test-deleted", "deletes a test file", c.old_path))
        if c.new_mode == "120000":
            target = (blob(repo, head, p) or b"").decode("utf-8", "replace")
            bad = target.startswith("/") or ".." in target.split("/")
            f.append(Finding(BLOCK if bad else REVIEW, "symlink", f"adds a symlink to {visible(target)!r}" + (" (points outside the repo)" if bad else ""), p))
        if c.new_mode == "160000":
            f.append(Finding(REVIEW, "submodule", "adds or moves a submodule (code from another repo)", p))
        if c.new_mode == "100755" and c.old_mode != "100755":
            f.append(Finding(REVIEW, "executable", "file becomes executable", p))
        if c.binary and c.status != "D":
            size = (blob(repo, head, p) or b"")
            elf = size[:4] == b"\x7fELF" or size[:2] == b"MZ" or size[:4] in (b"\xcf\xfa\xed\xfe", b"\xca\xfe\xba\xbe")
            ext = os.path.splitext(p)[1].lower()
            if elf or ext in (".so", ".dll", ".exe", ".dylib", ".bin", ".o", ".a") or kind_of(p) != "asset":
                f.append(Finding(REVIEW, "binary", f"binary file ({len(size):,} bytes{', executable code' if elf else ''}): a diff cannot show what it does; rebuild it from source and compare", p))
            else:
                f.append(Finding(INFO, "asset", f"image/font ({len(size):,} bytes): not executed, but check where it came from", p))
            if len(size) > 5_000_000:
                f.append(Finding(REVIEW, "large-file", f"{len(size):,} bytes", p))
    return f


def check_text(changes, repo, head):
    f = []
    seen_hosts = set()
    for n, c in enumerate(changes):
        if c.binary or c.status == "D" or c.new_mode in ("160000", "120000"):
            continue
        kind = kind_of(c.path)
        cr = 0
        for lineno, text in c.added:
            if "\r" in text:
                cr += 1
                text = text.replace("\r", "")
            for ch in text:
                o = ord(ch)
                if ch in BIDI:
                    f.append(Finding(BLOCK, "bidi", f"bidirectional control character U+{o:04X} (can make code read differently from how it runs)", c.path, lineno, text))
                    break
                if o == 0x1B:
                    f.append(Finding(BLOCK, "escape-char", "terminal escape character (can rewrite what you see when you cat or diff it)", c.path, lineno, text))
                    break
                if ch in HIDDEN and not (ch == "\ufeff" and lineno == 1):
                    f.append(Finding(REVIEW, "invisible", f"invisible character U+{o:04X}", c.path, lineno, text))
                    break
                if (o < 32 and ch != "\t") or 0x7F <= o < 0xA0:
                    f.append(Finding(REVIEW, "control-char", f"control character U+{o:04X}", c.path, lineno, text))
                    break
            if kind in ("rust", "shell", "workflow", "build", "other") and os.path.splitext(c.path)[1].lower() in CODE_EXT:
                for ch in text:
                    if ord(ch) > 127 and unicodedata.category(ch).startswith("L"):
                        name = unicodedata.name(ch, "")
                        if name.startswith(CONFUSABLE_SCRIPTS):
                            f.append(Finding(REVIEW, "homoglyph", f"look-alike letter {name.title()} (U+{ord(ch):04X}) in code", c.path, lineno, text))
                            break
            if len(text) > 500 and kind in NOT_DOC and kind != "asset":
                f.append(Finding(REVIEW, "long-line", f"{len(text)}-character line", c.path, lineno, text))
            if INJECTION.search(text):
                f.append(Finding(REVIEW, "reviewer-injection", "text that reads as instructions to an automated reviewer", c.path, lineno, text))
            for rule, sev, rx, kinds, msg in COMPILED:
                if kind in kinds and rx.search(text):
                    if rule == "encoded-blob" and c.path.endswith((".lock", ".json")):
                        continue
                    f.append(Finding(sev, rule, msg, c.path, lineno, text))
            if kind == "workflow":
                m = UNPINNED.search(text)
                if m and not m.group(1).startswith("./"):
                    ref = m.group(1)
                    pinned = re.search(r"@[0-9a-f]{40}$", ref)
                    first = ref.startswith(FIRST_PARTY)
                    if ref.startswith("docker://"):
                        f.append(Finding(REVIEW, "action", f"runs a container image directly: {ref}", c.path, lineno, text))
                    elif not first:
                        f.append(Finding(REVIEW, "action", f"third-party action {ref}" + ("" if pinned else " (not pinned to a commit)"), c.path, lineno, text))
                    elif not pinned:
                        f.append(Finding(INFO, "action", f"{ref} (a tag, not a commit)", c.path, lineno, text))
            for m in URL.finditer(text):
                host = m.group(1).lower().rstrip(".")
                if IP_URL.match(m.group(0)):
                    f.append(Finding(REVIEW, "ip-url", f"URL to a bare IP address: {m.group(0)}", c.path, lineno, text))
                elif not any(host == h or host.endswith("." + h) for h in TRUSTED_HOSTS) and (host, kind == "doc") not in seen_hosts:
                    seen_hosts.add((host, kind == "doc"))
                    f.append(Finding(INFO if kind == "doc" else REVIEW, "new-host", f"new network host {host}", c.path, lineno, text))
        if cr:
            f.append(Finding(REVIEW, "carriage-return", f"{cr} added line(s) contain a carriage return (mixed line endings can hide text)", c.path))
        f.extend(check_test_weakening(c))
    return f


ASSERT = re.compile(r"\bassert(_eq|_ne|_matches)?!|\bassert\b|\bexpect\(|\bpanic!|\bfail\(")


def check_test_weakening(c):
    istest = re.search(r"(^|/)(tests?|spec)/|_test\.|(^|/)test_|-test\.", c.path)
    f = []
    if istest:
        gone = sum(1 for t in c.removed if ASSERT.search(t))
        came = sum(1 for _, t in c.added if ASSERT.search(t))
        if gone > came:
            f.append(Finding(REVIEW, "test-weakened", f"removes {gone} assertion line(s), adds {came}", c.path))
    for lineno, t in c.added:
        if re.search(r"#\[ignore\b|@unittest\.skip|\bskipTest\b|pytest\.mark\.skip|\bit\.skip\b", t):
            f.append(Finding(REVIEW, "test-skipped", "skips a test", c.path, lineno, t))
    return f


def parse_toml(data):
    if tomllib is None or data is None:
        return None
    try:
        return tomllib.loads(data.decode("utf-8", "replace"))
    except Exception:
        return None


def dep_tables(doc):
    out = {}

    def walk(tbl, prefix):
        for key in ("dependencies", "dev-dependencies", "build-dependencies"):
            for name, spec in (tbl.get(key) or {}).items():
                out[f"{prefix}{key}:{name}"] = spec
    walk(doc, "")
    walk(doc.get("workspace") or {}, "workspace.")
    for tname, t in (doc.get("target") or {}).items():
        walk(t, f"target.{tname}.")
    return out


def check_cargo(changes, repo, base, head):
    f = []
    toml_changed = [c for c in changes if os.path.basename(c.path) == "Cargo.toml" and c.status != "D"]
    for c in toml_changed:
        new_doc, old_doc = parse_toml(blob(repo, head, c.path)), parse_toml(blob(repo, base, c.old_path))
        if new_doc is None:
            f.append(Finding(INFO, "cargo-parse", "could not parse Cargo.toml (needs Python 3.11+); read it yourself", c.path))
            continue
        new, old = dep_tables(new_doc), dep_tables(old_doc or {})
        for key, spec in new.items():
            if old.get(key) == spec:
                continue
            sect, name = key.rsplit(":", 1)
            spec_s = spec if isinstance(spec, str) else ", ".join(f"{k}={v}" for k, v in spec.items())
            what = "changed" if key in old else "added"
            if isinstance(spec, dict) and ("git" in spec or "registry" in spec):
                f.append(Finding(BLOCK, "dep-source", f"dependency {name} ({sect}) {what} with a git/registry source: {spec_s}", c.path))
            elif isinstance(spec, dict) and "path" in spec and spec["path"].count("..") >= 3:
                f.append(Finding(REVIEW, "dep-source", f"dependency {name} points at a path outside the workspace: {spec['path']}", c.path))
            else:
                f.append(Finding(REVIEW, "dependency", f"dependency {name} ({sect}) {what}: {spec_s}", c.path))
        for sect in ("patch", "replace"):
            if new_doc.get(sect) and new_doc.get(sect) != (old_doc or {}).get(sect):
                f.append(Finding(BLOCK, "dep-patch", f"[{sect}] section added or changed (swaps in different code for a dependency)", c.path))
        pkg, oldpkg = new_doc.get("package") or {}, (old_doc or {}).get("package") or {}
        if pkg.get("build") != oldpkg.get("build") and pkg.get("build"):
            f.append(Finding(REVIEW, "build-script", f"package.build set to {pkg['build']}", c.path))
        if (new_doc.get("lib") or {}).get("proc-macro") and not ((old_doc or {}).get("lib") or {}).get("proc-macro"):
            f.append(Finding(REVIEW, "proc-macro", "becomes a procedural macro crate (runs code at compile time)", c.path))
    for c in changes:
        if os.path.basename(c.path) != "Cargo.lock" or c.status == "D":
            continue
        new_doc, old_doc = parse_toml(blob(repo, head, c.path)), parse_toml(blob(repo, base, c.old_path))
        if new_doc is None:
            f.append(Finding(INFO, "cargo-parse", "could not parse Cargo.lock; read it yourself", c.path))
            continue
        index = lambda d: {(p["name"], p["version"]): p for p in (d or {}).get("package", [])}
        new, old = index(new_doc), index(old_doc)
        added = sorted(k for k in new if k not in old)
        for key in added:
            src = new[key].get("source", "")
            if src and not src.startswith("registry+https://github.com/rust-lang/crates.io-index"):
                f.append(Finding(BLOCK, "lock-source", f"{key[0]} {key[1]} comes from {src}", c.path))
        for key, pkg in new.items():
            if key in old and old[key].get("checksum") != pkg.get("checksum"):
                f.append(Finding(BLOCK, "lock-checksum", f"{key[0]} {key[1]} has a different checksum than before (same version, different code)", c.path))
        if added:
            names = ", ".join(f"{n} {v}" for n, v in added[:12]) + (f" (+{len(added) - 12} more)" if len(added) > 12 else "")
            f.append(Finding(REVIEW if len(added) > 15 else INFO, "lock-added", f"{len(added)} new crate(s) in the lockfile: {names}", c.path))
        if not toml_changed:
            f.append(Finding(REVIEW, "lock-alone", "Cargo.lock changed but no Cargo.toml did", c.path))
    return f


def scan_sensitive(repo, base, head):
    script = os.path.join(ROOT, "tools", "check-sensitive.sh")
    if not os.path.exists(script):
        return [Finding(INFO, "secrets-scan", "tools/check-sensitive.sh not found; secrets were not scanned")]
    p = subprocess.run(["sh", script, "--range", f"{base}..{head}"], cwd=repo, capture_output=True, text=True)
    if p.returncode == 0:
        return []
    lines = [ln for ln in (p.stdout + p.stderr).splitlines() if ln.strip()][:25]
    return [Finding(BLOCK, "secrets-scan", "tools/check-sensitive.sh reports secrets or personal data (it prints only where, never the value)", text=" | ".join(lines))]


def check_licences(changes, repo, head):
    if not any("/System/" in c.path and c.path.endswith((".so", ".txt", ".gz", ".meta", ".patch")) for c in changes):
        return []
    script = os.path.join(ROOT, "tools", "check-pak-licences.sh")
    if not os.path.exists(script):
        return []
    with tempfile.TemporaryDirectory() as tmp:
        pak = "flip/Tools/my355/Slot.pak"
        arch = subprocess.run(["git", "archive", head, pak], cwd=repo, capture_output=True)
        if arch.returncode != 0 or subprocess.run(["tar", "-x", "-C", tmp], input=arch.stdout, capture_output=True).returncode != 0:
            return [Finding(INFO, "licences", "could not extract the pak to run the licence check")]
        p = subprocess.run(["sh", script, os.path.join(tmp, pak)], capture_output=True, text=True)
    if p.returncode == 0:
        return [Finding(INFO, "licences", "tools/check-pak-licences.sh passes on the PR's pak")]
    out = " | ".join(ln for ln in (p.stdout + p.stderr).splitlines() if ln.strip())[:400]
    return [Finding(BLOCK, "licences", "tools/check-pak-licences.sh fails on the PR's pak", text=out)]


def check_messages(repo, base, head, title="", body=""):
    f = []
    log = git_text(repo, "log", "--format=%H%x1f%an <%ae>%x1f%cn <%ce>%x1f%B%x1e", f"{base}..{head}")
    authors = set()
    for rec in log.split("\x1e"):
        parts = rec.strip("\n").split("\x1f")
        if len(parts) < 4:
            continue
        sha, author, committer, msg = parts[0][:8], parts[1], parts[2], parts[3]
        authors.add(author)
        if INJECTION.search(msg):
            f.append(Finding(REVIEW, "reviewer-injection", f"commit {sha}: message reads as instructions to an automated reviewer", text=msg))
    for label, text in (("title", title), ("description", body)):
        if INJECTION.search(text or ""):
            f.append(Finding(REVIEW, "reviewer-injection", f"PR {label} reads as instructions to an automated reviewer", text=text))
    merges = git_text(repo, "rev-list", "--merges", "--count", f"{base}..{head}").strip()
    if merges not in ("", "0"):
        f.append(Finding(INFO, "merge-commits", f"{merges} merge commit(s) inside the PR (the diff can hide in them; this review is of the net change)"))
    return f, sorted(authors)


# ------------------------------------------------------------------------ GitHub


def gh_json(path):
    p = subprocess.run(["gh", "api", path], capture_output=True, text=True)
    if p.returncode != 0:
        raise ToolError(f"gh api {path}: {p.stderr.strip()}")
    import json
    return json.loads(p.stdout)


def repo_from_remote(repo, remote):
    """OWNER/REPO from the remote's URL (works with ssh host aliases). `gh repo view`
    would answer with the upstream project in a fork, which is the wrong repo."""
    url = git_text(repo, "remote", "get-url", remote, check=False).strip()
    m = re.search(r"[:/]([^/:\s]+)/([^/\s]+?)(?:\.git)?/?$", url)
    if not m:
        raise ToolError(f"could not work out OWNER/REPO from remote {remote!r} ({url!r}); pass --repo")
    return f"{m.group(1)}/{m.group(2)}"


def days_since(iso):
    return (datetime.now(timezone.utc) - datetime.fromisoformat(iso.replace("Z", "+00:00"))).total_seconds() / 86400


def check_meta(pr, repo_name):
    f = []
    try:
        user = gh_json(f"users/{pr['user']['login']}")
        age = days_since(user["created_at"])
        if age < 30:
            f.append(Finding(REVIEW, "new-account", f"author's account is {age:.0f} days old"))
        else:
            f.append(Finding(INFO, "author", f"{pr['user']['login']}: account {age / 365:.1f} years old, {user.get('public_repos', 0)} public repos, {user.get('followers', 0)} followers"))
    except Exception as e:
        f.append(Finding(INFO, "author", f"could not look up the author: {e}"))
    if pr.get("author_association") in ("FIRST_TIME_CONTRIBUTOR", "FIRST_TIMER", "NONE"):
        f.append(Finding(INFO, "first-time", f"author association is {pr['author_association']}; their CI runs wait for your approval, and approving runs the PR's workflow code (read it first)"))
    head_repo = pr["head"].get("repo")
    if head_repo is None:
        f.append(Finding(REVIEW, "head-gone", "the PR's source repository no longer exists"))
    else:
        try:
            r = gh_json(f"repos/{head_repo['full_name']}")
            hrs = days_since(r["created_at"]) * 24
            f.append(Finding(INFO, "fork", f"source {head_repo['full_name']} was created {hrs:.0f} hours before this review"))
        except Exception:
            pass
    try:
        runs = gh_json(f"repos/{repo_name}/commits/{pr['head']['sha']}/check-runs")["check_runs"]
        bad = [r["name"] for r in runs if r["conclusion"] in ("failure", "cancelled", "timed_out")]
        waiting = [r["name"] for r in runs if r["conclusion"] == "action_required"]
        if bad:
            f.append(Finding(REVIEW, "ci-failing", "failing checks: " + ", ".join(bad)))
        if waiting:
            f.append(Finding(INFO, "ci-waiting", "checks waiting for approval: " + ", ".join(waiting)))
        if not runs:
            f.append(Finding(INFO, "ci-none", "no checks have run on this commit"))
    except Exception:
        pass
    return f


# ------------------------------------------------------------------------ report


def render(findings, ctx, cap):
    order = {BLOCK: 0, REVIEW: 1, INFO: 2}
    findings = sorted(findings, key=lambda x: (order[x.sev], x.rule, x.path, x.line))
    nb = sum(1 for x in findings if x.sev == BLOCK)
    nr = sum(1 for x in findings if x.sev == REVIEW)
    verdict = ("NOTHING WAS REVIEWED: the diff is empty" if any(x.rule == "empty-diff" for x in findings) else
               "BLOCK: do not merge until the blocking findings are explained" if nb else
               "NEEDS A HUMAN LOOK: read the files listed below" if nr else
               "NO AUTOMATED FINDINGS (this is not proof of safety; still read the diff)")
    o = [f"# PR review: {ctx['title']}", ""]
    o.append(f"Reviewed `{ctx['base'][:8]}..{ctx['head'][:8]}`: {ctx['commits']} commit(s), {len(ctx['changes'])} file(s), +{ctx['added']} / -{ctx['removed']}. Authors: {', '.join(ctx['authors']) or 'unknown'}.")
    o += ["", f"**Verdict: {verdict}**", ""]
    for sev, title in ((BLOCK, "Blocking"), (REVIEW, "Needs a human look"), (INFO, "Notes")):
        group = [x for x in findings if x.sev == sev]
        if not group:
            continue
        o.append(f"## {title} ({len(group)})")
        shown_per_rule = {}
        for x in group:
            n = shown_per_rule.get(x.rule, 0)
            shown_per_rule[x.rule] = n + 1
            if n >= cap and sev != BLOCK:
                continue
            where = f"`{x.path}{':' + str(x.line) if x.line else ''}` " if x.path else ""
            o.append(f"- **{x.rule}** {where}{x.msg}")
            if x.text:
                o.append(f"    `{snippet(x.text)}`")
        for rule, n in shown_per_rule.items():
            if n > cap and sev != BLOCK:
                o.append(f"- _{rule}: {n - cap} more not shown_")
        o.append("")
    o.append("## Files to read yourself")
    o += ["", "| change | file | lines |", "| --- | --- | --- |"]
    for c in ctx["changes"]:
        o.append(f"| {c.status} | `{visible(c.path)}` | {'binary' if c.binary else f'+{len(c.added)} / -{len(c.removed)}'} |")
    o += ["", "## What this cannot tell you", "",
          "- Whether the change is a good idea, or does what its description says.",
          "- What a binary does, or what a new dependency's source does.",
          "- Mistakes that are not malice: logic errors, races, a missed edge case.",
          "- Anything added to the PR after this run (re-run, or merge by pinned head below)."]
    if ctx.get("merge_hint"):
        o += ["", "## Merge exactly what was reviewed", "", f"```\n{ctx['merge_hint']}\n```"]
    return "\n".join(o) + "\n", (2 if nb else 1 if nr else 0)


def run(repo, base, head, ctx_extra=None, meta=None, scan=True, cap=6):
    findings, ctx = analyse(repo, base, head, ctx_extra, meta, scan)
    return render(findings, ctx, cap)


def analyse(repo, base, head, ctx_extra=None, meta=None, scan=True):
    head = git_text(repo, "rev-parse", head).strip()
    mb = git_text(repo, "merge-base", base, head).strip() or base
    changes = collect_changes(repo, mb, head)
    for c in changes[:MAX_TEXT_FILES]:
        read_hunks(repo, mb, head, c)
    findings = []
    if not changes:
        findings.append(Finding(REVIEW, "empty-diff", "no changed files between the merge base and the head (already merged, or the wrong base?). Nothing was reviewed"))
    if len(changes) > MAX_TEXT_FILES:
        findings.append(Finding(REVIEW, "large-pr", f"{len(changes)} files; only the first {MAX_TEXT_FILES} were read line by line"))
    findings += meta or []
    findings += check_paths(changes, repo, head)
    findings += check_text(changes[:MAX_TEXT_FILES], repo, head)
    findings += check_cargo(changes, repo, mb, head)
    ctx_extra = ctx_extra or {}
    msg_findings, authors = check_messages(repo, mb, head, ctx_extra.get("pr_title", ""), ctx_extra.get("pr_body", ""))
    findings += msg_findings
    if scan:
        findings += scan_sensitive(repo, mb, head)
        findings += check_licences(changes, repo, head)
    ctx = {
        "title": ctx_extra.get("pr_title") or f"{mb[:8]}..{head[:8]}",
        "base": mb, "head": head, "changes": changes, "authors": authors,
        "commits": git_text(repo, "rev-list", "--count", f"{mb}..{head}").strip(),
        "added": sum(len(c.added) for c in changes), "removed": sum(len(c.removed) for c in changes),
        "merge_hint": ctx_extra.get("merge_hint", ""),
    }
    return findings, ctx


def main(argv=None):
    ap = argparse.ArgumentParser(description="Mechanical pre-merge review of a pull request.")
    ap.add_argument("pr", nargs="?", type=int, help="pull request number")
    ap.add_argument("--range", nargs=2, metavar=("BASE", "HEAD"), help="review two local commits instead of a PR")
    ap.add_argument("--repo", help="OWNER/REPO (default: from `gh repo view`)")
    ap.add_argument("--remote", default="origin")
    ap.add_argument("--dir", default=".", help="the git checkout to work in")
    ap.add_argument("--expect-head", help="abort unless the PR head starts with this SHA")
    ap.add_argument("--no-scan", action="store_true", help="skip tools/check-sensitive.sh and the licence check")
    ap.add_argument("--max-per-rule", type=int, default=6)
    a = ap.parse_args(argv)
    try:
        if a.range:
            text, code = run(a.dir, a.range[0], a.range[1], scan=not a.no_scan, cap=a.max_per_rule)
        elif a.pr:
            repo_name = a.repo or repo_from_remote(a.dir, a.remote)
            pr = gh_json(f"repos/{repo_name}/pulls/{a.pr}")
            head_sha, base_ref = pr["head"]["sha"], pr["base"]["ref"]
            if a.expect_head and not head_sha.startswith(a.expect_head):
                raise ToolError(f"PR head is {head_sha[:8]}, not {a.expect_head}: it changed since you last looked")
            git(a.dir, "fetch", "-q", a.remote, f"+pull/{a.pr}/head:refs/review/pr-{a.pr}", f"+refs/heads/{base_ref}:refs/review/base-{a.pr}")
            fetched = git_text(a.dir, "rev-parse", f"refs/review/pr-{a.pr}").strip()
            if fetched != head_sha:
                raise ToolError(f"fetched {fetched[:8]} but GitHub says the head is {head_sha[:8]}; try again")
            hint = f"gh api -X PUT repos/{repo_name}/pulls/{a.pr}/merge -f merge_method=merge -f sha={head_sha}"
            text, code = run(a.dir, f"refs/review/base-{a.pr}", f"refs/review/pr-{a.pr}", {
                "pr_title": f"#{a.pr} {pr['title']} (@{pr['user']['login']}, into {base_ref})",
                "pr_body": pr.get("body") or "", "merge_hint": hint,
            }, meta=check_meta(pr, repo_name), scan=not a.no_scan, cap=a.max_per_rule)
        else:
            ap.error("give a PR number or --range BASE HEAD")
    except ToolError as e:
        print(f"review-pr: {e}", file=sys.stderr)
        return 3
    sys.stdout.write(text)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as fh:
            fh.write(text)
    return code


if __name__ == "__main__":
    sys.exit(main())
