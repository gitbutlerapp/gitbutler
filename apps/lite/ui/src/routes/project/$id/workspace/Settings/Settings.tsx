import { Modal } from "@gitbutler/ui-react/Popup.tsx";
import { Suspense, useState, type FC } from "react";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import styles from "./Settings.module.css";
import {
	defaultSettingsPageKey,
	settingsPagesInScope,
	settingsScopes,
	type SettingsPageKey,
} from "./pages.ts";
import { Appearance } from "./Appearance.tsx";
import { Ai } from "./Ai.tsx";
import { Experimental } from "./Experimental.tsx";
import { General } from "./General.tsx";
import { Git } from "./Git.tsx";
import { Integrations } from "./Integrations.tsx";
import { Project } from "./Project.tsx";
import { ProjectAi } from "./ProjectAi.tsx";
import { ProjectExperimental } from "./ProjectExperimental.tsx";
import { Worktrees } from "./Worktrees.tsx";
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
	"global:experimental": Experimental,
	"project:project": Project,
	"project:ai": ProjectAi,
	"project:git": ProjectGit,
	"project:worktrees": Worktrees,
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

	const Content = pageContent[selected];

	return (
		<Modal
			size="medium"
			recessed
			open={p.open}
			onOpenChange={p.onOpenChange}
			aria-labelledby="settings-heading"
			className={styles.popup}
		>
			<nav aria-label="Settings pages" className={styles.sidebar}>
				<h1 id="settings-heading" className={classes("text-16", "text-semibold", styles.heading)}>
					Settings
				</h1>

				{groups.map((group) => (
					<div key={group.scope} className={styles.group}>
						{/* The application's pages need no heading: the dialog's own is theirs. The
						    project's are the ones a reader has to be told belong to something else. */}
						{group.scope === "project" && (
							<h2 className={classes("text-13", "text-semibold", styles.groupHeading)}>
								<Icon name="folder" className={styles.linkIcon} />
								<span className={styles.groupHeadingName}>{p.projectName}</span>
							</h2>
						)}

						{group.pages.map((page) => (
							<button
								key={page.key}
								type="button"
								aria-current={page.key === selected ? "page" : undefined}
								className={classes(
									"text-13",
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
			</nav>

			<div className={styles.content}>
				<div className={styles.contentColumn}>
					<Suspense fallback={<div className="text-13">Loading…</div>}>
						<Content projectId={p.projectId} />
					</Suspense>
				</div>
			</div>
		</Modal>
	);
};
