import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FC } from "react";
import type { UserProfile } from "@gitbutler/but-sdk";
import { aiConfigurationQueryOptions, userProfileQueryOptions } from "#ui/api/queries.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import styles from "./Account.module.css";
import { pollUntilSuccess } from "./poll.ts";
import { Row, Section } from "./Section.tsx";

/** How long to keep asking whether the browser half of the login finished. */
const pollIntervalMs = 2_000;
const pollTimeoutMs = 3 * 60 * 1000;

/** The API takes the image inline, so the bytes have to be base64 before they go. */
const readAsBase64 = (file: File): Promise<string> =>
	new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.addEventListener("load", () => {
			// A data URL, of which the API wants only the payload.
			const result = typeof reader.result === "string" ? reader.result : "";
			resolve(result.slice(result.indexOf(",") + 1));
		});
		reader.addEventListener("error", () => reject(new Error("Could not read the image")));
		reader.readAsDataURL(file);
	});

const SignedOut: FC = () => {
	const client = useQueryClient();
	const [signingIn, setSigningIn] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const inFlight = useRef<AbortController | null>(null);

	// Closing the dialog abandons the sign-in, so the poll should go with it.
	useEffect(() => () => inFlight.current?.abort(), []);

	const signIn = async () => {
		const controller = new AbortController();
		inFlight.current = controller;
		setSigningIn(true);
		setError(null);
		try {
			const login = await window.lite.getLoginToken();
			// Names the client for the login page, as apps/desktop does with its
			// build type. Without it the page can only offer a token to copy.
			const url = new URL(login.url);
			url.searchParams.set("bt", "release");
			await window.lite.openInWebBrowser(url.toString());
			// The page sends the account back over `but://login`, which the main
			// process persists, so this waits for the account to appear rather
			// than for a reply of its own.
			await pollUntilSuccess({
				attempt: async () => {
					// Signed out reads as `null` rather than an error, which would end
					// the poll on its first try. The message is only ever seen if the
					// poll then runs out of time, so it reads as the failure.
					const profile = await window.lite.getUserProfileLocal();
					if (profile === null) throw new Error("The browser did not finish signing in");
					return profile;
				},
				intervalMs: pollIntervalMs,
				timeoutMs: pollTimeoutMs,
				signal: controller.signal,
			});
			await client.invalidateQueries({ queryKey: userProfileQueryOptions.queryKey });
			await client.invalidateQueries({ queryKey: aiConfigurationQueryOptions.queryKey });
		} catch (caught) {
			// Abandoning is not a failure to report.
			if (!controller.signal.aborted) setError(errorMessageForToast(caught));
		} finally {
			if (!controller.signal.aborted) setSigningIn(false);
		}
	};

	return (
		<Section>
			<Row
				label="GitButler account"
				hint={
					error ?? "Opens gitbutler.com to sign in. Your access token stays in the app's backend."
				}
			>
				<button
					type="button"
					className={getButtonClassName({ variant: "pop", size: "small" })}
					disabled={signingIn}
					onClick={() => void signIn()}
				>
					{signingIn ? "Waiting for browser…" : "Sign in"}
				</button>
			</Row>
		</Section>
	);
};

export const AccountSection: FC<{ profile: UserProfile | null }> = ({ profile }) => {
	if (profile === null) return <SignedOut />;

	// Keyed on the account, so the form's fields seed from it. Signing in swaps the profile
	// under a component that is already mounted, and state seeded while signed out would
	// keep the empty name it started with — and read as an edit worth saving.
	return <SignedIn key={profile.id} profile={profile} />;
};

