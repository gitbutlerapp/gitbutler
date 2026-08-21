// oxlint-disable -- Plugin source for the harness runtime: plain JS against its globals, so the type-aware rules only ever see `any`.
// GitButler harness panel — plugin CLIENT half.
//
// Install: paste this file's contents as `code.client` in a `cordis_define`
// call (see README.md in this directory). Plain JavaScript only (no JSX, no
// imports): it runs as an async function body in the DSH page. Keep this file
// in sync with the running package.
return {
	inject: ["slots", "layout", "timer"],
	apply(ctx) {
		const slots = ctx.slots;
		const layout = ctx.layout;
		const timer = ctx.timer;

		// ---- open state shared by the toggle, the run-card control, and the panel ----
		const store = { open: false, listeners: new Set() };
		const setOpen = (value) => {
			store.open = value;
			// Listeners are React state setters: they need the value, not a bare call.
			for (const listener of [...store.listeners]) listener(value);
		};
		const useOpen = () => {
			const [open, setState] = React.useState(store.open);
			React.useEffect(() => {
				store.listeners.add(setState);
				return () => {
					store.listeners.delete(setState);
				};
			}, []);
			return open;
		};

		// ---- runtime state ----
		const state = {
			bundle: null,
			cssInserted: false,
			createPanel: null,
			project: null,
			platform: "darwin",
			app: null,
			listeners: new Map(),
			draining: false,
			stopped: false,
			detailsDisposer: null,
		};
		const closePanel = () => {
			if (state.detailsDisposer) {
				state.detailsDisposer();
				state.detailsDisposer = null;
			}
			setOpen(false);
		};
		const openPanel = () => {
			if (store.open) return;
			if (!state.detailsDisposer) {
				// priority -1 shadows the shipped DetailsPanel; lowest renders.
				state.detailsDisposer = slots.register({ name: "details", priority: -1 }, () =>
					React.createElement(PanelBody),
				);
			}
			layout.openDetails();
			setOpen(true);
		};
		const togglePanel = () => {
			if (store.open) closePanel();
			else openPanel();
		};
		ctx.effect(() => () => {
			state.stopped = true;
			closePanel();
		});

		// ---- native-menu popup state ----
		const BUT_OPEN_IN_GITBUTLER_ID = "but:open-in-gitbutler";
		const menuState = { menu: null, listeners: new Set() };
		const setMenu = (menu) => {
			menuState.menu = menu;
			// Listeners are React state setters: they need the value, not a bare call.
			for (const listener of [...menuState.listeners]) listener(menu);
		};
		const useMenu = () => {
			const [menu, setM] = React.useState(menuState.menu);
			React.useEffect(() => {
				menuState.listeners.add(setM);
				return () => {
					menuState.listeners.delete(setM);
				};
			}, []);
			return menu;
		};
		const openNativeMenu = (payload) =>
			new Promise((resolve) => {
				// A second menu supersedes an open one: resolve the old as cancelled.
				if (menuState.menu) menuState.menu.close();
				const items = (payload && payload.items) || [];
				// The lite side hands the file path along as opaque context; the app's
				// native menus ignore it. Only the plugin appends its own action.
				const path =
					payload && payload.context && typeof payload.context.path === "string"
						? payload.context.path
						: null;
				const withGitButler =
					path !== null
						? [
								...items,
								{ _tag: "Separator" },
								{
									_tag: "Item",
									label: "Open in GitButler",
									itemId: BUT_OPEN_IN_GITBUTLER_ID,
								},
							]
						: items;
				setMenu({
					items: withGitButler,
					butPath: path,
					position: (payload && payload.position) || { x: 0, y: 0 },
					resolve: resolve,
					close: () => {
						setMenu(null);
						resolve(null);
					},
				});
			});

		// ---- accelerator display ----
		// The items arrive with Electron-style accelerator strings (the app builds
		// them for its native menus); a JS menu has no OS conversion, so render the
		// glyphs macOS users expect instead of "CommandOrControl+X".
		const ACCELERATOR_SYMBOLS = {
			CommandOrControl: "⌘",
			CmdOrCtrl: "⌘",
			Command: "⌘",
			Control: "⌃",
			Ctrl: "⌃",
			Shift: "⇧",
			Alt: "⌥",
			Option: "⌥",
			Enter: "↵",
			Return: "↵",
			Backspace: "⌫",
			Delete: "⌫",
			Escape: "⎋",
			Esc: "⎋",
			Tab: "⇥",
			Space: "␣",
			ArrowUp: "↑",
			ArrowDown: "↓",
			ArrowLeft: "←",
			ArrowRight: "→",
			PageUp: "⇞",
			PageDown: "⇟",
			Home: "↖",
			End: "↘",
		};
		const toSymbolAccelerator = (accelerator) => {
			if (typeof accelerator !== "string") return accelerator;
			return accelerator
				.split("+")
				.map((part) => ACCELERATOR_SYMBOLS[part] || part)
				.join("");
		};

		// ---- deep links ----
		const clickDeepLink = (url) => {
			const anchor = document.createElement("a");
			anchor.href = url;
			anchor.style.display = "none";
			document.body.append(anchor);
			anchor.click();
			anchor.remove();
		};
		// Open the real GitButler (lite) app at this file in the uncommitted list.
		const openInGitButler = async (menu) => {
			const path = menu && menu.butPath;
			const project = await ensureProject();
			if (!path || !project) return;
			clickDeepLink(
				"but://app/project/" +
					encodeURIComponent(project.id) +
					"/workspace?active=uncommitted&uncommitted=" +
					encodeURIComponent(path),
			);
		};

		// ---- open-in-editor via deep link ----
		// The SDK's openInProgram cannot launch editors from this harness (it
		// resolves against the GitButler app bundle and fails), so vscode-family
		// editors are opened through their URL scheme instead. Other editors are
		// a no-op for now.
		const openInProgramBrowser = async (payload) => {
			const p = payload || {};
			if (p.programId !== "vscode") return null;
			const project = await ensureProject();
			if (!project || typeof p.path !== "string") return null;
			const absolute = await transport.invoke("pathJoin", project.path, p.path);
			clickDeepLink("vscode://file/" + absolute + (p.lineNr != null ? ":" + p.lineNr : ""));
			return null;
		};

		// ---- bundle + project ----
		const ensureBundle = async () => {
			if (!state.bundle) {
				const bundle = await host.call("but.bundle");
				state.bundle = bundle;
				if (!state.cssInserted && bundle.css) {
					try {
						styles.insert(bundle.css);
						state.cssInserted = true;
					} catch (error) {
						console.error("GitButler panel: css insert failed", error);
					}
				}
			}
			return state.bundle;
		};
		const ensureProject = async () => {
			if (!state.project) {
				const mounted = await host.call("but.mount");
				state.project = mounted && mounted.project;
				state.platform = (mounted && mounted.platform) || "darwin";
			}
			return state.project;
		};

		// ---- the transport createLiteApi consumes ----
		const ensureDrain = () => {
			if (state.draining || state.stopped) return;
			state.draining = true;
			void (async () => {
				try {
					// A healthy drain long-polls, which paces this loop. A failing
					// one returns at once, so back off instead of spinning: growing
					// to a few seconds while the bridge is down, reset once it answers.
					let failures = 0;
					while (state.listeners.size > 0 && !state.stopped) {
						let events = [];
						try {
							events = await host.call("but.drain");
							failures = 0;
						} catch (error) {
							console.error("GitButler panel: drain failed", error);
							failures += 1;
							await new Promise((resolve) =>
								setTimeout(resolve, Math.min(250 * 2 ** failures, 5000)),
							);
						}
						if (state.listeners.size === 0 || state.stopped) break;
						for (const event of events || []) {
							const set = state.listeners.get(event.channel);
							if (!set) continue;
							for (const listener of [...set]) {
								try {
									listener(event.payload);
								} catch (error) {
									console.error("GitButler panel: watcher listener failed", error);
								}
							}
						}
					}
				} finally {
					state.draining = false;
				}
			})();
		};

		const transport = {
			get platform() {
				return state.platform;
			},
			invoke: async (channel, ...args) => {
				if (channel === "showNativeMenu") return openNativeMenu(args[0]);
				if (channel === "openInProgram") return openInProgramBrowser(args[0]);
				// pathJoin is variadic; everything else is 0- or 1-arg.
				let params = channel === "pathJoin" ? args : args.length <= 1 ? args[0] : args;
				// The wire is lossless JSON: undefined must not ride along, either as
				// the whole payload or inside an object param (a JSON round-trip drops
				// undefined keys and coerces NaN, so the arg validates losslessly).
				if (params !== undefined) params = JSON.parse(JSON.stringify(params));
				return host.call(
					"but.ipc",
					params === undefined ? { endpoint: channel } : { endpoint: channel, params: params },
				);
			},
			subscribe: (channel, listener) => {
				let set = state.listeners.get(channel);
				if (!set) {
					set = new Set();
					state.listeners.set(channel, set);
				}
				set.add(listener);
				ensureDrain();
				return () => {
					set.delete(listener);
					if (set.size === 0) state.listeners.delete(channel);
				};
			},
		};

		// ---- the details-column panel body ----
		const PanelBody = () => {
			const ref = React.useRef(null);
			const [error, setError] = React.useState(null);
			React.useEffect(() => {
				let cancelled = false;
				const container = ref.current;
				if (!container) return undefined;
				void (async () => {
					try {
						const [bundle, project] = await Promise.all([ensureBundle(), ensureProject()]);
						if (cancelled || !project || !bundle) return;
						// The IIFE text + appended return yields the createPanel FUNCTION
						// (vite lib mode exports the default directly), which is then
						// called with the transport. The bundle's React 19 + its own root
						// are bundled in and never touch the page's React 18.
						if (!state.createPanel) {
							state.createPanel = new Function(bundle.js + "\n;return createButPanel;")();
						}
						const app = state.createPanel({ transport, projectId: state.project.id, params: {} });
						state.app = app;
						app.mount(container);
					} catch (error) {
						console.error("GitButler panel: mount failed", error);
						if (!cancelled) setError(String((error && error.message) || error));
					}
				})();
				return () => {
					cancelled = true;
					if (state.app) {
						try {
							state.app.unmount();
						} catch (error) {
							console.error("GitButler panel: unmount failed", error);
						}
						state.app = null;
					}
					try {
						void host.call("but.unmount");
					} catch (error) {
						console.error("GitButler panel: unmount call failed", error);
					}
				};
			}, []);
			return React.createElement(
				"div",
				{ ref: ref, style: { width: "100%", height: "100%", minHeight: 0, overflow: "hidden" } },
				error
					? React.createElement(
							"div",
							{
								style: {
									padding: 12,
									fontFamily: "ui-monospace, monospace",
									fontSize: 12,
									color: "#e5484d",
								},
							},
							String(error),
						)
					: null,
			);
		};

		// ---- the per-session header toggle (session header action row) ----
		const Toggle = (props) => {
			const open = useOpen();
			return React.createElement(
				"button",
				{
					onClick: togglePanel,
					title: "GitButler workspace",
					style: {
						display: "inline-flex",
						alignItems: "center",
						gap: 6,
						border: "1px solid color-mix(in srgb, currentColor 25%, transparent)",
						borderRadius: 8,
						padding: "4px 9px",
						background: open ? "color-mix(in srgb, currentColor 12%, transparent)" : "transparent",
						color: "inherit",
						fontSize: 12,
						fontWeight: 600,
						cursor: "pointer",
					},
				},
				open ? "GitButler ✓" : "GitButler",
			);
		};

		// ---- the run-card control (tool.view.cordis) ----
		const RunCard = (props) => {
			const open = useOpen();
			return React.createElement(
				"div",
				{ style: { display: "flex", alignItems: "center", gap: 10, padding: "4px 0" } },
				React.createElement(
					"span",
					{ style: { fontSize: 12, opacity: 0.75 } },
					"GitButler workspace panel",
				),
				React.createElement(
					"button",
					{
						onClick: togglePanel,
						style: {
							border: "1px solid color-mix(in srgb, currentColor 25%, transparent)",
							borderRadius: 8,
							padding: "4px 10px",
							background: open
								? "color-mix(in srgb, currentColor 12%, transparent)"
								: "transparent",
							color: "inherit",
							fontSize: 12,
							fontWeight: 600,
							cursor: "pointer",
						},
					},
					open ? "Close panel" : "Open panel",
				),
			);
		};

		// ---- native-menu popup (showNativeMenu bridge) ----
		const NativeMenuOverlay = () => {
			const menu = useMenu();
			if (!menu) return null;
			return React.createElement(NativeMenu, { menu: menu, timer: timer });
		};

		const NativeMenu = ({ menu, timer: timerSvc }) => {
			const rootRef = React.useRef(null);
			const openPath = React.useRef([]);
			const [forceCount, force] = React.useState(0);
			const closeTimerRef = React.useRef(null);
			const itemRefs = React.useRef(new Map());
			const submenuRef = React.useRef(null);
			const [submenuLayout, setSubmenuLayout] = React.useState(null);
			const [rootLayout, setRootLayout] = React.useState(null);

			const cancelTimer = () => {
				if (closeTimerRef.current) {
					closeTimerRef.current();
					closeTimerRef.current = null;
				}
			};
			const armTimer = (fn) => {
				cancelTimer();
				closeTimerRef.current = timerSvc.timeout(() => {
					closeTimerRef.current = null;
					fn();
				}, 120);
			};

			React.useEffect(() => () => cancelTimer(), []);
			React.useEffect(() => {
				const onKey = (e) => {
					if (e.key === "Escape") menu.close();
				};
				const onPointer = (e) => {
					if (rootRef.current && !rootRef.current.contains(e.target)) menu.close();
				};
				document.addEventListener("keydown", onKey, true);
				document.addEventListener("pointerdown", onPointer, true);
				window.addEventListener("blur", menu.close);
				return () => {
					document.removeEventListener("keydown", onKey, true);
					document.removeEventListener("pointerdown", onPointer, true);
					window.removeEventListener("blur", menu.close);
				};
			}, [menu]);

			// Clamp the root menu into the viewport once it has rendered.
			React.useLayoutEffect(() => {
				const root = rootRef.current;
				if (!root) return;
				const rect = root.getBoundingClientRect();
				const margin = 4;
				setRootLayout({
					left: Math.max(margin, Math.min(rect.left, window.innerWidth - rect.width - margin)),
					top: Math.max(margin, Math.min(rect.top, window.innerHeight - rect.height - margin)),
				});
			}, [menu]);

			// Measure the deepest open submenu and clamp it into the viewport:
			// flip to the left when it would cross the right edge, clamp top so it
			// never leaves the bottom.
			React.useLayoutEffect(() => {
				const key = openPath.current.join("/");
				const submenu = submenuRef.current;
				const itemEl = itemRefs.current.get(key);
				if (!submenu || !itemEl) {
					setSubmenuLayout(null);
					return;
				}
				const margin = 4;
				const ir = itemEl.getBoundingClientRect();
				const sr = submenu.getBoundingClientRect();
				const flip =
					ir.right + sr.width > window.innerWidth - margin && ir.left - sr.width >= margin;
				const left = Math.max(
					margin,
					Math.min(flip ? ir.left - sr.width : ir.right, window.innerWidth - sr.width - margin),
				);
				const top = Math.max(margin, Math.min(ir.top - 5, window.innerHeight - sr.height - margin));
				setSubmenuLayout({ left: left, top: top });
			}, [forceCount]);

			const itemBase = {
				display: "flex",
				alignItems: "center",
				gap: 10,
				padding: "5px 10px",
				borderRadius: 5,
				cursor: "default",
				position: "relative",
			};
			const hoverBg = "color-mix(in srgb, currentColor 14%, transparent)";
			const submenuBase = {
				minWidth: 190,
				background: "Canvas",
				color: "CanvasText",
				border: "1px solid color-mix(in srgb, currentColor 30%, transparent)",
				borderRadius: 8,
				boxShadow: "0 8px 24px rgba(0,0,0,.3)",
				padding: 4,
			};

			const renderItems = (items, depth) =>
				items.map((item, index) => {
					if (item._tag === "Separator") {
						return React.createElement("div", {
							key: depth + "-" + index,
							style: {
								height: 1,
								background: "color-mix(in srgb, currentColor 20%, transparent)",
								margin: "4px 6px",
							},
						});
					}
					const path = [...openPath.current.slice(0, depth), index];
					const key = path.join("/");
					const isOpen = depth < openPath.current.length && openPath.current[depth] === index;
					const enabled = item.enabled !== false;
					const hasSubmenu = Array.isArray(item.submenu) && item.submenu.length > 0;
					const isDeepest = depth === openPath.current.length - 1;
					return React.createElement(
						"div",
						{
							key: key,
							ref: (el) => {
								if (el) itemRefs.current.set(key, el);
								else itemRefs.current.delete(key);
							},
							style: {
								...itemBase,
								opacity: enabled ? 1 : 0.4,
								background: isOpen ? hoverBg : undefined,
							},
							onMouseEnter: () => {
								if (!enabled) return;
								if (hasSubmenu) {
									armTimer(() => {
										openPath.current = path;
										force((n) => n + 1);
									});
								} else {
									armTimer(() => {
										openPath.current = openPath.current.slice(0, depth);
										force((n) => n + 1);
									});
								}
							},
							onClick: () => {
								if (!enabled) return;
								if (item.itemId) {
									// The plugin-only action resolves the lite side with an id it
									// has no handler for (a no-op there) and does its own work.
									if (item.itemId === BUT_OPEN_IN_GITBUTLER_ID) void openInGitButler(menu);
									menu.resolve(item.itemId);
									menu.close();
								}
							},
						},
						React.createElement("span", { style: { flex: 1 } }, item.label),
						item.accelerator
							? React.createElement(
									"span",
									{
										style: {
											color: "color-mix(in srgb, currentColor 60%, transparent)",
											fontSize: 11,
											letterSpacing: "0.04em",
										},
									},
									toSymbolAccelerator(item.accelerator),
								)
							: null,
						hasSubmenu ? React.createElement("span", { style: { fontSize: 10 } }, "▸") : null,
						isOpen && hasSubmenu
							? React.createElement(
									"div",
									{
										ref: isDeepest ? (el) => (submenuRef.current = el) : undefined,
										style:
											isDeepest && submenuLayout
												? {
														...submenuBase,
														position: "fixed",
														left: submenuLayout.left,
														top: submenuLayout.top,
													}
												: { ...submenuBase, position: "absolute", left: "100%", top: -5 },
										onMouseEnter: () => cancelTimer(),
									},
									...renderItems(item.submenu, depth + 1),
								)
							: null,
					);
				});

			const pos = menu.position;
			return React.createElement(
				"div",
				{
					ref: rootRef,
					style: {
						position: "fixed",
						left: rootLayout ? rootLayout.left : pos.x,
						top: rootLayout ? rootLayout.top : pos.y,
						zIndex: 2147483000,
						minWidth: 200,
						background: "Canvas",
						color: "CanvasText",
						border: "1px solid color-mix(in srgb, currentColor 30%, transparent)",
						borderRadius: 8,
						boxShadow: "0 8px 24px rgba(0,0,0,.3)",
						padding: 4,
						fontFamily: "system-ui, sans-serif",
						fontSize: 13,
					},
				},
				...renderItems(menu.items, 0),
			);
		};

		ctx.effect(() =>
			slots.inject("conversation.session.header.actions", () =>
				slots.register(
					{
						name: "conversation.session.header.actions",
						id: "gitbutler-panel-toggle",
						order: 0,
						label: "GitButler",
					},
					(props) => React.createElement(Toggle, props),
				),
			),
		);
		ctx.effect(() =>
			slots.inject("tool.view.cordis", () =>
				slots.register({ name: "tool.view.cordis", key: "self" }, (props) =>
					React.createElement(RunCard, props),
				),
			),
		);
		ctx.effect(() =>
			slots.inject("shell.overlay", () =>
				slots.register({ name: "shell.overlay", id: "gitbutler-native-menu" }, () =>
					React.createElement(NativeMenuOverlay),
				),
			),
		);
	},
};
