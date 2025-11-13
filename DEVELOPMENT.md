asdfasdf
Alrighty, you want to get compiling. We love you already. Your parents raised
you right. Let's get started.

---

## Table of Contents

- [Overview](#overview)
- [The Basics](#the-basics)
  - [Prerequisites](#prerequisites)
  - [Install dependencies](#install-dependencies)
  - [Run the app](#run-the-app)
  - [Lint & format](#lint--format)
- [Debugging](#debugging)
  - [Logs](#logs)
  - [Repository](#Repository)
  - [Tokio](#tokio)
- [Troubleshooting](#troubleshooting)
---

## The Basics

OK, let's get it running.

### Prerequisites

First of all, this is a Tauri app, which uses Rust for the backend and Javascript for the frontend. So let's make sure you have all the prerequisites installed.

1. Tauri Dev Deps (https://tauri.app/start/prerequisites/#system-dependencies)

On Mac OS, ensure you've installed XCode and `cmake`. On Linux, if you're on Debian or one of its derivatives like Ubuntu, you can use the following command.

<details>
<summary>Linux Tauri dependencies</summary>

```bash
