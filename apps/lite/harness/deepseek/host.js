// oxlint-disable -- Plugin source for the harness runtime: plain JS against its globals, so the type-aware rules only ever see `any`.
// GitButler harness panel — plugin HOST half.
//
// Install: paste this file's contents as `code.host` in a `cordis_define`
// call (see README.md in this directory). It runs in the DSH host's vm
// sandbox: no imports, no require — it spawns the repo's harness CLI
// (`node apps/lite/harness/deepseek/cli.mjs`) and speaks line-delimited JSON
// over stdio. Keep this file in sync with the running package.
return {
	apply(ctx) {
		const subprocess = ctx.get("subprocess");
		const fs = ctx.get("fs");
		const sandboxPolicy = ctx.get("sandboxPolicy");
		const timer = ctx.get("timer");
		if (!subprocess || !fs) {
			console.error("GitButler panel: missing subprocess/fs services");
			return;
		}

		// The vm sandbox cannot import modules or load the NAPI SDK, so the host
		// spawns `node apps/lite/harness/deepseek/cli.mjs` and speaks line-delimited
		// JSON over stdio. The repo root is resolved per-boot from the durable
		// workspace registry (sandboxPolicy.workspaceRoot is the HARNESS checkout,
		// not the session workspace), confirmed by an fs existence probe.
		let bridge = null; // { child, call, root, project, platform, bundle }
		let bridgePromise = null; // dedupe concurrent first-boot spawns

		const resolveNode = async () => {
			try {
				return await subprocess.resolveExecutable("node");
			} catch (error) {
				for (const candidate of [
					"/usr/local/bin/node",
					"/opt/homebrew/bin/node",
					"/usr/bin/node",
				]) {
					try {
						return await subprocess.resolveExecutable(candidate);
					} catch (ignored) {
						/* try next */
					}
				}
				throw error;
			}
		};

		// Find the repo root: the client-forwarded workdir first, then every
		// durable workspace path, then the sandbox root as a last resort. The
		// first one containing apps/lite/harness/deepseek/cli.mjs wins. The
		// probe uses an absolute path (no cwd), matching the read path the fs
		// service is proven to serve from the vm sandbox.
		const findRepoRoot = async (workdir) => {
			const candidates = [];
			if (workdir) candidates.push(workdir);
			const registry = ctx.get("workspaceRegistry");
			if (registry) {
				try {
					for (const workspace of registry.list()) {
						const p = workspace && (workspace.path || workspace.cwd);
						if (typeof p === "string" && p) candidates.push(p);
					}
				} catch (error) {
					console.error("GitButler panel: workspaceRegistry.list failed", error);
				}
			}
			if (sandboxPolicy && sandboxPolicy.workspaceRoot) {
				candidates.push(sandboxPolicy.workspaceRoot);
			}
			for (const candidate of candidates) {
				try {
					const absolute = candidate + "/apps/lite/harness/deepseek/cli.mjs";
					const target = await fs.resolve(absolute);
					const info = await fs.stat(target);
					if (info && info.type === "file") return candidate;
				} catch (error) {
					console.error(
						"GitButler panel: probe failed for " + candidate,
						(error && error.message) || String(error),
					);
				}
			}
			throw new Error(
				"GitButler panel: could not locate apps/lite/harness in any workspace (tried: " +
					candidates.join(", ") +
					")",
			);
		};

		const spawnBridge = async (root) => {
			const node = await resolveNode();
			const child = subprocess.spawn({
				argv: [node, root + "/apps/lite/harness/deepseek/cli.mjs"],
				cwd: root,
				stdio: { stdin: "pipe", stdout: "pipe", stderr: { maxBytes: 262144 } },
				graceMs: 3000,
			});
			if (!child.stdin || !child.stdout)
				throw new Error("GitButler panel: bridge stdio unavailable");

			const pending = new Map();
			let buffer = "";
			let seq = 0;

			const failAll = (message) => {
				for (const entry of pending.values()) entry.reject(new Error(message));
				pending.clear();
			};

			child.stdout.on("data", (chunk) => {
				buffer += typeof chunk === "string" ? chunk : chunk.toString("utf8");
				let nl;
				while ((nl = buffer.indexOf("\n")) !== -1) {
					const line = buffer.slice(0, nl).trim();
					buffer = buffer.slice(nl + 1);
					if (!line) continue;
					let msg;
					try {
						msg = JSON.parse(line);
					} catch (ignored) {
						continue;
					}
					if (msg.type === "event") continue; // events surface through drain
					const entry = pending.get(msg.id);
					if (!entry) continue;
					pending.delete(msg.id);
					if (msg.error) entry.reject(new Error(String(msg.error)));
					// The wire is lossless JSON: never hand `undefined` to the RPC guard.
					else entry.resolve(msg.result === undefined ? null : msg.result);
				}
			});

			if (child.stderr) {
				child.stderr.on("data", (c) => {
					console.error("GitButler bridge:", String(c).trim());
				});
			}

			child.done.then(
				(outcome) => {
					failAll("GitButler bridge exited (" + String(outcome.exitCode) + ")");
					if (bridge && bridge.child === child) bridge = null;
					bridgePromise = null;
				},
				(error) => {
					failAll("GitButler bridge spawn failed: " + String((error && error.message) || error));
					if (bridge && bridge.child === child) bridge = null;
					bridgePromise = null;
				},
			);

			const call = (method, extra, timeoutMs) =>
				new Promise((resolve, reject) => {
					const id = ++seq;
					let done = false;
					let disposer = null;
					const settle = (fn, value) => {
						if (done) return;
						done = true;
						if (disposer) disposer();
						fn(value);
					};
					if (timer)
						disposer = timer.timeout(
							() => settle(reject, new Error("GitButler bridge " + method + " timed out")),
							timeoutMs,
						);
					pending.set(id, { resolve: (v) => settle(resolve, v), reject: (e) => settle(reject, e) });
					try {
						child.stdin.write(JSON.stringify({ id: id, method: method, ...extra }) + "\n");
					} catch (error) {
						pending.delete(id);
						settle(reject, error);
					}
				});

			bridge = { child: child, call: call, root: root };
			return bridge;
		};

		const ensureBridge = async (workdir) => {
			if (bridge) return bridge;
			if (!bridgePromise) {
				bridgePromise = findRepoRoot(workdir)
					.then((root) => spawnBridge(root))
					.catch((error) => {
						bridgePromise = null;
						throw error;
					});
			}
			return bridgePromise;
		};

		ctx.effect(() =>
			harness.handle("but.mount", async (args) => {
				const workdir = args && typeof args.workdir === "string" ? args.workdir : undefined;
				const b = await ensureBridge(workdir);
				if (!b.project) {
					const [project, ping] = await Promise.all([
						b.call("resolveProject", { candidates: [b.root] }, 20000),
						b.call("ping", {}, 10000),
					]);
					b.project = project;
					b.platform = (ping && ping.platform) || "darwin";
				}
				return { project: b.project, platform: b.platform };
			}),
		);

		ctx.effect(() =>
			harness.handle("but.bundle", async () => {
				const b = await ensureBridge();
				if (!b.bundle) {
					const read = async (absolute) => {
						const target = await fs.resolve(absolute);
						return fs.readText(target);
					};
					const [js, css] = await Promise.all([
						read(b.root + "/apps/lite/dist/harness/browser.js"),
						read(b.root + "/apps/lite/dist/harness/lite.css"),
					]);
					b.bundle = { js: js, css: css };
				}
				return b.bundle;
			}),
		);

		ctx.effect(() =>
			harness.handle("but.ipc", async (args) => {
				const b = await ensureBridge();
				if (!args || !args.endpoint) throw new Error("GitButler panel: but.ipc needs an endpoint");
				return b.call("invoke", { endpoint: args.endpoint, params: args.params }, 30000);
			}),
		);

		ctx.effect(() =>
			harness.handle("but.drain", async () => {
				const b = await ensureBridge();
				return b.call("drain", {}, 45000);
			}),
		);

		ctx.effect(() =>
			harness.handle("but.unmount", async () => {
				const b = await ensureBridge();
				try {
					await b.call("invoke", { endpoint: "watcherStopAll" }, 10000);
				} catch (error) {
					console.error("GitButler panel: watcherStopAll failed", error);
				}
				return null;
			}),
		);

		ctx.effect(() => () => {
			const child = bridge && bridge.child;
			if (child) {
				try {
					child.terminate();
				} catch (error) {
					console.error("GitButler panel: bridge terminate failed", error);
				}
			}
			bridge = null;
			bridgePromise = null;
		});
	},
};
