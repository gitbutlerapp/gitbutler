import { defineConfig } from "@playwright/test";
import path from "node:path";
import type { TestOptions } from "./test.ts";

const liteRoot = path.resolve(import.meta.dirname, "..");

// Screenshot capture inside the container built by e2e/docker/Dockerfile.
//
// Deliberately a second config rather than options folded into playwright.config.ts:
// everything here is either meaningless or actively harmful off the container.
// Forcing software rasterisation on a developer's machine would degrade the very
// images a reviewer judges the change by, and --no-sandbox is a container
// concession, not something to normalise for local runs.
export default defineConfig<TestOptions>({
	testDir: "./tests",
	// Capture only. The functional specs keep running under the host config.
	testMatch: "screenshots.spec.ts",
	// See the host config: once per run, not once per worker.
	globalSetup: "./clear-screenshots.ts",
	// Inside the one directory the container shares with the host, so the failure
	// attachments from e2e/test.ts — the Electron main- and renderer-process logs —
	// survive the container exiting. It cannot be a mount point of its own:
	// Playwright clears this directory on start, and rm on a mount point fails
	// with EBUSY. A sibling of the capture directories is harmless, since
	// compare-screenshots.mjs is handed the two it should read by name.
	outputDir: "./screenshots/_diagnostics",
	// One window at a time. Two Electron instances sharing a single Xvfb display
	// contend for the compositor, and a capture then races another window's
	// repaint — which is indistinguishable from the surface being unchanged.
	workers: 1,
	fullyParallel: false,
	// Relaunching Electron from scratch is the one thing that has ever recovered a
	// renderer that came up without painting, and a missing surface costs a whole
	// second capture run to notice.
	retries: 2,
	webServer: {
		command: "pnpm dev:ui",
		cwd: liteRoot,
		url: "http://127.0.0.1:5173",
		reuseExistingServer: true,
		stdout: "pipe",
	},
	use: {
		// Despite the name this does not make Electron headless — e2e/test.ts only
		// forwards it as GITBUTLER_LITE_HEADLESS, which the app uses to set a macOS
		// dock policy and to skip installing the React and Redux DevTools
		// extensions. Skipping them is what we want: the install reaches out to the
		// network mid-launch, which is neither deterministic nor any use to a
		// screenshot. The window is real either way, because Xvfb is a real X server.
		headless: true,
		electronArgs: [
			// Deliberately no --no-sandbox. The container runs as an ordinary user
			// (see docker/run-capture.sh), so nothing requires it — and it actively
			// breaks the app: with the sandbox waived, renderers created by a reload
			// come up without startup data and die on
			// "Cannot destructure property 'preloadScripts' of 'binding.startupData'",
			// logging "sandboxed_renderer.bundle.js script failed to run". Every
			// surface this catalogue reaches by reloading failed on exactly that.
			//
			// There is no GPU behind Xvfb, so make the fallback explicit rather than
			// leave the choice to a probe that can answer differently on different
			// kernels. --in-process-gpu is deliberately absent too: it collapses the
			// process model that the preload path above depends on.
			"--disable-gpu",
			// The three below exist so the same commit renders to the same bytes.
			// Without them a run can differ from the previous one by a hair of
			// antialiasing, and every surface then reports as changed.
			"--force-device-scale-factor=1",
			"--disable-lcd-text",
			"--font-render-hinting=none",
			// Transitions are the one remaining source of a capture that depends
			// on when the shutter opened rather than on what the UI shows.
			"--force-prefers-reduced-motion",
		],
	},
});
