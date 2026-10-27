# DEVCAT

A lightweight local snapshot and code context tool for rapid experimentation.

---

## Overview

Git is great, until you're in the middle of a messy experiment and breaking things every five minutes. Nobody wants to pollute their commit history with twenty `"wip"` commits just to save a working state before trying something dumb.

I built **DevCat** for those fast trial-and-error loops. There are no branches and no staging areas just instant local snapshots of your working directory. If an experiment goes sideways, you can diff or revert to a known good state in a second.

It also handles a few annoying day-to-day chores: dumping a directory of files into a single text stream (useful for piping into other CLI tools or copying to your clipboard), pulling exact code snippets out of a crash stack trace, and unpacking markdown code blocks into actual files.

---

## Features

* **Snapshots & Rollbacks**: Save the current state of your project with a quick note and revert to it anytime.
* **File Concatenation**: Dump your project or a specific module into a single stream for easy piping or copy-pasting.
* **Diffing**: Compare any two snapshots, or check what you've changed since your last save.
* **Stack Trace Extraction**: Pipe in a crash log and get the exact source code lines referenced in the trace.
* **Auto-Watch**: Watch your directory and automatically take snapshots whenever files are modified.
* **Zero Setup**: Everything lives locally in a `.devcat` folder. Drop the binary in your PATH and run it.

---

## Installation

[DevCat](https://github.com/gni/devcat) is written in Rust.

### From Source

```bash
git clone https://github.com/gni/devcat.git
cd devcat
cargo install --path .
```

### With Cargo

```bash
cargo install devcat
```

---

## Usage

### `devcat save <message>`

Take a snapshot of the current directory.

```bash
# Save current progress
devcat save "parser works, about to refactor"

# Save a snapshot but ignore the 'data' folder
devcat save "testing new data loader" --exclude 'data/'

```

### `devcat log`

List all saved snapshots.

```bash
devcat log
```

Output:

```text
ID  TIMESTAMP              MESSAGE
--- ---------------------- --------------------------------------------------
1   2025-08-04 17:01:44    Initial state
2   2025-08-04 17:05:21    Refactored the core loop
3   2025-08-04 17:12:03    Fixed off-by-one error in parser

```

### `devcat` (Default Command)

Running `devcat` with no arguments concatenates all files in the current directory (or a given path) to `stdout`.

```bash
# Copy all project files to the clipboard
devcat | pbcopy

# Concatenate files from a specific directory
devcat ./src/my_module
```

### `devcat --id <id>`

Output the concatenated files from a previous snapshot without touching your current working files.

```bash
# Pipe the code from snapshot 2 into another tool
devcat --id 2 | grep "TODO"
```

### `devcat diff [id1] [id2]`

Show changes between snapshots, or between the latest snapshot and your working directory.

```bash
# See what changed since the last snapshot
devcat diff
```

### `devcat revert <id>`

Restore your working directory to a specific snapshot.

```bash
# Roll back to snapshot 2
devcat revert 2
```

### `devcat trace`

Read a stack trace from `stdin` and print the relevant code blocks from the referenced files.

```bash
# Grab the code surrounding a Python crash
python my_script.py 2>&1 | devcat trace
```

### `devcat watch`

Run a file watcher that automatically creates a snapshot whenever you save changes.

```bash
devcat watch
```

### `devcat module <path>`

Concatenate all files inside a specific module directory.

```bash
devcat module ./src
```

### `devcat inspect <id>`

List all files tracked in a specific snapshot.

```bash
devcat inspect 5
```

### `devcat prune --keep <n>`

Delete older snapshots, keeping only the `n` most recent ones. This only cleans up `.devcat/objects/` and leaves your working files untouched.

```bash
# Keep only the last 5 snapshots
devcat prune --keep 5

```

### `devcat clean [-f]`

Wipe the `.devcat` directory and delete all snapshot history. Your actual project files are not affected.

```bash
# Delete all snapshots (asks for confirmation)
devcat clean

# Force delete without confirmation
devcat clean -f
```

### `devcat split`

Extract fenced code blocks from a markdown file and write them out as individual source files.

```bash
# Unpack code blocks from docs.md into the output/ folder
devcat split --input docs.md --outdir output/

# Preview what files would be created without writing them
devcat split --input docs.md --outdir output/ --dry-run
```

---

## Configuration

DevCat works out of the box without configuration. If you have directories you always want to ignore, drop a `.devcatrc` file in your project root:

```toml
# Ignore build artifacts and dependencies
exclude = ["target/", "node_modules/"]
```

---

## Contributing

Bug reports, feature requests, and pull requests are welcome. Feel free to open an issue if you run into any problems.

## Authors

[Lucian BLETAN](https://github.com/gni)