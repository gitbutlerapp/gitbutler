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
import { AccountSection } from "./Account.tsx";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Switch } from "#ui/components/Switch.tsx";
import { defaultSettings } from "#ui/settings.ts";
import styles from "./General.module.css";
import { Row, Section } from "./Section.tsx";

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
				<Row label="Default editor" htmlFor="editor">
					<select
						id="editor"
						value={settings.editorId ?? ""}
						onChange={(evt) => saveGUISettings({ editorId: evt.currentTarget.value })}
					>
						<option value="" disabled>
							Select an editor...
						</option>
						{editors.map((editor) => (
							<option key={editor.id} value={editor.id}>
								{editor.name}
							</option>
						))}
					</select>
				</Row>

				<Row label="Default terminal" htmlFor="terminal">
					<select
						id="terminal"
						value={settings.terminalId ?? ""}
						onChange={(evt) => saveGUISettings({ terminalId: evt.currentTarget.value })}
					>
						<option value="" disabled>
							Select a terminal...
						</option>
						{terminals.map((terminal) => (
							<option key={terminal.identifier} value={terminal.identifier}>
								{terminal.displayName}
							</option>
						))}
					</select>
				</Row>

				<Row
					label="Check for updates automatically"
					labelId="auto-update"
					hint="An update already downloaded still installs on quit."
				>
					<Switch
						aria-labelledby="auto-update"
						checked={settings.autoUpdate ?? defaultSettings.autoUpdate}
						onCheckedChange={(autoUpdate) => saveGUISettings({ autoUpdate })}
					/>
				</Row>
			</Section>

			<Section heading="Danger zone">
				<Row
					label="Remove all projects"
					hint={`Forgets all ${projects.length} of them. The repositories on disk are untouched.`}
				>
					{confirmingRemoveAll ? (
						<div className={styles.confirm}>
							<button
								type="button"
								className={getButtonClassName({ variant: "danger", size: "small" })}
								disabled={isRemoving}
								onClick={removeAllProjects}
							>
								{isRemoving ? "Removing…" : "Confirm"}
							</button>
							<button
								type="button"
								className={getButtonClassName({ size: "small" })}
								disabled={isRemoving}
								onClick={() => setConfirmingRemoveAll(false)}
							>
								Cancel
							</button>
						</div>
					) : (
						<button
							type="button"
							className={getButtonClassName({ variant: "danger", size: "small" })}
							disabled={projects.length === 0}
							onClick={() => setConfirmingRemoveAll(true)}
						>
							Remove all…
						</button>
					)}
				</Row>
			</Section>
		</>
	);
};
