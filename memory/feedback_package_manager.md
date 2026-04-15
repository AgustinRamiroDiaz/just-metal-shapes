---
name: Use uv for Python packages
description: Always use uv (not pip/pip3) to install Python packages in this project
type: feedback
---

Use `uv` to install Python packages, not `pip3` or `.venv/bin/pip`.

**Why:** User explicitly corrected pip/pip3 usage twice and said to use uv.

**How to apply:** For any Python package installation in this project, use `uv add <package>` or `uv run` commands.
