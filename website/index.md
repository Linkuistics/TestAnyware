---
title: TestAnyware
---

TestAnyware is an AI-driven GUI testing framework for virtual machines. It connects to any VM over two channels: VNC for pixel-level capture and keyboard/mouse input, and an HTTP agent for accessibility-tree queries, semantic actions, command execution, and file transfer.

Platform agents for macOS (Swift/Hummingbird), Linux (Python/AT-SPI2), and Windows (C#/FlaUI) expose a unified JSON API, so the host CLI and driver library are written once and work across all three platforms. A Python vision pipeline decomposes VM screenshots into structured UI data — window detection, element classification, OCR, and icon classification — for use in LLM-driven test and automation workflows.

TestAnyware runs on a macOS 14+ host with Apple Silicon. VMs are managed via tart (macOS and Linux guests) and QEMU+swtpm (Windows 11 with TPM 2.0). Golden images are built once per platform; every test run clones from a golden for a clean starting state.