const SignedIn: FC<{ profile: UserProfile }> = ({ profile }) => {
	const client = useQueryClient();
	const pictureInput = useRef<HTMLInputElement>(null);

	const [name, setName] = useState(profile.name ?? "");
	const [pendingPicture, setPendingPicture] = useState<{ base64: string; filename: string } | null>(
		null,
	);
	const [previewUrl, setPreviewUrl] = useState<string | null>(null);
	const [saving, setSaving] = useState(false);
	const [error, setError] = useState<string | null>(null);

	// An object URL is held by the document, not by the state that named it, so each one
	// has to be handed back when it is replaced, cleared on save, or unmounted.
	useEffect(() => {
		if (previewUrl === null) return;
		return () => URL.revokeObjectURL(previewUrl);
	}, [previewUrl]);

	const dirty = name !== (profile.name ?? "") || pendingPicture !== null;

	const choosePicture = async (file: File) => {
		try {
			setPendingPicture({ base64: await readAsBase64(file), filename: file.name });
			setPreviewUrl(URL.createObjectURL(file));
		} catch (caught) {
			setError(errorMessageForToast(caught));
		}
	};

	const save = async () => {
		setSaving(true);
		setError(null);
		try {
			// UpdateUserParams has no rename_all, so its wire shape is snake_case.
			await window.lite.updateProfileAndPersist({
				name: name.trim() === "" ? null : name,
				website: null,
				twitter: null,
				bluesky: null,
				timezone: null,
				location: null,
				email_share: null,
				avatar_base64: pendingPicture?.base64 ?? null,
				avatar_filename: pendingPicture?.filename ?? null,
			});
			setPendingPicture(null);
			setPreviewUrl(null);
			await client.invalidateQueries({ queryKey: userProfileQueryOptions.queryKey });
		} catch (caught) {
			setError(errorMessageForToast(caught));
		} finally {
			setSaving(false);
		}
	};

	const signOut = async () => {
		try {
			await window.lite.deleteUser();
			await client.invalidateQueries({ queryKey: userProfileQueryOptions.queryKey });
			await client.invalidateQueries({ queryKey: aiConfigurationQueryOptions.queryKey });
		} catch (caught) {
			// Otherwise the click leaves a rejected promise and the account still showing.
			setError(errorMessageForToast(caught));
		}
	};

	return (
		<>
			{/* Not the label-and-control grid the other settings use: a form of its own,
			    with the picture beside the fields it belongs to. */}
			<section className={styles.profileCard}>
				<button
					type="button"
					className={styles.avatarButton}
					aria-label="Change profile picture"
					onClick={() => pictureInput.current?.click()}
				>
					{(previewUrl ?? profile.picture) !== "" ? (
						<img src={previewUrl ?? profile.picture} alt="" className={styles.avatar} />
					) : (
						<span className={classes("text-12", styles.avatarEmpty)}>Choose</span>
					)}
					<span className={classes("text-12", styles.avatarOverlay)}>Change</span>
				</button>
				<input
					ref={pictureInput}
					type="file"
					accept="image/png,image/jpeg"
					className={styles.fileInput}
					onChange={(evt) => {
						const file = evt.currentTarget.files?.[0];
						// Cleared so choosing the same file again still counts as a change, which
						// is what a retry after a failed save looks like.
						evt.currentTarget.value = "";
						if (file) void choosePicture(file);
					}}
				/>

				<div className={styles.fields}>
					<label className={classes("text-12", styles.fieldLabel)} htmlFor="account-name">
						Full name
					</label>
					<input
						id="account-name"
						type="text"
						className={classes("text-13", styles.field)}
						value={name}
						onChange={(evt) => setName(evt.currentTarget.value)}
					/>

					<label className={classes("text-12", styles.fieldLabel)} htmlFor="account-email">
						Email
					</label>
					<input
						id="account-email"
						type="text"
						className={classes("text-13", styles.field, styles.fieldReadonly)}
						value={profile.email ?? ""}
						readOnly
						title="Changed on gitbutler.com"
					/>

					<div className={styles.formActions}>
						{error !== null && <span className={classes("text-12", styles.error)}>{error}</span>}
						<button
							type="button"
							className={getButtonClassName({ variant: "pop" })}
							disabled={!dirty || saving}
							onClick={() => void save()}
						>
							{saving ? "Updating…" : "Update profile"}
						</button>
					</div>
				</div>
			</section>

			<Section>
				<Row
					label="Forget credentials and log out"
					hint="Clears the account from this machine. Your repositories are untouched."
				>
					<button
						type="button"
						className={getButtonClassName({ size: "small" })}
						onClick={() => void signOut()}
					>
						Forget credentials
					</button>
				</Row>
			</Section>
		</>
	);
};
