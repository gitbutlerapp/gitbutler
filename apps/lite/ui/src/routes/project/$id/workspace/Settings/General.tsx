import { useSuspenseQueries } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useState, type FC } from "react";
import {
	guiSettingsQueryOptions,
	listEditorsQueryOptions,
	listProjectsQueryOptions,
	terminalsQueryOptions,
	userProfileQueryOptions,
} from "#ui/api/queries.ts";
import { useDeleteAllData, useSaveGUISettings } from "#ui/api/mutations.ts";
import { AccountSection, SignOutRow } from "./Account.tsx";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { ProgramIcon } from "#ui/components/ProgramIcon.tsx";
import { Select } from "#ui/components/Select.tsx";
import { Switch } from "#ui/components/Switch.tsx";
import { defaultSettings } from "#ui/settings.ts";
import { CheckForUpdatesButton } from "#ui/CheckForUpdatesButton.tsx";
import styles from "./General.module.css";
import { Row, Section } from "./Section.tsx";

const prNotificationLevels = [
	{ value: "loud", label: "Loud" },
	{ value: "quiet", label: "Quiet" },
	{ value: "off", label: "Off" },
] as const;

export const General: FC = () => {
	const [
		{ data: editors },
		{ data: terminals },
		{ data: settings },
		{ data: projects },
		{ data: profile },
	] = useSuspenseQueries({
		queries: [
			listEditorsQueryOptions,
			terminalsQueryOptions,
			guiSettingsQueryOptions,
			listProjectsQueryOptions,
			userProfileQueryOptions,
		],
	});
	const { mutate: saveGUISettings } = useSaveGUISettings();
	const { isPending: isRemoving, mutate: deleteAllData } = useDeleteAllData();
	const navigate = useNavigate();

	const [confirmingRemoveAll, setConfirmingRemoveAll] = useState(false);

	const removeAllProjects = () =>
		deleteAllData(undefined, {
			// Every route below /project is gone, so leave before one notices.
			onSuccess: () => void navigate({ to: "/" }),
		});

	return (
		<>
			<AccountSection profile={profile} />

			<Section>
				<Row label="Default editor">
					<Select
						aria-label="Default editor"
						className={styles.select}
						placeholder="Select an editor…"
						items={editors.map((editor) => ({
							value: editor.id,
							label: editor.name,
							leading: <ProgramIcon program={editor.id} />,
						}))}
						value={settings.editorId ?? null}
						// The placeholder row stands for no choice; the setting has no such value.
						onValueChange={(editorId) => editorId !== null && saveGUISettings({ editorId })}
					/>
				</Row>

				<Row label="Default terminal">
					<Select
						aria-label="Default terminal"
						className={styles.select}
						placeholder="Select a terminal…"
						items={terminals.map((terminal) => ({
							value: terminal.identifier,
							label: terminal.displayName,
							leading: <ProgramIcon program={terminal.identifier} />,
						}))}
						value={settings.terminalId ?? null}
						onValueChange={(terminalId) => terminalId !== null && saveGUISettings({ terminalId })}
					/>
				</Row>
			</Section>

			<Section>
				<Row
					label="Check for updates automatically"
					labelId="auto-update"
					hint="An update already downloaded still installs on quit."
					below={<CheckForUpdatesButton />}
				>
					<Switch
						size="large"
						aria-labelledby="auto-update"
						checked={settings.autoUpdate ?? defaultSettings.autoUpdate}
						onCheckedChange={(autoUpdate) => saveGUISettings({ autoUpdate })}
					/>
				</Row>

				<Row
					label="Desktop notifications"
					labelId="desktop-notifications"
					hint="Loud activity that arrives while GitButler is in the background is also shown by the system."
				>
					<Switch
						size="large"
						aria-labelledby="desktop-notifications"
						checked={settings.desktopNotifications ?? defaultSettings.desktopNotifications}
						disabled={(settings.prNotifications ?? defaultSettings.prNotifications) !== "loud"}
						onCheckedChange={(desktopNotifications) => saveGUISettings({ desktopNotifications })}
					/>
				</Row>

				<Row
					label="Pull request activity"
					hint="Loud collects notifications in the bell; quiet and off keep it hidden."
				>
					<Select
						aria-label="Pull request activity"
						className={styles.select}
						items={prNotificationLevels}
						value={settings.prNotifications ?? defaultSettings.prNotifications}
						onValueChange={(prNotifications) =>
							prNotifications !== null && saveGUISettings({ prNotifications })
						}
					/>
				</Row>
			</Section>

			<Section>
				{profile !== null && <SignOutRow />}

				<Row
					label="Remove all projects"
					hint={`Forgets all ${projects.length} of them. The repositories on disk are untouched.`}
				>
					{confirmingRemoveAll ? (
						<div className={styles.confirm}>
							<button
								type="button"
								className={getButtonClassName({ variant: "danger" })}
								disabled={isRemoving}
								onClick={removeAllProjects}
							>
								{isRemoving ? "Removing…" : "Confirm"}
							</button>
							<button
								type="button"
								className={getButtonClassName({})}
								disabled={isRemoving}
								onClick={() => setConfirmingRemoveAll(false)}
							>
								Cancel
							</button>
						</div>
					) : (
						<button
							type="button"
							className={getButtonClassName({ variant: "danger" })}
							disabled={projects.length === 0}
							onClick={() => setConfirmingRemoveAll(true)}
						>
							<Icon name="bin" />
							Remove all…
						</button>
					)}
				</Row>
			</Section>
		</>
	);
};
