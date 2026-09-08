import { interactiveLoginShellEnvironment } from "@gitbutler/but-sdk";

// Overlap shell startup with main-module evaluation as well as Electron startup.
const shellEnvironment = interactiveLoginShellEnvironment();
const { start } = await import("./main.js");
// Electron waits for entrypoint evaluation before emitting ready.
void start(shellEnvironment);
