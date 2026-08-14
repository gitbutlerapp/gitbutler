import { Dialog } from "@base-ui/react";
import { Suspense, useState, type FC } from "react";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import styles from "./Settings.module.css";
import {
	defaultSettingsPageKey,
	externalLinks,
	settingsPagesInScope,
	settingsScopes,
	type SettingsPageKey,
} from "./pages.ts";
import { Appearance } from "./Appearance.tsx";
import { Ai } from "./Ai.tsx";
import { General } from "./General.tsx";
import { Git } from "./Git.tsx";
import { Integrations } from "./Integrations.tsx";
import { Project } from "./Project.tsx";
import { ProjectAi } from "./ProjectAi.tsx";
import { ProjectExperimental } from "./ProjectExperimental.tsx";
import { ProjectGit } from "./ProjectGit.tsx";

/**
 * Keyed by the registry's own keys, so a page without a component is a type error.
 * Every page is handed the project; global ones declare no props and ignore it, which
 * is what lets one record hold both scopes.
 */
const pageContent: Record<SettingsPageKey, FC<{ projectId: string }>> = {
	"global:general": General,
	"global:appearance": Appearance,
	"global:ai": Ai,
	"global:git": Git,
	"global:integrations": Integrations,
	"project:project": Project,
	"project:ai": ProjectAi,
	"project:git": ProjectGit,
	"project:experimental": ProjectExperimental,
};

type Props = {
	open: boolean;
	/** Which page to open on. */
	page?: SettingsPageKey;
	projectId: string;
	/** Names the project group in the sidebar. */
	projectName: string;
	onOpenChange: (open: boolean) => void;
};

export const Settings: FC<Props> = (p) => {
	// Seeded once, which is correct because the dialog is mounted only while open — every
	// open is a fresh mount, and so re-reads the requested page.
	const [selected, setSelected] = useState<SettingsPageKey>(p.page ?? defaultSettingsPageKey);

	const groups = settingsScopes
		.map((scope) => ({ scope, pages: settingsPagesInScope(scope) }))
		.filter((group) => group.pages.length > 0);

	// A lone group has nothing to be told apart from.
	const showHeadings = groups.length > 1;

	const Content = pageContent[selected];

	return (
		<Dialog.Root open={p.open} onOpenChange={p.onOpenChange}>
			<Dialog.Portal>
				<Dialog.Backdrop className={styles.backdrop} />
				<Dialog.Viewport className={styles.viewport}>
					<Dialog.Popup aria-labelledby="settings-heading" className={styles.popup}>
						<nav aria-label="Settings pages" className={styles.sidebar}>
							<h1 id="settings-heading" className={classes("text-14", "text-bold", styles.heading)}>
								Settings
							</h1>

							{groups.map((group) => (
								<div key={group.scope} className={styles.group}>
									{showHeadings && (
										<h2 className={classes("text-12", styles.groupHeading)}>
											{group.scope === "global" ? "Application" : p.projectName}
										</h2>
									)}

									{group.pages.map((page) => (
										<button
											key={page.key}
											type="button"
											aria-current={page.key === selected ? "page" : undefined}
											className={classes(
												"text-13",
												"text-semibold",
												styles.link,
												page.key === selected && styles.linkSelected,
											)}
											onClick={() => setSelected(page.key)}
										>
											<Icon name={page.icon} className={styles.linkIcon} />
											<span>{page.label}</span>
										</button>
									))}
								</div>
							))}

							<div className={styles.social}>
								{externalLinks.map((link) => (
									<button
										key={link.label}
										type="button"
										className={classes("text-13", "text-semibold", styles.link)}
										onClick={() => void window.lite.openInWebBrowser(link.url)}
									>
										<Icon name={link.icon} className={styles.linkIcon} />
										<span>{link.label}</span>
										<span aria-hidden className={styles.linkExternal}>
											↗
										</span>
									</button>
								))}
							</div>
						</nav>

						<div className={styles.content}>
							<div className={styles.contentColumn}>
								<Suspense fallback={<div className="text-13">Loading…</div>}>
									<Content projectId={p.projectId} />
								</Suspense>
							</div>
						</div>
					</Dialog.Popup>
				</Dialog.Viewport>
			</Dialog.Portal>
		</Dialog.Root>
	);
};
