import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FC } from "react";
import type { UserProfile } from "@gitbutler/but-sdk";
import { Field } from "@base-ui/react";
import { aiConfigurationQueryOptions, userProfileQueryOptions } from "#ui/api/queries.ts";
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import {
	FieldControlStyles,
	FieldLabelStyles,
	FieldRootStyles,
} from "@gitbutler/ui-react/Field.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { Illustration } from "@gitbutler/ui-react/Illustration.tsx";
import { ProfileImage } from "@gitbutler/ui-react/ProfileImage.tsx";
import { errorMessageForToast } from "#ui/errors.ts";
import styles from "./Account.module.css";
import { pollUntilSuccess } from "./poll.ts";
import { Row } from "./Section.tsx";

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
		// A card of its own rather than a row: the drawing leads, and the button sits under
		// the words rather than at the row's end.
		<section className={classes(styles.card, styles.signedOutCard)}>
			<Illustration name="id-card" />
			<div className={styles.signedOut}>
				<div className={styles.signedOutText}>
					<span className={classes("text-15", "text-semibold", styles.signedOutLabel)}>
						GitButler account
					</span>
					<span className={classes("text-12", "text-body", styles.signedOutHint)}>
						{error ?? (
							<>
								Log in to sync your account and pull requests.
								<br />
								Your access token stays in the app&apos;s backend.
							</>
						)}
					</span>
				</div>
				<Button variant="gray" disabled={signingIn} onClick={() => void signIn()}>
					{signingIn ? "Waiting for browser…" : "Log in to GitButler"}
					<Icon name={signingIn ? "spinner" : "login"} />
				</Button>
			</div>
		</section>
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

	const [name, setName] = useState(profile.name ?? "");
	// The chosen picture, with a data URL to preview it by: CSP allows `data:` images but not
	// `blob:` ones, and the bytes are already base64 for the upload.
	const [pendingPicture, setPendingPicture] = useState<{
		base64: string;
		filename: string;
		previewUrl: string;
	} | null>(null);
	// The uploaded picture, marked to go on save.
	const [removingPicture, setRemovingPicture] = useState(false);
	const [saving, setSaving] = useState(false);
	const [error, setError] = useState<string | null>(null);

	const dirty = name !== (profile.name ?? "") || pendingPicture !== null || removingPicture;
	const picture = removingPicture ? null : (pendingPicture?.previewUrl ?? profile.picture);
	// Only an uploaded picture can be removed; the sign-in picture and Gravatar are what
	// removing falls back to. The API says which by its storage path until it says so
	// outright (GB-2076).
	const uploadedPicture = profile.picture.includes("/rails/active_storage/");

	const choosePicture = async (file: File) => {
		try {
			const base64 = await readAsBase64(file);
			setPendingPicture({
				base64,
				filename: file.name,
				previewUrl: `data:${file.type};base64,${base64}`,
			});
			setRemovingPicture(false);
		} catch (caught) {
			setError(errorMessageForToast(caught));
		}
	};

	const save = async () => {
		setSaving(true);
		setError(null);
		try {
			// UpdateUserParams has no rename_all, so its wire shape is snake_case.
			const updated = await window.lite.updateProfileAndPersist({
				name: name.trim() === "" ? null : name,
				website: null,
				twitter: null,
				bluesky: null,
				timezone: null,
				location: null,
				email_share: null,
				avatar_base64: pendingPicture?.base64 ?? null,
				avatar_filename: pendingPicture?.filename ?? null,
				remove_avatar: removingPicture ? true : null,
			});
			// The saved profile goes in before the preview comes out: dropping the preview
			// first showed the cached profile, and its old picture, until a refetch landed.
			client.setQueryData(userProfileQueryOptions.queryKey, updated);
			setPendingPicture(null);
			setRemovingPicture(false);
		} catch (caught) {
			setError(errorMessageForToast(caught));
		} finally {
			setSaving(false);
		}
	};

	return (
		// Not the rows the other settings use: a form of its own, with the picture beside the
		// fields it belongs to.
		<section className={styles.card}>
			<ProfileImage
				src={picture}
				// The email first: it is what the server's own fallback, Gravatar, is keyed on.
				seed={profile.email ?? profile.login ?? String(profile.id)}
				onChoose={(file) => void choosePicture(file)}
				// Remove drops a picture chosen but not saved; failing that, it marks the
				// uploaded one to go on save.
				onRemove={
					pendingPicture !== null
						? () => setPendingPicture(null)
						: uploadedPicture && !removingPicture
							? () => setRemovingPicture(true)
							: undefined
				}
			/>

			<div className={styles.fields}>
				<Field.Root render={<FieldRootStyles />}>
					<Field.Label render={<FieldLabelStyles />}>Email</Field.Label>
					<Field.Control
						render={<FieldControlStyles />}
						value={profile.email ?? ""}
						disabled
						title="Changed on gitbutler.com"
					/>
				</Field.Root>

				<div className={styles.nameRow}>
					<Field.Root render={<FieldRootStyles />} className={styles.nameField}>
						<Field.Label render={<FieldLabelStyles />}>Full name</Field.Label>
						<Field.Control
							render={<FieldControlStyles />}
							value={name}
							onValueChange={(value) => setName(value)}
						/>
					</Field.Root>
					<Button variant="gray" disabled={!dirty || saving} onClick={() => void save()}>
						{saving ? "Saving…" : "Save changes"}
					</Button>
				</div>

				{error !== null && <span className={classes("text-12", styles.error)}>{error}</span>}
			</div>
		</section>
	);
};

/** Forgets the account on this machine. Only for a page that knows someone is signed in. */
export const SignOutRow: FC = () => {
	const client = useQueryClient();
	const [error, setError] = useState<string | null>(null);

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
		<Row
			label="Forget credentials and log out"
			hint={error ?? "Clears the account from this machine. Your repositories are untouched."}
		>
			<Button onClick={() => void signOut()}>
				Sign out
				<Icon name="logout" />
			</Button>
		</Row>
	);
};
