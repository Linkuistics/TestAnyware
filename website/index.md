---
title: TestAnyware
---

TestAnyware is an AI-driven GUI testing framework for virtual machines. It connects to any VM over two channels. VNC carries pixel-level capture and keyboard and mouse input. An HTTP agent answers accessibility-tree queries and runs semantic actions, commands and file transfers.

Platform agents for macOS (Swift/Hummingbird), Linux (Python/AT-SPI2) and Windows (C#/FlaUI) expose one JSON API. So one host CLI and one driver library work across all three platforms. A Python vision pipeline turns VM screenshots into structured UI data for LLM-driven test and automation workflows. Its stages are window detection, element classification, OCR and icon classification.

TestAnyware runs on a macOS 14+ host with Apple Silicon. tart manages the macOS and Linux guests. QEMU with swtpm manages the Windows 11 guests, with TPM 2.0. TestAnyware builds a golden image once per platform. Every test run clones a golden, so each run starts from a clean state.
