# DEVCAT

A self-contained snapshot and context tool for your **AI development workflow**.

-----

## Overview

We've all been there. You're deep in an **LLM project** or **machine learning development**, juggling code, prompts, and a chaotic mess of trial-and-error. Git feels too heavy for these quick, experimental loops. Your terminal history is a graveyard of forgotten commands, and your text editor is a minefield of unsaved changes.

That's why I made **DevCat**. It's a micro-version control system built for how we actually work. No branches, no commits, just instant, clean snapshots of your project. It's a lifesaver when you need to quickly save a good state, revert to a previous version, or grab a full code context to feed back to an **AI model**. DevCat gets out of your way and gives you a fighting chance in the wild world of **AI development**.

-----

## Features

  - **Snapshot & Revert**: Take a snapshot of your project with a simple message. This is essential for quickly saving states during **AI model tuning** or **prompt engineering**. Later, you can jump back to any of those snapshots instantly if things go sideways.
  - **Context Generation**: Ever need to give an **LLM** a full view of a module or your entire project? DevCat concatenates all your relevant code into a single stream, ready for copy/pasting or piping. This is perfect for providing **AI models** with the context they need.
  - **Diffing**: Compare snapshots to see what changed, or check the difference between your last saved state and your current work.
  - **Trace Extraction**: Pipe in a stack trace from your application, and DevCat will fetch the exact code snippets from each file reference, giving you instant debugging context.
  - **Auto-Watch**: Let DevCat handle the saves for you. It can watch your files and automatically create snapshots when it detects changes.
  - **Zero-Config**: Just download it and run it. No complex setup or remote servers. It just works.

-----

## Installation

[DevCat](https://github.com/gni/devcat) is a Rust binary.

### From Source

```bash
git clone https://github.com/gni/devcat.git
cd devcat
cargo install --path .
```

### With cargo
```bash
cargo install devcat
```

-----

## Usage

Here’s a quick rundown of the essential commands.

### `devcat save <message>`

Save a new snapshot of the current project state.

```bash
# Save your current progress
devcat save "refactored the prompt engineering logic"

# Save a snapshot but exclude the 'data' folder
devcat save "experimenting with new data loaders" --exclude 'data/'
```

### `devcat log`

Show a history of all saved snapshots.

```bash
devcat log
```

Output:

```text
ID  TIMESTAMP              MESSAGE
--- ---------------------- --------------------------------------------------
1   2025-08-04 17:01:44    Initial commit
2   2025-08-04 17:05:21    Refactored the core loop
3   2025-08-04 17:12:03    Implemented new AI logic
```

### `devcat` (Default Cat)

This is the default command. It concatenates all files in the current directory (or a specified path) into a single output. It's perfect for giving context to an LLM.

```bash
# Send all files in the current directory to your LLM
devcat | pbcopy

# Concatenate files from a specific module
devcat ./src/my_module
```

### `devcat --id <id>`

Concatenate all files from a specific snapshot.

```bash
# Re-run a prompt with the state from snapshot 2
devcat --id 2 | some_llm_cli "debug this"
```

### `devcat diff <id1> [id2]`

Show the differences between two snapshots or between a snapshot and your current working directory.

```bash
# Compare the latest snapshot with your current work
devcat diff
```

### `devcat revert <id>`

Revert the working directory to the state of a specific snapshot.

```bash
# Revert to a stable state
devcat revert 2
```

### `devcat trace`

Pipes in a stack trace and outputs the referenced code snippets.

```bash
# Get context for a Python stack trace
python my_script.py 2>&1 | devcat trace
```

### `devcat watch`

Starts a file watcher that automatically creates a new snapshot whenever files are changed.

```bash
# Don't think about it, just code. DevCat will handle the snapshots.
devcat watch
```

### `devcat module <path>`

Concatenates all files in a specific module directory.

```bash
# Concatenate all files in the src directory
devcat module ./src
```

### `devcat inspect <id>`

Shows a list of all files in a specific snapshot.

```bash
# See what files were included in snapshot 5
devcat inspect 5
```

### `devcat prune --keep <n>`

Deletes old snapshots, keeping only the N most recent ones.

```bash
# Keep only the last 5 snapshots
devcat prune --keep 5
```

**Safety**: Only removes old snapshot data from `.devcat/objects/`. Your project files are never affected.

### `devcat clean [-f]`

Deletes the entire `.devcat` directory and all snapshots.

```bash
# Delete all snapshots (prompts for confirmation)
devcat clean

# Delete all snapshots without confirmation
devcat clean -f
```

**Safety**: 
- Only removes `.devcat/` directory (snapshot data)
- Never deletes your project files
- Prompts for confirmation unless `-f` flag is used

### `devcat split`

Splits a markdown file with fenced code blocks into individual source files.

```bash
# Split a markdown file into separate source files
devcat split --input docs.md --outdir output/

# Dry run to see what would be created
devcat split --input docs.md --outdir output/ --dry-run
```

-----

## Configuration

For now, DevCat is mostly plug-and-play. If you want to permanently exclude files or directories, you can create a `.devcatrc` file in the root of your project.

### `.devcatrc`

```toml
# Exclude the 'target' and 'node_modules' directories globally
exclude = ["target/", "node_modules/"]
```

-----

## Contributing

DevCat is an open-source project. If you have an idea, a bug report, or a feature request, feel free to open an issue or submit a pull request.

## Authors

[Lucian BLETAN](https://github.com/gni) - initialized project