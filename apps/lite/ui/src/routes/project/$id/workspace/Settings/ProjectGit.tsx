import { useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { useState, type FC } from "react";
import type { GitConfigSettings } from "@gitbutler/but-sdk";
import {
	gbConfigQueryOptions,
	headInfoQueryOptions,
	listProjectsQueryOptions,
	signingSettingsQueryOptions,
} from "#ui/api/queries.ts";
import {
	failed,
	firstLine,
	type CredentialCheckState,
} from "#ui/routes/project/$id/workspace/Settings/credential-check.ts";
import { useSetGbConfig, useUpdateProjectSettings } from "#ui/api/mutations.ts";
import { assert } from "#ui/assert.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Switch } from "#ui/components/Switch.tsx";
import { classes } from "#ui/components/classes.ts";
import styles from "./ProjectGit.module.css";
import { changing } from "./project-settings.ts";
import { Row, Section } from "./Section.tsx";

type SigningFormat = "openpgp" | "ssh";

const signingFormats = [
	{
		value: "openpgp",
		label: "GPG",
		keyPlaceholder: "ex: 723CCA3AC13CF28D",
		programPlaceholder: "ex: /usr/local/bin/gpg",
	},
	{
		value: "ssh",
		label: "SSH",
		keyPlaceholder: "ex: /Users/bob/.ssh/id_rsa.pub",
		programPlaceholder: "ex: /Applications/1Password.app/Contents/MacOS/op-ssh-sign",
	},
] as const satisfies ReadonlyArray<
	{ value: SigningFormat; label: string } & Record<string, string>
>;

/** The program field writes to whichever of the two git keys the format selects. */
const programOf = (config: GitConfigSettings, format: SigningFormat): string =>
	(format === "openpgp" ? config.gpgProgram : config.gpgSshProgram) ?? "";

export const ProjectGit: FC<{ projectId: string }> = ({ projectId }) => {
	const { data: config } = useSuspenseQuery(gbConfigQueryOptions(projectId));
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	// Credentials are tested against the target's remote, the same pair a push uses.
	const { data: target } = useQuery({
		...headInfoQueryOptions(projectId),
		select: (info) => info.target?.remoteTrackingRef ?? null,
	});
	const [credentials, setCredentials] = useState<CredentialCheckState>({ _tag: "Idle" });
	const project = assert(projects.find((candidate) => candidate.id === projectId));
	const { mutate: setGbConfig } = useSetGbConfig();
	const { mutate: updateProjectSettings } = useUpdateProjectSettings();

	const {
		data: signingWorks,
		error: signingError,
		isFetching: isCheckingSigning,
		refetch: checkSigning,
	} = useQuery(signingSettingsQueryOptions(projectId));

	const signCommits = config.signCommits ?? false;
	const format: SigningFormat = config.signingFormat === "ssh" ? "ssh" : "openpgp";
	const selected = signingFormats.find((option) => option.value === format);

	// Held locally so a refetch cannot interrupt typing; committed on blur or Enter.
	const [key, setKey] = useState(config.signingKey ?? "");
	const [program, setProgram] = useState(programOf(config, format));

	const save = (update: Partial<GitConfigSettings>) =>
		setGbConfig({ projectId, config: { ...config, ...update } });

	// Each format keeps its own program key, so switching only moves which one is shown
	// and edited. Writing `program` across would put the old format's binary in the new
	// format's slot and blank the one it came from.
	const saveFormat = (next: SigningFormat) => {
		setProgram(programOf(config, next));
		save({ signingFormat: next });
	};

	const saveProgram = () =>
		save(format === "openpgp" ? { gpgProgram: program } : { gpgSshProgram: program });

	const checkCredentials = async () => {
		if (target === null || target === undefined) return;
		const { remoteName, displayName: branchName } = target;

		setCredentials({ _tag: "Running", checks: [{ name: "Fetch" }] });
		let fetchError: string | undefined;
		try {
			await window.lite.gitTestFetch({ projectId, remoteName, action: "modal" });
		} catch (error) {
			fetchError = firstLine(error);
		}

		// Push is still worth attempting after a fetch failure: read and write
		// credentials are not always the same thing.
		setCredentials({
			_tag: "Running",
			checks: [{ name: "Fetch", error: fetchError }, { name: "Push" }],
		});
		let pushError: string | undefined;
		try {
			await window.lite.gitTestPush({ projectId, remoteName, branchName });
		} catch (error) {
			pushError = firstLine(error);
		}

		setCredentials({
			_tag: "Done",
			checks: [
				{ name: "Fetch", error: fetchError },
				{ name: "Push", error: pushError },
			],
		});
	};

	return (
		<>
			<Section>
				<Row
					label="Force push protection"
					labelId="force-push-protection"
					hint="Uses git's safer force-push flags so remote commits are not overwritten."
				>
					<Switch
						aria-labelledby="force-push-protection"
						checked={project.force_push_protection ?? false}
						onCheckedChange={(forcePushProtection) =>
							updateProjectSettings({
								projectId,
								settings: changing({ forcePushProtection }),
							})
						}
					/>
				</Row>

				<Row
					label="Sign commits"
					labelId="sign-commits"
					hint="GitButler signs as your git configuration says, but gitbutler.signCommits wins."
				>
					<Switch
						aria-labelledby="sign-commits"
						checked={signCommits}
						onCheckedChange={(next) => save({ signCommits: next })}
					/>
				</Row>
			</Section>

			{signCommits && (
				<Section heading="Signing">
					<Row label="Format" htmlFor="signing-format">
						<select
							id="signing-format"
							value={format}
							onChange={(evt) => saveFormat(evt.currentTarget.value as SigningFormat)}
						>
							{signingFormats.map((option) => (
								<option key={option.value} value={option.value}>
									{option.label}
								</option>
							))}
						</select>
					</Row>

					<Row label="Signing key" htmlFor="signing-key">
						<input
							id="signing-key"
							type="text"
							placeholder={selected?.keyPlaceholder}
							value={key}
							onChange={(evt) => setKey(evt.currentTarget.value)}
							onBlur={() => save({ signingKey: key })}
							onKeyDown={(evt) => evt.key === "Enter" && save({ signingKey: key })}
						/>
					</Row>

					<Row label="Signing program" htmlFor="signing-program">
						<input
							id="signing-program"
							type="text"
							placeholder={selected?.programPlaceholder}
							value={program}
							onChange={(evt) => setProgram(evt.currentTarget.value)}
							onBlur={saveProgram}
							onKeyDown={(evt) => evt.key === "Enter" && saveProgram()}
						/>
					</Row>

					<Row label="Check signing" hint="Signs a throwaway commit to prove the settings work.">
						<div className={styles.check}>
							{signingError !== null && (
								<span className={classes("text-12", styles.failed)}>
									{signingError.message.split("\n")[0]}
								</span>
							)}
							{signingError === null && signingWorks === true && (
								<span className={classes("text-12", styles.passed)}>Signing works</span>
							)}
							<button
								type="button"
								className={getButtonClassName({ size: "small" })}
								disabled={isCheckingSigning}
								onClick={() => void checkSigning()}
							>
								{isCheckingSigning ? "Checking…" : "Check"}
							</button>
						</div>
					</Row>
				</Section>
			)}

			<Section heading="Git authentication">
				<Row
					label="Credentials"
					hint={
						target === null || target === undefined
							? "Needs a target branch with a remote to test against."
							: `Fetches from ${target.remoteName}, then pushes an empty branch and removes it again.`
					}
				>
					<button
						type="button"
						className={getButtonClassName({ size: "small" })}
						disabled={credentials._tag === "Running" || target === null || target === undefined}
						onClick={() => void checkCredentials()}
					>
						{credentials._tag === "Running" ? "Testing…" : "Re-test credentials"}
					</button>
				</Row>

				{credentials._tag !== "Idle" &&
					credentials.checks.map((check) => (
						<Row key={check.name} label={check.name}>
							<span
								className={classes(
									"text-12",
									check.error === undefined
										? credentials._tag === "Running"
											? styles.pending
											: styles.passed
										: styles.failed,
								)}
							>
								{check.error ?? (credentials._tag === "Running" ? "Checking…" : "Works")}
							</span>
						</Row>
					))}

				{credentials._tag === "Done" && !failed(credentials) && (
					<Row label="Result">
						<span className={classes("text-12", styles.passed)}>GitButler can fetch and push</span>
					</Row>
				)}
			</Section>
		</>
	);
};
